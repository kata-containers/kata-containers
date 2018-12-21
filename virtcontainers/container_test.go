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
	"reflect"
	"strings"
	"syscall"
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
	"github.com/kata-containers/runtime/virtcontainers/device/manager"
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
		if annotations[k] != v {
			t.Fatalf("Expecting ['%s']='%s', Got ['%s']='%s'\n", k, annotations[k], k, v)
		}
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

	if !reflect.DeepEqual(sandbox, expectedSandbox) {
		t.Fatalf("Expecting %+v\nGot %+v", expectedSandbox, sandbox)
	}
}

func TestContainerRemoveDrive(t *testing.T) {
	sandbox := &Sandbox{
		id:         "sandbox",
		devManager: manager.NewDeviceManager(manager.VirtioSCSI, nil),
		storage:    &filesystem{},
	}

	container := Container{
		sandbox: sandbox,
		id:      "testContainer",
	}

	container.state.Fstype = ""
	err := container.removeDrive()

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
	err = sandbox.storage.createAllResources(context.Background(), sandbox)
	if err != nil {
		t.Fatal(err)
	}

	err = sandbox.storeSandboxDevices()
	assert.Nil(t, err)

	container.state.Fstype = "xfs"
	container.state.BlockDeviceID = device.DeviceID()
	err = container.removeDrive()
	assert.Nil(t, err, "remove drive should succeed")
}

func testSetupFakeRootfs(t *testing.T) (testRawFile, loopDev, mntDir string, err error) {
	tmpDir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}

	testRawFile = filepath.Join(tmpDir, "raw.img")
	if _, err := os.Stat(testRawFile); !os.IsNotExist(err) {
		os.Remove(testRawFile)
	}

	output, err := exec.Command("losetup", "-f").CombinedOutput()
	if err != nil {
		t.Fatalf("Skipping test since no loop device available for tests : %s, %s", output, err)
		return
	}
	loopDev = strings.TrimSpace(string(output[:]))

	output, err = exec.Command("fallocate", "-l", "256K", testRawFile).CombinedOutput()
	if err != nil {
		t.Fatalf("fallocate failed %s %s", output, err)
	}

	output, err = exec.Command("mkfs.ext4", "-F", testRawFile).CombinedOutput()
	if err != nil {
		t.Fatalf("mkfs.ext4 failed for %s:  %s, %s", testRawFile, output, err)
	}

	output, err = exec.Command("losetup", loopDev, testRawFile).CombinedOutput()
	if err != nil {
		t.Fatalf("Losetup for %s at %s failed : %s, %s ", loopDev, testRawFile, output, err)
		return
	}

	mntDir = filepath.Join(tmpDir, "rootfs")
	err = os.Mkdir(mntDir, dirMode)
	if err != nil {
		t.Fatalf("Error creating dir %s: %s", mntDir, err)
	}

	err = syscall.Mount(loopDev, mntDir, "ext4", uintptr(0), "")
	if err != nil {
		t.Fatalf("Error while mounting loop device %s at %s: %s", loopDev, mntDir, err)
	}
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
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	testRawFile, loopDev, fakeRootfs, err := testSetupFakeRootfs(t)

	defer cleanupFakeRootfsSetup(testRawFile, loopDev, fakeRootfs)

	if err != nil {
		t.Fatalf("Error while setting up fake rootfs: %v, Skipping test", err)
	}

	fs := &filesystem{}
	sandbox := &Sandbox{
		id:         testSandboxID,
		devManager: manager.NewDeviceManager(manager.VirtioSCSI, nil),
		storage:    fs,
		hypervisor: &mockHypervisor{},
		agent:      &noopAgent{},
		config: &SandboxConfig{
			HypervisorConfig: HypervisorConfig{
				DisableBlockDeviceUse: false,
			},
		},
	}

	contID := "100"
	container := Container{
		sandbox: sandbox,
		id:      contID,
		rootFs:  fakeRootfs,
	}

	// create state file
	path := filepath.Join(runStoragePath, testSandboxID, container.ID())
	err = os.MkdirAll(path, dirMode)
	if err != nil {
		t.Fatal(err)
	}

	defer os.RemoveAll(path)

	stateFilePath := filepath.Join(path, stateFile)
	os.Remove(stateFilePath)

	_, err = os.Create(stateFilePath)
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(stateFilePath)

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
	if err != nil {
		t.Fatalf("Error with hotplugDrive :%v", err)
	}

	if container.state.Fstype == "" {
		t.Fatal()
	}
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
			state: types.State{
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
			state: types.State{
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
			state: types.State{
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
			state: types.State{
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
			state: types.State{
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
			state: types.State{
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
