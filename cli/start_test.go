// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"encoding/json"
	"flag"
	"io/ioutil"
	"os"
	"testing"

	"github.com/kata-containers/runtime/pkg/katautils"
	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func TestStartInvalidArgs(t *testing.T) {
	assert := assert.New(t)

	// Missing container id
	_, err := start(context.Background(), "")
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	// Mock StatusContainer error
	_, err = start(context.Background(), testContainerID)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	path, err = ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	katautils.SetCtrsMapTreePath(path)

	// Container missing in container mapping
	_, err = start(context.Background(), testContainerID)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestStartSandbox(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	path, err := createTempContainerIDMapping(sandbox.ID(), sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	ociSpecJSON, err := json.Marshal(oci.CompatOCISpec{})
	assert.NoError(err)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
				vcAnnotations.ConfigJSONKey:    string(ociSpecJSON),
			},
		}, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	_, err = start(context.Background(), sandbox.ID())
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.StartSandboxFunc = func(ctx context.Context, sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.StartSandboxFunc = nil
	}()

	_, err = start(context.Background(), sandbox.ID())
	assert.Nil(err)
}

func TestStartMissingAnnotation(t *testing.T) {
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

	_, err = start(context.Background(), sandbox.ID())
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestStartContainerSucessFailure(t *testing.T) {
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

	path, err := createTempContainerIDMapping(testContainerID, sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	ociSpecJSON, err := json.Marshal(oci.CompatOCISpec{})
	assert.NoError(err)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: testContainerID,
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
				vcAnnotations.ConfigJSONKey:    string(ociSpecJSON),
			},
		}, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	_, err = start(context.Background(), testContainerID)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	testingImpl.StartContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.VCContainer, error) {
		return sandbox.MockContainers[0], nil
	}

	defer func() {
		testingImpl.StartContainerFunc = nil
	}()

	_, err = start(context.Background(), testContainerID)
	assert.Nil(err)
}

func TestStartCLIFunction(t *testing.T) {
	assert := assert.New(t)

	flagSet := &flag.FlagSet{}
	ctx := createCLIContext(flagSet)

	fn, ok := startCLICommand.Action.(func(context *cli.Context) error)
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

func TestStartCLIFunctionSuccess(t *testing.T) {
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

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	ociSpecJSON, err := json.Marshal(oci.CompatOCISpec{})
	assert.NoError(err)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: testContainerID,
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
				vcAnnotations.ConfigJSONKey:    string(ociSpecJSON),
			},
		}, nil
	}

	testingImpl.StartContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.VCContainer, error) {
		return sandbox.MockContainers[0], nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
		testingImpl.StartContainerFunc = nil
	}()

	fn, ok := startCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	flagSet := flag.NewFlagSet("test", 0)
	flagSet.Parse([]string{testContainerID})
	ctx := createCLIContext(flagSet)
	assert.NotNil(ctx)

	err = fn(ctx)
	assert.NoError(err)
}
