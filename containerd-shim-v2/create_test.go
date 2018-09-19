// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	"github.com/containerd/containerd/namespaces"
	taskAPI "github.com/containerd/containerd/runtime/v2/task"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"

	"github.com/kata-containers/runtime/pkg/katautils"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

func TestCreateSandboxSuccess(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledNeedRoot)
	}

	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
		MockContainers: []*vcmock.Container{
			{MockID: testContainerID},
		},
	}

	testingImpl.CreateSandboxFunc = func(ctx context.Context, sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.CreateSandboxFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(katautils.FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Force sandbox-type container
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeSandbox

	// Set a limit to ensure processCgroupsPath() considers the
	// cgroup part of the spec
	limit := int64(1024 * 1024)
	spec.Linux.Resources.Memory = &specs.LinuxMemory{
		Limit: &limit,
	}

	// Rewrite the file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	s := &service{
		id:         testSandboxID,
		containers: make(map[string]*container),
		config:     &runtimeConfig,
	}

	req := &taskAPI.CreateTaskRequest{
		ID:       testSandboxID,
		Bundle:   bundlePath,
		Terminal: true,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Create(ctx, req)
	assert.NoError(err)
}

func TestCreateSandboxFail(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledNeedRoot)
	}
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(katautils.FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	s := &service{
		id:         testSandboxID,
		containers: make(map[string]*container),
		config:     &runtimeConfig,
	}

	req := &taskAPI.CreateTaskRequest{
		ID:       testSandboxID,
		Bundle:   bundlePath,
		Terminal: true,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Create(ctx, req)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))
}

func TestCreateSandboxConfigFail(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledNeedRoot)
	}

	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(katautils.FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	quota := int64(0)
	limit := int64(0)

	spec.Linux.Resources.Memory = &specs.LinuxMemory{
		Limit: &limit,
	}

	// specify an invalid spec
	spec.Linux.Resources.CPU = &specs.LinuxCPU{
		Quota: &quota,
	}

	s := &service{
		id:         testSandboxID,
		containers: make(map[string]*container),
		config:     &runtimeConfig,
	}

	req := &taskAPI.CreateTaskRequest{
		ID:       testSandboxID,
		Bundle:   bundlePath,
		Terminal: true,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Create(ctx, req)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))
}

func TestCreateContainerSuccess(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	testingImpl.CreateContainerFunc = func(ctx context.Context, sandboxID string, containerConfig vc.ContainerConfig) (vc.VCSandbox, vc.VCContainer, error) {
		return sandbox, &vcmock.Container{}, nil
	}

	defer func() {
		testingImpl.CreateContainerFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(katautils.FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// set expected container type and sandboxID
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeContainer
	spec.Annotations[testSandboxIDAnnotation] = testSandboxID

	// rewrite file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	s := &service{
		id:         testContainerID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
		config:     &runtimeConfig,
	}

	req := &taskAPI.CreateTaskRequest{
		ID:       testContainerID,
		Bundle:   bundlePath,
		Terminal: true,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Create(ctx, req)
	assert.NoError(err)
}

func TestCreateContainerFail(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(katautils.FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeContainer
	spec.Annotations[testSandboxIDAnnotation] = testSandboxID

	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	// doesn't create sandbox first
	s := &service{
		id:         testContainerID,
		containers: make(map[string]*container),
		config:     &runtimeConfig,
	}

	req := &taskAPI.CreateTaskRequest{
		ID:       testContainerID,
		Bundle:   bundlePath,
		Terminal: true,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Create(ctx, req)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestCreateContainerConfigFail(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	testingImpl.CreateContainerFunc = func(ctx context.Context, sandboxID string, containerConfig vc.ContainerConfig) (vc.VCSandbox, vc.VCContainer, error) {
		return sandbox, &vcmock.Container{}, nil
	}

	defer func() {
		testingImpl.CreateContainerFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(katautils.FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// set the error containerType
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = "errorType"
	spec.Annotations[testSandboxIDAnnotation] = testSandboxID

	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	s := &service{
		id:         testContainerID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
		config:     &runtimeConfig,
	}

	req := &taskAPI.CreateTaskRequest{
		ID:       testContainerID,
		Bundle:   bundlePath,
		Terminal: true,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Create(ctx, req)
	assert.Error(err)
}
