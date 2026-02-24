// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	"github.com/stretchr/testify/assert"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
)

func TestDeleteContainerSuccessAndFail(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	_, bundlePath, _ := ktu.SetupOCIConfigFile(t)

	_, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
	}

	reqCreate := &taskAPI.CreateTaskRequest{
		ID: testContainerID,
	}
	s.containers[testContainerID], err = newContainer(s, reqCreate, "", nil, true)
	assert.NoError(err)
}

func TestCleanupPodContainerDoesNotRemoveSharedSocket(t *testing.T) {
	assert := assert.New(t)

	_, bundlePath, ociConfigFile := ktu.SetupOCIConfigFile(t)
	spec, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	spec.Annotations = map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
		vcAnnotations.SandboxIDKey:     testSandboxID,
	}
	err = ktu.WriteOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	err = os.MkdirAll(filepath.Join(bundlePath, "rootfs"), testFileMode)
	assert.NoError(err)

	socketPath := filepath.Join(t.TempDir(), "shared.sock")
	err = os.WriteFile(socketPath, nil, testFileMode)
	assert.NoError(err)

	err = os.WriteFile(filepath.Join(bundlePath, "address"), []byte("unix://"+socketPath), testFileMode)
	assert.NoError(err)

	oldWd, err := os.Getwd()
	assert.NoError(err)
	defer func() {
		_ = os.Chdir(oldWd)
	}()
	err = os.Chdir(bundlePath)
	assert.NoError(err)

	testingImpl.CleanupContainerFunc = func(ctx context.Context, sandboxID, containerID string, force bool) error {
		return nil
	}
	defer func() {
		testingImpl.CleanupContainerFunc = nil
	}()

	s := &service{
		id:      testContainerID,
		rootCtx: context.Background(),
	}

	_, err = s.Cleanup(context.Background())
	assert.NoError(err)

	_, err = os.Stat(socketPath)
	assert.NoError(err)
}

func TestCleanupSandboxRemovesSocket(t *testing.T) {
	assert := assert.New(t)

	_, bundlePath, ociConfigFile := ktu.SetupOCIConfigFile(t)
	spec, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	spec.Annotations = map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
	}
	err = ktu.WriteOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	err = os.MkdirAll(filepath.Join(bundlePath, "rootfs"), testFileMode)
	assert.NoError(err)

	socketPath := filepath.Join(t.TempDir(), "sandbox.sock")
	err = os.WriteFile(socketPath, nil, testFileMode)
	assert.NoError(err)

	err = os.WriteFile(filepath.Join(bundlePath, "address"), []byte("unix://"+socketPath), testFileMode)
	assert.NoError(err)

	oldWd, err := os.Getwd()
	assert.NoError(err)
	defer func() {
		_ = os.Chdir(oldWd)
	}()
	err = os.Chdir(bundlePath)
	assert.NoError(err)

	testingImpl.CleanupContainerFunc = func(ctx context.Context, sandboxID, containerID string, force bool) error {
		return nil
	}
	defer func() {
		testingImpl.CleanupContainerFunc = nil
	}()

	s := &service{
		id:      testSandboxID,
		rootCtx: context.Background(),
	}

	_, err = s.Cleanup(context.Background())
	assert.NoError(err)

	_, err = os.Stat(socketPath)
	assert.True(os.IsNotExist(err))
}
