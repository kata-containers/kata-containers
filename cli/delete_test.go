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

package main

import (
	"flag"
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcMock"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func testRemoveCgroupsPathSuccessful(t *testing.T, cgroupsPathList []string) {
	if err := removeCgroupsPath("foo", cgroupsPathList); err != nil {
		t.Fatalf("This test should succeed (cgroupsPathList = %v): %s", cgroupsPathList, err)
	}
}

func TestRemoveCgroupsPathEmptyPathSuccessful(t *testing.T) {
	testRemoveCgroupsPathSuccessful(t, []string{})
}

func TestRemoveCgroupsPathNonEmptyPathSuccessful(t *testing.T) {
	cgroupsPath, err := ioutil.TempDir(testDir, "cgroups-path-")
	if err != nil {
		t.Fatalf("Could not create temporary cgroups directory: %s", err)
	}

	if err := os.MkdirAll(cgroupsPath, testDirMode); err != nil {
		t.Fatalf("CgroupsPath directory %q could not be created: %s", cgroupsPath, err)
	}

	testRemoveCgroupsPathSuccessful(t, []string{cgroupsPath})

	if _, err := os.Stat(cgroupsPath); err == nil {
		t.Fatalf("CgroupsPath directory %q should have been removed: %s", cgroupsPath, err)
	}
}

func TestDeleteInvalidContainer(t *testing.T) {
	assert := assert.New(t)

	// Missing container id
	err := delete("", false)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))

	// Mock Listpod error
	err = delete(testContainerID, false)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	// Container missing in ListPod
	err = delete(testContainerID, false)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestDeleteMissingContainerTypeAnnotation(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID:          pod.ID(),
						Annotations: map[string]string{},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	err := delete(pod.ID(), false)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestDeleteInvalidConfig(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: pod.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
						},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	err := delete(pod.ID(), false)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func testConfigSetup(t *testing.T) string {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")
	err = os.MkdirAll(bundlePath, testDirMode)
	assert.NoError(err)

	err = createOCIConfig(bundlePath)
	assert.NoError(err)

	// config json path
	configPath := filepath.Join(bundlePath, "config.json")
	return configPath
}

func TestDeletePod(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: pod.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
							vcAnnotations.ConfigJSONKey:    configJSON,
						},
						State: vc.State{
							State: "ready",
						},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	err = delete(pod.ID(), false)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	testingImpl.StopPodFunc = func(podID string) (vc.VCPod, error) {
		return pod, nil
	}

	defer func() {
		testingImpl.StopPodFunc = nil
	}()

	err = delete(pod.ID(), false)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	testingImpl.DeletePodFunc = func(podID string) (vc.VCPod, error) {
		return pod, nil
	}

	defer func() {
		testingImpl.DeletePodFunc = nil
	}()

	err = delete(pod.ID(), false)
	assert.Nil(err)
}

func TestDeleteInvalidContainerType(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: pod.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: "InvalidType",
							vcAnnotations.ConfigJSONKey:    configJSON,
						},
						State: vc.State{
							State: "created",
						},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	// Delete an invalid container type
	err = delete(pod.ID(), false)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestDeletePodRunning(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: pod.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
							vcAnnotations.ConfigJSONKey:    configJSON,
						},
						State: vc.State{
							State: "running",
						},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	// Delete on a running pod should fail
	err = delete(pod.ID(), false)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))

	testingImpl.StopPodFunc = func(podID string) (vc.VCPod, error) {
		return pod, nil
	}

	defer func() {
		testingImpl.StopPodFunc = nil
	}()

	// Force delete a running pod
	err = delete(pod.ID(), true)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	testingImpl.DeletePodFunc = func(podID string) (vc.VCPod, error) {
		return pod, nil
	}

	defer func() {
		testingImpl.DeletePodFunc = nil
	}()

	err = delete(pod.ID(), true)
	assert.Nil(err)
}

func TestDeleteRunningContainer(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	pod.MockContainers = []*vcMock.Container{
		{
			MockID:  testContainerID,
			MockPod: pod,
		},
	}

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: pod.MockContainers[0].ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
							vcAnnotations.ConfigJSONKey:    configJSON,
						},
						State: vc.State{
							State: "running",
						},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	// Delete on a running container should fail.
	err = delete(pod.MockContainers[0].ID(), false)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))

	// force delete
	err = delete(pod.MockContainers[0].ID(), true)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	testingImpl.StopContainerFunc = testStopContainerFuncReturnNil
	defer func() {
		testingImpl.StopContainerFunc = nil
	}()

	err = delete(pod.MockContainers[0].ID(), true)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	testingImpl.DeleteContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		return &vcMock.Container{}, nil
	}

	defer func() {
		testingImpl.DeleteContainerFunc = nil
	}()

	err = delete(pod.MockContainers[0].ID(), true)
	assert.Nil(err)
}

func TestDeleteContainer(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	pod.MockContainers = []*vcMock.Container{
		{
			MockID:  testContainerID,
			MockPod: pod,
		},
	}

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: pod.MockContainers[0].ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
							vcAnnotations.ConfigJSONKey:    configJSON,
						},
						State: vc.State{
							State: "ready",
						},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	err = delete(pod.MockContainers[0].ID(), false)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	testingImpl.StopContainerFunc = testStopContainerFuncReturnNil
	defer func() {
		testingImpl.StopContainerFunc = nil
	}()

	err = delete(pod.MockContainers[0].ID(), false)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	testingImpl.DeleteContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		return &vcMock.Container{}, nil
	}

	defer func() {
		testingImpl.DeleteContainerFunc = nil
	}()

	err = delete(pod.MockContainers[0].ID(), false)
	assert.Nil(err)
}

func TestDeleteCLIFunction(t *testing.T) {
	assert := assert.New(t)

	flagSet := &flag.FlagSet{}
	app := cli.NewApp()

	ctx := cli.NewContext(app, flagSet, nil)

	fn, ok := deleteCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	// no container id in the Metadata
	err := fn(ctx)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))

	flagSet = flag.NewFlagSet("container-id", flag.ContinueOnError)
	flagSet.Parse([]string{"xyz"})
	ctx = cli.NewContext(app, flagSet, nil)

	err = fn(ctx)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))
}

func TestDeleteCLIFunctionSuccess(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	pod.MockContainers = []*vcMock.Container{
		{
			MockID:  testContainerID,
			MockPod: pod,
		},
	}

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: pod.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
							vcAnnotations.ConfigJSONKey:    configJSON,
						},
						State: vc.State{
							State: "ready",
						},
					},
				},
			},
		}, nil
	}

	testingImpl.StopPodFunc = func(podID string) (vc.VCPod, error) {
		return pod, nil
	}

	testingImpl.DeletePodFunc = func(podID string) (vc.VCPod, error) {
		return pod, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
		testingImpl.StopPodFunc = nil
		testingImpl.DeletePodFunc = nil
	}()

	flagSet := &flag.FlagSet{}
	app := cli.NewApp()

	ctx := cli.NewContext(app, flagSet, nil)

	fn, ok := deleteCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))

	flagSet = flag.NewFlagSet("container-id", flag.ContinueOnError)
	flagSet.Parse([]string{pod.ID()})
	ctx = cli.NewContext(app, flagSet, nil)
	assert.NotNil(ctx)

	err = fn(ctx)
	assert.NoError(err)
}

func TestRemoveCGroupsPath(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip(testDisabledNeedNonRoot)
	}

	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	dir := filepath.Join(tmpdir, "dir")

	err = os.Mkdir(dir, testDirMode)
	assert.NoError(err)

	// make directory unreadable by non-root user
	err = os.Chmod(tmpdir, 0000)
	assert.NoError(err)
	defer func() {
		_ = os.Chmod(tmpdir, 0755)
	}()

	err = removeCgroupsPath("foo", []string{dir})
	assert.Error(err)
}
