// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"testing"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/drivers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/manager"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

func TestGetAnnotations(t *testing.T) {
	annotations := map[string]string{
		"annotation1": "abc",
		"annotation2": "xyz",
		"annotation3": "123",
	}

	container := Container{
		config: &ContainerConfig{
			Annotations: annotations,
		},
	}

	containerAnnotations := container.GetAnnotations()

	for k, v := range containerAnnotations {
		assert.Equal(t, annotations[k], v)
	}
}

func TestContainerSystemMountsInfo(t *testing.T) {
	mounts := []Mount{
		{
			Source:      "/dev",
			Destination: "/dev",
			Type:        "bind",
		},
		{
			Source:      "procfs",
			Destination: "/proc",
			Type:        "procfs",
		},
	}

	c := Container{
		mounts: mounts,
	}

	assert.False(t, c.systemMountsInfo.BindMountDev)
	c.getSystemMountInfo()
	assert.True(t, c.systemMountsInfo.BindMountDev)

	c.mounts[0].Type = "tmpfs"
	c.getSystemMountInfo()
	assert.False(t, c.systemMountsInfo.BindMountDev)
}

func TestContainerSandbox(t *testing.T) {
	expectedSandbox := &Sandbox{}

	container := Container{
		sandbox: expectedSandbox,
	}

	sandbox := container.Sandbox()
	assert.Exactly(t, sandbox, expectedSandbox)
}

func TestContainerRemoveDrive(t *testing.T) {
	sandbox := &Sandbox{
		ctx:        context.Background(),
		id:         "sandbox",
		devManager: manager.NewDeviceManager(manager.VirtioSCSI, false, "", nil),
		config:     &SandboxConfig{},
	}

	container := Container{
		sandbox: sandbox,
		id:      "testContainer",
	}

	container.state.Fstype = ""
	err := container.removeDrive(sandbox.ctx)

	// hotplugRemoveDevice for hypervisor should not be called.
	// test should pass without a hypervisor created for the container's sandbox.
	assert.Nil(t, err, "remove drive should succeed")

	sandbox.hypervisor = &mockHypervisor{}
	path := "/dev/hda"
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "b",
	}
	devReceiver := &api.MockDeviceReceiver{}

	device, err := sandbox.devManager.NewDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok := device.(*drivers.BlockDevice)
	assert.True(t, ok)
	err = device.Attach(sandbox.ctx, devReceiver)
	assert.Nil(t, err)

	container.state.Fstype = "xfs"
	container.state.BlockDeviceID = device.DeviceID()
	err = container.removeDrive(sandbox.ctx)
	assert.Nil(t, err, "remove drive should succeed")
}

func TestUnmountHostMountsRemoveBindHostPath(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	createFakeMountDir := func(t *testing.T, dir, prefix string) string {
		name, err := ioutil.TempDir(dir, "test-mnt-"+prefix+"-")
		if err != nil {
			t.Fatal(err)
		}
		return name
	}

	createFakeMountFile := func(t *testing.T, dir, prefix string) string {
		f, err := ioutil.TempFile(dir, "test-mnt-"+prefix+"-")
		if err != nil {
			t.Fatal(err)
		}
		f.Close()
		return f.Name()
	}

	doUnmountCheck := func(src, dest, hostPath, nonEmptyHostpath, devPath string) {
		mounts := []Mount{
			{
				Source:      src,
				Destination: dest,
				HostPath:    hostPath,
				Type:        "bind",
			},
			{
				Source:      src,
				Destination: dest,
				HostPath:    nonEmptyHostpath,
				Type:        "bind",
			},
			{
				Source:      src,
				Destination: dest,
				HostPath:    devPath,
				Type:        "dev",
			},
		}

		c := Container{
			mounts: mounts,
			ctx:    context.Background(),
		}

		if err := bindMount(c.ctx, src, hostPath, false, "private"); err != nil {
			t.Fatal(err)
		}
		defer syscall.Unmount(hostPath, 0)
		if err := bindMount(c.ctx, src, nonEmptyHostpath, false, "private"); err != nil {
			t.Fatal(err)
		}
		defer syscall.Unmount(nonEmptyHostpath, 0)
		if err := bindMount(c.ctx, src, devPath, false, "private"); err != nil {
			t.Fatal(err)
		}
		defer syscall.Unmount(devPath, 0)

		err := c.unmountHostMounts(c.ctx)
		if err != nil {
			t.Fatal(err)
		}

		for _, path := range [3]string{src, dest, devPath} {
			if _, err := os.Stat(path); err != nil {
				if os.IsNotExist(err) {
					t.Fatalf("path %s should not be removed", path)
				} else {
					t.Fatal(err)
				}
			}
		}

		if _, err := os.Stat(hostPath); err == nil {
			t.Fatal("empty host-path should be removed")
		} else if !os.IsNotExist(err) {
			t.Fatal(err)
		}

		if _, err := os.Stat(nonEmptyHostpath); err != nil {
			if os.IsNotExist(err) {
				t.Fatal("non-empty host-path should not be removed")
			} else {
				t.Fatal(err)
			}
		}
	}

	src := createFakeMountDir(t, testDir, "src")
	dest := createFakeMountDir(t, testDir, "dest")
	hostPath := createFakeMountDir(t, testDir, "host-path")
	nonEmptyHostpath := createFakeMountDir(t, testDir, "non-empty-host-path")
	devPath := createFakeMountDir(t, testDir, "dev-hostpath")
	createFakeMountDir(t, nonEmptyHostpath, "nop")
	doUnmountCheck(src, dest, hostPath, nonEmptyHostpath, devPath)

	src = createFakeMountFile(t, testDir, "src")
	dest = createFakeMountFile(t, testDir, "dest")
	hostPath = createFakeMountFile(t, testDir, "host-path")
	nonEmptyHostpath = createFakeMountFile(t, testDir, "non-empty-host-path")
	devPath = createFakeMountFile(t, testDir, "dev-host-path")
	f, err := os.OpenFile(nonEmptyHostpath, os.O_WRONLY, os.FileMode(0640))
	if err != nil {
		t.Fatal(err)
	}
	f.WriteString("nop\n")
	f.Close()
	doUnmountCheck(src, dest, hostPath, nonEmptyHostpath, devPath)
}

func testSetupFakeRootfs(t *testing.T) (testRawFile, loopDev, mntDir string, err error) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	tmpDir, err := ioutil.TempDir("", "")
	assert.NoError(err)

	testRawFile = filepath.Join(tmpDir, "raw.img")
	_, err = os.Stat(testRawFile)
	assert.True(os.IsNotExist(err))

	output, err := exec.Command("losetup", "-f").CombinedOutput()
	assert.NoError(err)
	loopDev = strings.TrimSpace(string(output[:]))

	_, err = exec.Command("fallocate", "-l", "256K", testRawFile).CombinedOutput()
	assert.NoError(err)

	_, err = exec.Command("mkfs.ext4", "-F", testRawFile).CombinedOutput()
	assert.NoError(err)

	_, err = exec.Command("losetup", loopDev, testRawFile).CombinedOutput()
	assert.NoError(err)

	mntDir = filepath.Join(tmpDir, "rootfs")
	err = os.Mkdir(mntDir, DirMode)
	assert.NoError(err)

	err = syscall.Mount(loopDev, mntDir, "ext4", uintptr(0), "")
	assert.NoError(err)
	return
}

func cleanupFakeRootfsSetup(testRawFile, loopDev, mntDir string) {
	// unmount loop device
	if mntDir != "" {
		syscall.Unmount(mntDir, 0)
	}

	// detach loop device
	if loopDev != "" {
		exec.Command("losetup", "-d", loopDev).CombinedOutput()
	}

	if _, err := os.Stat(testRawFile); err == nil {
		tmpDir := filepath.Dir(testRawFile)
		os.RemoveAll(tmpDir)
	}
}

func TestContainerAddDriveDir(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	testRawFile, loopDev, fakeRootfs, err := testSetupFakeRootfs(t)

	defer cleanupFakeRootfsSetup(testRawFile, loopDev, fakeRootfs)

	assert.NoError(err)

	sandbox := &Sandbox{
		ctx:        context.Background(),
		id:         testSandboxID,
		devManager: manager.NewDeviceManager(manager.VirtioSCSI, false, "", nil),
		hypervisor: &mockHypervisor{},
		agent:      &mockAgent{},
		config: &SandboxConfig{
			HypervisorConfig: HypervisorConfig{
				DisableBlockDeviceUse: false,
			},
		},
	}

	sandbox.store, err = persist.GetDriver()
	assert.NoError(err)
	assert.NotNil(sandbox.store)

	defer sandbox.store.Destroy(sandbox.id)

	contID := "100"
	container := Container{
		sandbox: sandbox,
		id:      contID,
		rootFs:  RootFs{Target: fakeRootfs, Mounted: true},
	}

	// Make the checkStorageDriver func variable point to a fake check function
	savedFunc := checkStorageDriver
	checkStorageDriver = func(major, minor int) (bool, error) {
		return true, nil
	}

	defer func() {
		checkStorageDriver = savedFunc
	}()

	container.state.Fstype = ""

	err = container.hotplugDrive(sandbox.ctx)
	assert.NoError(err)

	assert.NotEmpty(container.state.Fstype)
}

func TestContainerRootfsPath(t *testing.T) {

	testRawFile, loopDev, fakeRootfs, err := testSetupFakeRootfs(t)
	defer cleanupFakeRootfsSetup(testRawFile, loopDev, fakeRootfs)
	assert.Nil(t, err)

	truecheckstoragedriver := checkStorageDriver
	checkStorageDriver = func(major, minor int) (bool, error) {
		return true, nil
	}
	defer func() {
		checkStorageDriver = truecheckstoragedriver
	}()

	sandbox := &Sandbox{
		ctx:        context.Background(),
		id:         "rootfstestsandbox",
		agent:      &mockAgent{},
		hypervisor: &mockHypervisor{},
		config: &SandboxConfig{
			HypervisorConfig: HypervisorConfig{
				DisableBlockDeviceUse: false,
			},
		},
	}

	container := Container{
		id:           "rootfstestcontainerid",
		sandbox:      sandbox,
		rootFs:       RootFs{Target: fakeRootfs, Mounted: true},
		rootfsSuffix: "rootfs",
	}

	container.hotplugDrive(sandbox.ctx)
	assert.Empty(t, container.rootfsSuffix)

	// Reset the value to test the other case
	container.rootFs = RootFs{Target: fakeRootfs + "/rootfs", Mounted: true}
	container.rootfsSuffix = "rootfs"

	container.hotplugDrive(sandbox.ctx)
	assert.Equal(t, container.rootfsSuffix, "rootfs")
}

func TestCheckSandboxRunningEmptyCmdFailure(t *testing.T) {
	c := &Container{}
	err := c.checkSandboxRunning("")
	assert.NotNil(t, err, "Should fail because provided command is empty")
}

func TestCheckSandboxRunningNotRunningFailure(t *testing.T) {
	c := &Container{
		sandbox: &Sandbox{},
	}
	err := c.checkSandboxRunning("test_cmd")
	assert.NotNil(t, err, "Should fail because sandbox state is empty")
}

func TestCheckSandboxRunningSuccessful(t *testing.T) {
	c := &Container{
		sandbox: &Sandbox{
			state: types.SandboxState{
				State: types.StateRunning,
			},
		},
	}
	err := c.checkSandboxRunning("test_cmd")
	assert.Nil(t, err, "%v", err)
}

func TestContainerEnterErrorsOnContainerStates(t *testing.T) {
	assert := assert.New(t)
	c := &Container{
		sandbox: &Sandbox{
			state: types.SandboxState{
				State: types.StateRunning,
			},
		},
	}
	cmd := types.Cmd{}

	ctx := context.Background()

	// Container state undefined
	_, err := c.enter(ctx, cmd)
	assert.Error(err)

	// Container paused
	c.state.State = types.StatePaused
	_, err = c.enter(ctx, cmd)
	assert.Error(err)

	// Container stopped
	c.state.State = types.StateStopped
	_, err = c.enter(ctx, cmd)
	assert.Error(err)
}

func TestContainerWaitErrorState(t *testing.T) {
	assert := assert.New(t)
	c := &Container{
		sandbox: &Sandbox{
			state: types.SandboxState{
				State: types.StateRunning,
			},
		},
	}
	processID := "foobar"

	ctx := context.Background()

	// Container state undefined
	_, err := c.wait(ctx, processID)
	assert.Error(err)

	// Container paused
	c.state.State = types.StatePaused
	_, err = c.wait(ctx, processID)
	assert.Error(err)

	// Container stopped
	c.state.State = types.StateStopped
	_, err = c.wait(ctx, processID)
	assert.Error(err)
}

func TestKillContainerErrorState(t *testing.T) {
	assert := assert.New(t)
	c := &Container{
		sandbox: &Sandbox{
			state: types.SandboxState{
				State: types.StateRunning,
			},
		},
	}

	ctx := context.Background()

	// Container state undefined
	err := c.kill(ctx, syscall.SIGKILL, true)
	assert.Error(err)

	// Container stopped
	c.state.State = types.StateStopped
	err = c.kill(ctx, syscall.SIGKILL, true)
	assert.Error(err)
}

func TestWinsizeProcessErrorState(t *testing.T) {
	assert := assert.New(t)
	c := &Container{
		sandbox: &Sandbox{
			state: types.SandboxState{
				State: types.StateRunning,
			},
		},
	}
	processID := "foobar"

	ctx := context.Background()

	// Container state undefined
	err := c.winsizeProcess(ctx, processID, 100, 200)
	assert.Error(err)

	// Container paused
	c.state.State = types.StatePaused
	err = c.winsizeProcess(ctx, processID, 100, 200)
	assert.Error(err)

	// Container stopped
	c.state.State = types.StateStopped
	err = c.winsizeProcess(ctx, processID, 100, 200)
	assert.Error(err)
}

func TestProcessIOStream(t *testing.T) {
	assert := assert.New(t)
	c := &Container{
		sandbox: &Sandbox{
			state: types.SandboxState{
				State: types.StateRunning,
			},
		},
	}
	processID := "foobar"

	// Container state undefined
	_, _, _, err := c.ioStream(processID)
	assert.Error(err)

	// Container paused
	c.state.State = types.StatePaused
	_, _, _, err = c.ioStream(processID)
	assert.Error(err)

	// Container stopped
	c.state.State = types.StateStopped
	_, _, _, err = c.ioStream(processID)
	assert.Error(err)
}
