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

func TestStateCliAction(t *testing.T) {
	assert := assert.New(t)

	actionFunc, ok := stateCLICommand.Action.(func(ctx *cli.Context) error)
	assert.True(ok)

	flagSet := flag.NewFlagSet("flag", flag.ContinueOnError)

	// without container id
	flagSet.Parse([]string{"runtime"})
	ctx := cli.NewContext(&cli.App{}, flagSet, nil)
	err := actionFunc(ctx)
	assert.Error(err)

	// with container id
	flagSet.Parse([]string{"runtime", testContainerID})
	ctx = cli.NewContext(&cli.App{}, flagSet, nil)
	err = actionFunc(ctx)
	assert.Error(err)
}

func TestStateSuccessful(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testContainerID,
	}

	pod.MockContainers = []*vcMock.Container{
		{
			MockID:  pod.ID(),
			MockPod: pod,
		},
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// return a podStatus with the container status
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: pod.ID(),
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

	// trying with an inexistent id
	err := state("123456789")
	assert.Error(err)

	err = state(pod.ID())
	assert.NoError(err)
}
