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

	ktu "github.com/kata-containers/runtime/pkg/katatestutils"
	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
	"github.com/kata-containers/runtime/virtcontainers/device/manager"
	"github.com/kata-containers/runtime/virtcontainers/persist"
	"github.com/kata-containers/runtime/virtcontainers/store"
	"github.com/kata-containers/runtime/virtcontainers/types"
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
		devManager: manager.NewDeviceManager(manager.VirtioSCSI, nil),
		config:     &SandboxConfig{},
	}

	vcStore, err := store.NewVCSandboxStore(sandbox.ctx, sandbox.id)
	assert.Nil(t, err)

	sandbox.store = vcStore

	container := Container{
		sandbox: sandbox,
		id:      "testContainer",
	}

	container.state.Fstype = ""
	err = container.removeDrive()

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
	err = device.Attach(devReceiver)
	assert.Nil(t, err)
	err = sandbox.storeSandboxDevices()
	assert.Nil(t, err)

	container.state.Fstype = "xfs"
	container.state.BlockDeviceID = device.DeviceID()
	err = container.removeDrive()
	assert.Nil(t, err, "remove drive should succeed")
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
	err = os.Mkdir(mntDir, store.DirMode)
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
		devManager: manager.NewDeviceManager(manager.VirtioSCSI, nil),
		hypervisor: &mockHypervisor{},
		agent:      &noopAgent{},
		config: &SandboxConfig{
			HypervisorConfig: HypervisorConfig{
				DisableBlockDeviceUse: false,
			},
		},
	}

	defer store.DeleteAll()

	sandboxStore, err := store.NewVCSandboxStore(sandbox.ctx, sandbox.id)
	assert.Nil(err)
	sandbox.store = sandboxStore

	sandbox.newStore, err = persist.GetDriver("fs")
	assert.NoError(err)
	assert.NotNil(sandbox.newStore)

	contID := "100"
	container := Container{
		sandbox: sandbox,
		id:      contID,
		rootFs:  RootFs{Target: fakeRootfs, Mounted: true},
	}

	containerStore, err := store.NewVCContainerStore(sandbox.ctx, sandbox.id, container.id)
	assert.Nil(err)
	container.store = containerStore

	// create state file
	path := store.ContainerRuntimeRootPath(testSandboxID, container.ID())
	stateFilePath := filepath.Join(path, store.StateFile)
	os.Remove(stateFilePath)

	_, err = os.Create(stateFilePath)
	assert.NoError(err)

	// Make the checkStorageDriver func variable point to a fake check function
	savedFunc := checkStorageDriver
	checkStorageDriver = func(major, minor int) (bool, error) {
		return true, nil
	}

	defer func() {
		checkStorageDriver = savedFunc
	}()

	container.state.Fstype = ""

	err = container.hotplugDrive()
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
		agent:      &noopAgent{},
		hypervisor: &mockHypervisor{},
		config: &SandboxConfig{
			HypervisorConfig: HypervisorConfig{
				DisableBlockDeviceUse: false,
			},
		},
	}
	vcstore, err := store.NewVCSandboxStore(sandbox.ctx, sandbox.id)
	sandbox.store = vcstore
	assert.Nil(t, err)
	container := Container{
		id:           "rootfstestcontainerid",
		sandbox:      sandbox,
		rootFs:       RootFs{Target: fakeRootfs, Mounted: true},
		rootfsSuffix: "rootfs",
	}
	cvcstore, err := store.NewVCContainerStore(context.Background(),
		sandbox.id,
		container.id)
	assert.Nil(t, err)
	container.store = cvcstore

	container.hotplugDrive()
	assert.Empty(t, container.rootfsSuffix)

	// Reset the value to test the other case
	container.rootFs = RootFs{Target: fakeRootfs + "/rootfs", Mounted: true}
	container.rootfsSuffix = "rootfs"

	container.hotplugDrive()
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

	// Container state undefined
	_, err := c.enter(cmd)
	assert.Error(err)

	// Container paused
	c.state.State = types.StatePaused
	_, err = c.enter(cmd)
	assert.Error(err)

	// Container stopped
	c.state.State = types.StateStopped
	_, err = c.enter(cmd)
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

	// Container state undefined
	_, err := c.wait(processID)
	assert.Error(err)

	// Container paused
	c.state.State = types.StatePaused
	_, err = c.wait(processID)
	assert.Error(err)

	// Container stopped
	c.state.State = types.StateStopped
	_, err = c.wait(processID)
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
	// Container state undefined
	err := c.kill(syscall.SIGKILL, true)
	assert.Error(err)

	// Container stopped
	c.state.State = types.StateStopped
	err = c.kill(syscall.SIGKILL, true)
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

	// Container state undefined
	err := c.winsizeProcess(processID, 100, 200)
	assert.Error(err)

	// Container paused
	c.state.State = types.StatePaused
	err = c.winsizeProcess(processID, 100, 200)
	assert.Error(err)

	// Container stopped
	c.state.State = types.StateStopped
	err = c.winsizeProcess(processID, 100, 200)
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
