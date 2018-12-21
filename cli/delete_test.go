// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"flag"
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func testRemoveCgroupsPathSuccessful(t *testing.T, cgroupsPathList []string) {
	if err := removeCgroupsPath(context.Background(), "foo", cgroupsPathList); err != nil {
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
	err := delete(context.Background(), "", false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	// Container missing in ListSandbox
	err = delete(context.Background(), testContainerID, false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestDeleteMissingContainerTypeAnnotation(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	path, err := createTempContainerIDMapping(sandbox.ID(), sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID:          sandbox.ID(),
			Annotations: map[string]string{},
		}, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	err = delete(context.Background(), sandbox.ID(), false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestDeleteInvalidConfig(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	path, err := createTempContainerIDMapping(sandbox.ID(), sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
			},
		}, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	err = delete(context.Background(), sandbox.ID(), false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func testConfigSetup(t *testing.T) (rootPath string, configPath string) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")
	err = os.MkdirAll(bundlePath, testDirMode)
	assert.NoError(err)

	err = createOCIConfig(bundlePath)
	assert.NoError(err)

	// config json path
	configPath = filepath.Join(bundlePath, "config.json")
	return tmpdir, configPath
}

func TestDeleteSandbox(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	path, err := createTempContainerIDMapping(sandbox.ID(), sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
				vcAnnotations.ConfigJSONKey:    configJSON,
			},
			State: types.State{
				State: "ready",
			},
		}, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	err = delete(context.Background(), sandbox.ID(), false)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.StatusSandboxFunc = func(ctx context.Context, sandboxID string) (vc.SandboxStatus, error) {
		return vc.SandboxStatus{
			ID: sandbox.ID(),
			State: types.State{
				State: types.StateReady,
			},
		}, nil
	}

	defer func() {
		testingImpl.StatusSandboxFunc = nil
	}()

	err = delete(context.Background(), sandbox.ID(), false)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.StopSandboxFunc = func(ctx context.Context, sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.StopSandboxFunc = nil
	}()

	err = delete(context.Background(), sandbox.ID(), false)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.DeleteSandboxFunc = func(ctx context.Context, sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.DeleteSandboxFunc = nil
	}()

	err = delete(context.Background(), sandbox.ID(), false)
	assert.Nil(err)
}

func TestDeleteInvalidContainerType(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	path, err := createTempContainerIDMapping(sandbox.ID(), sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: "InvalidType",
				vcAnnotations.ConfigJSONKey:    configJSON,
			},
			State: types.State{
				State: "created",
			},
		}, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	// Delete an invalid container type
	err = delete(context.Background(), sandbox.ID(), false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestDeleteSandboxRunning(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	path, err := createTempContainerIDMapping(sandbox.ID(), sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
				vcAnnotations.ConfigJSONKey:    configJSON,
			},
			State: types.State{
				State: "running",
			},
		}, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	// Delete on a running sandbox should fail
	err = delete(context.Background(), sandbox.ID(), false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	testingImpl.StatusSandboxFunc = func(ctx context.Context, sandboxID string) (vc.SandboxStatus, error) {
		return vc.SandboxStatus{
			ID: sandbox.ID(),
			State: types.State{
				State: types.StateRunning,
			},
		}, nil
	}

	testingImpl.StopSandboxFunc = func(ctx context.Context, sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.StatusSandboxFunc = nil
		testingImpl.StopSandboxFunc = nil
	}()

	// Force delete a running sandbox
	err = delete(context.Background(), sandbox.ID(), true)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.DeleteSandboxFunc = func(ctx context.Context, sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.DeleteSandboxFunc = nil
	}()

	err = delete(context.Background(), sandbox.ID(), true)
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

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	path, err := createTempContainerIDMapping(sandbox.MockContainers[0].ID(), sandbox.MockContainers[0].ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.MockContainers[0].ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
				vcAnnotations.ConfigJSONKey:    configJSON,
			},
			State: types.State{
				State: "running",
			},
		}, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	// Delete on a running container should fail.
	err = delete(context.Background(), sandbox.MockContainers[0].ID(), false)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	path, err = createTempContainerIDMapping(sandbox.MockContainers[0].ID(), sandbox.MockContainers[0].ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	// force delete
	err = delete(context.Background(), sandbox.MockContainers[0].ID(), true)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.StopContainerFunc = testStopContainerFuncReturnNil
	defer func() {
		testingImpl.StopContainerFunc = nil
	}()

	path, err = createTempContainerIDMapping(sandbox.MockContainers[0].ID(), sandbox.MockContainers[0].ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	err = delete(context.Background(), sandbox.MockContainers[0].ID(), true)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.DeleteContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.VCContainer, error) {
		return &vcmock.Container{}, nil
	}

	defer func() {
		testingImpl.DeleteContainerFunc = nil
	}()

	path, err = createTempContainerIDMapping(sandbox.MockContainers[0].ID(), sandbox.MockContainers[0].ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	err = delete(context.Background(), sandbox.MockContainers[0].ID(), true)
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

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	path, err := createTempContainerIDMapping(sandbox.MockContainers[0].ID(), sandbox.MockContainers[0].ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.MockContainers[0].ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
				vcAnnotations.ConfigJSONKey:    configJSON,
			},
			State: types.State{
				State: "ready",
			},
		}, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	err = delete(context.Background(), sandbox.MockContainers[0].ID(), false)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	path, err = createTempContainerIDMapping(sandbox.MockContainers[0].ID(), sandbox.MockContainers[0].ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StopContainerFunc = testStopContainerFuncReturnNil
	defer func() {
		testingImpl.StopContainerFunc = nil
	}()

	err = delete(context.Background(), sandbox.MockContainers[0].ID(), false)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.DeleteContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.VCContainer, error) {
		return &vcmock.Container{}, nil
	}

	defer func() {
		testingImpl.DeleteContainerFunc = nil
	}()

	path, err = createTempContainerIDMapping(sandbox.MockContainers[0].ID(), sandbox.MockContainers[0].ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	err = delete(context.Background(), sandbox.MockContainers[0].ID(), false)
	assert.Nil(err)
}

func TestDeleteCLIFunction(t *testing.T) {
	assert := assert.New(t)

	flagSet := &flag.FlagSet{}
	ctx := createCLIContext(flagSet)

	fn, ok := deleteCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	// no container id in the Metadata
	err := fn(ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	path, err := createTempContainerIDMapping("xyz", "xyz")
	assert.NoError(err)
	defer os.RemoveAll(path)

	flagSet = flag.NewFlagSet("container-id", flag.ContinueOnError)
	flagSet.Parse([]string{"xyz"})
	ctx = createCLIContext(flagSet)

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

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	path, err := createTempContainerIDMapping(sandbox.ID(), sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
				vcAnnotations.ConfigJSONKey:    configJSON,
			},
			State: types.State{
				State: "ready",
			},
		}, nil
	}

	testingImpl.StatusSandboxFunc = func(ctx context.Context, sandboxID string) (vc.SandboxStatus, error) {
		return vc.SandboxStatus{
			ID: sandbox.ID(),
			State: types.State{
				State: types.StateReady,
			},
		}, nil
	}

	testingImpl.StopSandboxFunc = func(ctx context.Context, sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	testingImpl.DeleteSandboxFunc = func(ctx context.Context, sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
		testingImpl.StopSandboxFunc = nil
		testingImpl.DeleteSandboxFunc = nil
	}()

	flagSet := &flag.FlagSet{}
	ctx := createCLIContext(flagSet)

	fn, ok := deleteCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	flagSet = flag.NewFlagSet("container-id", flag.ContinueOnError)
	flagSet.Parse([]string{sandbox.ID()})
	ctx = createCLIContext(flagSet)
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

	err = removeCgroupsPath(context.Background(), "foo", []string{dir})
	assert.Error(err)
}
