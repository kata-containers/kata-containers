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
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcMock"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func TestStartInvalidArgs(t *testing.T) {
	assert := assert.New(t)

	// Missing container id
	_, err := start("")
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))

	// Mock Listpod error
	_, err = start(testContainerID)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	// Container missing in ListPod
	_, err = start(testContainerID)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestStartPod(t *testing.T) {
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

	_, err := start(pod.ID())
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	testingImpl.StartPodFunc = func(podID string) (vc.VCPod, error) {
		return pod, nil
	}

	defer func() {
		testingImpl.StartPodFunc = nil
	}()

	_, err = start(pod.ID())
	assert.Nil(err)
}

func TestStartMissingAnnotation(t *testing.T) {
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

	_, err := start(pod.ID())
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestStartContainerSucessFailure(t *testing.T) {
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

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: testContainerID,
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
						},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	_, err := start(testContainerID)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	testingImpl.StartContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		return pod.MockContainers[0], nil
	}

	defer func() {
		testingImpl.StartContainerFunc = nil
	}()

	_, err = start(testContainerID)
	assert.Nil(err)
}

func TestStartCLIFunction(t *testing.T) {
	assert := assert.New(t)

	flagSet := &flag.FlagSet{}
	app := cli.NewApp()

	ctx := cli.NewContext(app, flagSet, nil)

	fn, ok := startCLICommand.Action.(func(context *cli.Context) error)
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

func TestStartCLIFunctionSuccess(t *testing.T) {
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

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: testContainerID,
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
						},
					},
				},
			},
		}, nil
	}

	testingImpl.StartContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		return pod.MockContainers[0], nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
		testingImpl.StartContainerFunc = nil
	}()

	app := cli.NewApp()

	fn, ok := startCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	flagSet := flag.NewFlagSet("test", 0)
	flagSet.Parse([]string{testContainerID})
	ctx := cli.NewContext(app, flagSet, nil)
	assert.NotNil(ctx)

	err := fn(ctx)
	assert.NoError(err)
}
