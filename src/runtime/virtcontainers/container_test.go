// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"os"
	"path/filepath"
	"syscall"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/drivers"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/manager"
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
		devManager: manager.NewDeviceManager(config.VirtioSCSI, false, "", 0, nil),
		config:     &SandboxConfig{},
	}

	container := Container{
		sandbox: sandbox,
		id:      "testContainer",
	}

	container.state.Fstype = ""
	err := container.removeDrive(sandbox.ctx)

	// HotplugRemoveDevice for hypervisor should not be called.
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

func TestMountSharedDirMounts(t *testing.T) {
	if os.Getuid() != 0 {
		t.Skip("Test disabled as requires root user")
	}

	assert := assert.New(t)

	// create a new shared directory for our test:
	kataHostSharedDirSaved := kataHostSharedDir
	testHostDir := t.TempDir()
	kataHostSharedDir = func() string {
		return testHostDir
	}
	defer func() {
		kataHostSharedDir = kataHostSharedDirSaved
	}()

	// Create a kata agent
	k := &kataAgent{ctx: context.Background()}

	// Create sandbox
	sandbox := &Sandbox{
		ctx:        context.Background(),
		id:         "foobar",
		agent:      newMockAgent(),
		hypervisor: &mockHypervisor{},
		config: &SandboxConfig{
			HypervisorConfig: HypervisorConfig{
				BlockDeviceDriver: config.VirtioBlock,
			},
		},
	}

	fsShare, err := NewFilesystemShare(sandbox)
	assert.Nil(err)
	sandbox.fsShare = fsShare

	// setup the shared mounts:
	err = sandbox.fsShare.Prepare(sandbox.ctx)
	assert.NoError(err)

	//
	// Create the mounts that we'll test with
	//
	testMountPath := t.TempDir()
	secretpath := filepath.Join(testMountPath, K8sSecret)
	err = os.MkdirAll(secretpath, 0777)
	assert.NoError(err)
	secret := filepath.Join(secretpath, "super-secret-thing")
	f, err := os.Create(secret)
	assert.NoError(err)
	t.Cleanup(func() {
		if err := f.Close(); err != nil {
			t.Fatalf("failed to close file: %v", err)
		}
	})

	mountDestination := "/fluffhead/token"
	//
	// Create container to utilize this mount/secret
	//
	container := Container{
		sandbox:   sandbox,
		sandboxID: "foobar",
		id:        "test-ctr",
		mounts: []Mount{
			{
				Source:      secret,
				Destination: mountDestination,
				Type:        "bind",
			},
		},
	}

	updatedMounts := make(map[string]Mount)
	ignoredMounts := make(map[string]Mount)
	storage, err := container.mountSharedDirMounts(k.ctx, updatedMounts, ignoredMounts)
	assert.NoError(err)

	// Look at the resulting hostpath that was created. Since a unique ID is applied, we'll use this
	// to verify the updated mounts and storage object
	hostMountName := filepath.Base(container.mounts[0].HostPath)
	expectedStorageSource := filepath.Join(defaultKataGuestSharedDir, hostMountName)
	expectedStorageDest := filepath.Join(defaultKataGuestSharedDir, "watchable", hostMountName)

	// We expect a single new storage object who's source is the original mount's base path and desitation is same with -watchable appended:
	assert.Equal(len(storage), 1)
	assert.Equal(expectedStorageSource, storage[0].Source)
	assert.Equal(expectedStorageDest, storage[0].MountPoint)

	// We expect a single updated mount, who's source is the watchable mount path, and destination remains unchanged:
	assert.Equal(len(updatedMounts), 1)
	assert.Equal(updatedMounts[mountDestination].Source, expectedStorageDest)
	assert.Equal(updatedMounts[mountDestination].Destination, mountDestination)

	// Perform cleanups
	err = container.unmountHostMounts(k.ctx)
	assert.NoError(err)

	err = fsShare.Cleanup(k.ctx)
	assert.NoError(err)
}

func TestGetContainerId(t *testing.T) {
	containerIDs := []string{"abc", "foobar", "123"}
	containers := [3]*Container{}

	for i, id := range containerIDs {
		c := &Container{id: id}
		containers[i] = c
	}

	for id, container := range containers {
		assert.Equal(t, containerIDs[id], container.ID())
	}
}

func TestContainerProcess(t *testing.T) {
	assert := assert.New(t)

	expectedProcess := Process{
		Token: "foobar",
		Pid:   123,
	}
	container := &Container{
		process: expectedProcess,
	}

	process := container.Process()
	assert.Exactly(process, expectedProcess)

	token := container.GetToken()
	assert.Exactly(token, "foobar")

	pid := container.GetPid()
	assert.Exactly(pid, 123)
}

func TestConfigValid(t *testing.T) {
	assert := assert.New(t)

	//no config
	config := ContainerConfig{}
	result := config.valid()
	assert.False(result)

	//no container ID
	config = newTestContainerConfigNoop("")
	result = config.valid()
	assert.False(result)

	config = newTestContainerConfigNoop("foobar")
	result = config.valid()
	assert.True(result)
}
