//
// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

package virtcontainers

import (
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"syscall"
	"testing"

	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
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

func TestContainerPod(t *testing.T) {
	expectedPod := &Pod{}

	container := Container{
		pod: expectedPod,
	}

	pod := container.Pod()

	if !reflect.DeepEqual(pod, expectedPod) {
		t.Fatalf("Expecting %+v\nGot %+v", expectedPod, pod)
	}
}

func TestContainerRemoveDrive(t *testing.T) {
	pod := &Pod{}

	container := Container{
		pod: pod,
		id:  "testContainer",
	}

	container.state.Fstype = ""
	err := container.removeDrive()

	// hotplugRemoveDevice for hypervisor should not be called.
	// test should pass without a hypervisor created for the container's pod.
	if err != nil {
		t.Fatal("")
	}

	container.state.Fstype = "xfs"
	container.state.HotpluggedDrive = false
	err = container.removeDrive()

	// hotplugRemoveDevice for hypervisor should not be called.
	if err != nil {
		t.Fatal("")
	}

	container.state.HotpluggedDrive = true
	pod.hypervisor = &mockHypervisor{}
	err = container.removeDrive()

	if err != nil {
		t.Fatal()
	}
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
	pod := &Pod{
		id:         testPodID,
		storage:    fs,
		hypervisor: &mockHypervisor{},
		agent:      &noopAgent{},
		config: &PodConfig{
			HypervisorConfig: HypervisorConfig{
				DisableBlockDeviceUse: false,
			},
		},
	}

	contID := "100"
	container := Container{
		pod:    pod,
		id:     contID,
		rootFs: fakeRootfs,
	}

	// create state file
	path := filepath.Join(runStoragePath, testPodID, container.ID())
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
	container.state.HotpluggedDrive = false

	err = container.hotplugDrive()
	if err != nil {
		t.Fatalf("Error with hotplugDrive :%v", err)
	}

	if container.state.Fstype == "" || !container.state.HotpluggedDrive {
		t.Fatal()
	}
}

func TestCheckPodRunningEmptyCmdFailure(t *testing.T) {
	c := &Container{}
	err := c.checkPodRunning("")
	assert.NotNil(t, err, "Should fail because provided command is empty")
}

func TestCheckPodRunningNotRunningFailure(t *testing.T) {
	c := &Container{
		pod: &Pod{},
	}
	err := c.checkPodRunning("test_cmd")
	assert.NotNil(t, err, "Should fail because pod state is empty")
}

func TestCheckPodRunningSuccessful(t *testing.T) {
	c := &Container{
		pod: &Pod{
			state: State{
				State: StateRunning,
			},
		},
	}
	err := c.checkPodRunning("test_cmd")
	assert.Nil(t, err, "%v", err)
}

func TestContainerAddResources(t *testing.T) {
	assert := assert.New(t)

	c := &Container{}
	err := c.addResources()
	assert.Nil(err)

	c.config = &ContainerConfig{Annotations: make(map[string]string)}
	c.config.Annotations[vcAnnotations.ContainerTypeKey] = string(PodSandbox)
	err = c.addResources()
	assert.Nil(err)

	c.config.Annotations[vcAnnotations.ContainerTypeKey] = string(PodContainer)
	err = c.addResources()
	assert.Nil(err)

	c.config.Resources = ContainerResources{
		CPUQuota:  5000,
		CPUPeriod: 1000,
	}
	c.pod = &Pod{
		hypervisor: &mockHypervisor{},
		agent:      &noopAgent{},
	}
	err = c.addResources()
	assert.Nil(err)
}

func TestContainerRemoveResources(t *testing.T) {
	assert := assert.New(t)

	c := &Container{}
	err := c.addResources()
	assert.Nil(err)

	c.config = &ContainerConfig{Annotations: make(map[string]string)}
	c.config.Annotations[vcAnnotations.ContainerTypeKey] = string(PodSandbox)
	err = c.removeResources()
	assert.Nil(err)

	c.config.Annotations[vcAnnotations.ContainerTypeKey] = string(PodContainer)
	err = c.removeResources()
	assert.Nil(err)

	c.config.Resources = ContainerResources{
		CPUQuota:  5000,
		CPUPeriod: 1000,
	}
	c.pod = &Pod{hypervisor: &mockHypervisor{}}
	err = c.removeResources()
	assert.Nil(err)
}

func TestContainerEnterErrorsOnContainerStates(t *testing.T) {
	assert := assert.New(t)
	c := &Container{
		pod: &Pod{
			state: State{
				State: StateRunning,
			},
		},
	}
	cmd := Cmd{}

	// Container state undefined
	_, err := c.enter(cmd)
	assert.Error(err)

	// Container paused
	c.state.State = StatePaused
	_, err = c.enter(cmd)
	assert.Error(err)

	// Container stopped
	c.state.State = StateStopped
	_, err = c.enter(cmd)
	assert.Error(err)
}
