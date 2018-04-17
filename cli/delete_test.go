// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
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
	assert.False(vcmock.IsMockError(err))

	// Mock Listsandbox error
	err = delete(testContainerID, false)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{}, nil
	}

	defer func() {
		testingImpl.ListSandboxFunc = nil
	}()

	// Container missing in ListSandbox
	err = delete(testContainerID, false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestDeleteMissingContainerTypeAnnotation(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{
			{
				ID: sandbox.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID:          sandbox.ID(),
						Annotations: map[string]string{},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListSandboxFunc = nil
	}()

	err := delete(sandbox.ID(), false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestDeleteInvalidConfig(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{
			{
				ID: sandbox.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: sandbox.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
						},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListSandboxFunc = nil
	}()

	err := delete(sandbox.ID(), false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
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

func TestDeleteSandbox(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{
			{
				ID: sandbox.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: sandbox.ID(),
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
		testingImpl.ListSandboxFunc = nil
	}()

	err = delete(sandbox.ID(), false)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.StopSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.StopSandboxFunc = nil
	}()

	err = delete(sandbox.ID(), false)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.DeleteSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.DeleteSandboxFunc = nil
	}()

	err = delete(sandbox.ID(), false)
	assert.Nil(err)
}

func TestDeleteInvalidContainerType(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{
			{
				ID: sandbox.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: sandbox.ID(),
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
		testingImpl.ListSandboxFunc = nil
	}()

	// Delete an invalid container type
	err = delete(sandbox.ID(), false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestDeleteSandboxRunning(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{
			{
				ID: sandbox.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: sandbox.ID(),
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
		testingImpl.ListSandboxFunc = nil
	}()

	// Delete on a running sandbox should fail
	err = delete(sandbox.ID(), false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	testingImpl.StopSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.StopSandboxFunc = nil
	}()

	// Force delete a running sandbox
	err = delete(sandbox.ID(), true)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.DeleteSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.DeleteSandboxFunc = nil
	}()

	err = delete(sandbox.ID(), true)
	assert.Nil(err)
}

func TestDeleteRunningContainer(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	sandbox.MockContainers = []*vcmock.Container{
		{
			MockID:      testContainerID,
			MockSandbox: sandbox,
		},
	}

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{
			{
				ID: sandbox.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: sandbox.MockContainers[0].ID(),
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
		testingImpl.ListSandboxFunc = nil
	}()

	// Delete on a running container should fail.
	err = delete(sandbox.MockContainers[0].ID(), false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	// force delete
	err = delete(sandbox.MockContainers[0].ID(), true)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.StopContainerFunc = testStopContainerFuncReturnNil
	defer func() {
		testingImpl.StopContainerFunc = nil
	}()

	err = delete(sandbox.MockContainers[0].ID(), true)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.DeleteContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		return &vcmock.Container{}, nil
	}

	defer func() {
		testingImpl.DeleteContainerFunc = nil
	}()

	err = delete(sandbox.MockContainers[0].ID(), true)
	assert.Nil(err)
}

func TestDeleteContainer(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	sandbox.MockContainers = []*vcmock.Container{
		{
			MockID:      testContainerID,
			MockSandbox: sandbox,
		},
	}

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{
			{
				ID: sandbox.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: sandbox.MockContainers[0].ID(),
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
		testingImpl.ListSandboxFunc = nil
	}()

	err = delete(sandbox.MockContainers[0].ID(), false)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.StopContainerFunc = testStopContainerFuncReturnNil
	defer func() {
		testingImpl.StopContainerFunc = nil
	}()

	err = delete(sandbox.MockContainers[0].ID(), false)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.DeleteContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		return &vcmock.Container{}, nil
	}

	defer func() {
		testingImpl.DeleteContainerFunc = nil
	}()

	err = delete(sandbox.MockContainers[0].ID(), false)
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
	assert.False(vcmock.IsMockError(err))

	flagSet = flag.NewFlagSet("container-id", flag.ContinueOnError)
	flagSet.Parse([]string{"xyz"})
	ctx = cli.NewContext(app, flagSet, nil)

	err = fn(ctx)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))
}

func TestDeleteCLIFunctionSuccess(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	sandbox.MockContainers = []*vcmock.Container{
		{
			MockID:      testContainerID,
			MockSandbox: sandbox,
		},
	}

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{
			{
				ID: sandbox.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: sandbox.ID(),
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

	testingImpl.StopSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	testingImpl.DeleteSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.ListSandboxFunc = nil
		testingImpl.StopSandboxFunc = nil
		testingImpl.DeleteSandboxFunc = nil
	}()

	flagSet := &flag.FlagSet{}
	app := cli.NewApp()

	ctx := cli.NewContext(app, flagSet, nil)

	fn, ok := deleteCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	flagSet = flag.NewFlagSet("container-id", flag.ContinueOnError)
	flagSet.Parse([]string{sandbox.ID()})
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
