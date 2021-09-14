// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"testing"

	"github.com/containerd/containerd/namespaces"
	taskAPI "github.com/containerd/containerd/runtime/v2/task"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"

	"github.com/stretchr/testify/assert"
)

func TestStartStartSandboxSuccess(t *testing.T) {
	assert := assert.New(t)
	var err error

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	sandbox.StatusContainerFunc = func(contID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
			},
		}, nil
	}

	defer func() {
		sandbox.StatusContainerFunc = nil
	}()

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
		ctx:        namespaces.WithNamespace(context.Background(), "UnitTest"),
	}

	reqCreate := &taskAPI.CreateTaskRequest{
		ID: testSandboxID,
	}
	s.containers[testSandboxID], err = newContainer(s, reqCreate, vc.PodSandbox, nil, false)
	assert.NoError(err)

	reqStart := &taskAPI.StartRequest{
		ID: testSandboxID,
	}

	sandbox.StartFunc = func() error {
		return nil
	}

	defer func() {
		sandbox.StartFunc = nil
	}()

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Start(ctx, reqStart)
	assert.NoError(err)
}

func TestStartMissingAnnotation(t *testing.T) {
	assert := assert.New(t)
	var err error

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	sandbox.StatusContainerFunc = func(contID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID:          sandbox.ID(),
			Annotations: map[string]string{},
		}, nil
	}

	defer func() {
		sandbox.StatusContainerFunc = nil
	}()

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
		ctx:        namespaces.WithNamespace(context.Background(), "UnitTest"),
	}

	reqCreate := &taskAPI.CreateTaskRequest{
		ID: testSandboxID,
	}
	s.containers[testSandboxID], err = newContainer(s, reqCreate, "", nil, false)
	assert.NoError(err)

	reqStart := &taskAPI.StartRequest{
		ID: testSandboxID,
	}

	sandbox.StartFunc = func() error {
		return nil
	}

	defer func() {
		sandbox.StartFunc = nil
	}()

	_, err = s.Start(s.ctx, reqStart)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestStartStartContainerSucess(t *testing.T) {
	assert := assert.New(t)
	var err error

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	sandbox.MockContainers = []*vcmock.Container{
		{
			MockID:      testContainerID,
			MockSandbox: sandbox,
		},
	}

	sandbox.StatusContainerFunc = func(contID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: testContainerID,
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
			},
		}, nil
	}

	defer func() {
		sandbox.StatusContainerFunc = nil
	}()

	sandbox.StartContainerFunc = func(contID string) (vc.VCContainer, error) {
		return sandbox.MockContainers[0], nil
	}

	defer func() {
		sandbox.StartContainerFunc = nil
	}()

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
		ctx:        namespaces.WithNamespace(context.Background(), "UnitTest"),
	}

	reqCreate := &taskAPI.CreateTaskRequest{
		ID: testContainerID,
	}
	s.containers[testContainerID], err = newContainer(s, reqCreate, vc.PodContainer, nil, false)
	assert.NoError(err)

	reqStart := &taskAPI.StartRequest{
		ID: testContainerID,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Start(ctx, reqStart)
	assert.NoError(err)
}
