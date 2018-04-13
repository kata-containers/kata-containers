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
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func TestPSCLIAction(t *testing.T) {
	assert := assert.New(t)

	flagSet := flag.NewFlagSet("flag", flag.ContinueOnError)
	flagSet.Parse([]string{"runtime"})

	// create a new fake context
	ctx := cli.NewContext(&cli.App{Metadata: map[string]interface{}{}}, flagSet, nil)

	// get Action function
	actionFunc, ok := psCLICommand.Action.(func(ctx *cli.Context) error)
	assert.True(ok)

	err := actionFunc(ctx)
	assert.Error(err, "Missing container ID")
}

func TestPSFailure(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testContainerID,
	}

	sandbox.MockContainers = []*vcmock.Container{
		{
			MockID:      sandbox.ID(),
			MockSandbox: sandbox,
		},
	}

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		// return a sandboxStatus with the container status
		return []vc.SandboxStatus{
			{
				ID: sandbox.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: sandbox.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
						},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListSandboxFunc = nil
	}()

	// inexistent container
	err := ps("xyz123abc", "json", []string{"-ef"})
	assert.Error(err)

	// container is not running
	err = ps(sandbox.ID(), "json", []string{"-ef"})
	assert.Error(err)
}

func TestPSSuccessful(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testContainerID,
	}

	sandbox.MockContainers = []*vcmock.Container{
		{
			MockID:      sandbox.ID(),
			MockSandbox: sandbox,
		},
	}

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		// return a sandboxStatus with the container status
		return []vc.SandboxStatus{
			{
				ID: sandbox.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						State: vc.State{
							State: vc.StateRunning,
						},
						ID: sandbox.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
						},
					},
				},
			},
		}, nil
	}

	testingImpl.ProcessListContainerFunc = func(sandboxID, containerID string, options vc.ProcessListOptions) (vc.ProcessList, error) {
		return []byte("echo,sleep,grep"), nil
	}

	defer func() {
		testingImpl.ListSandboxFunc = nil
		testingImpl.ProcessListContainerFunc = nil
	}()

	err := ps(sandbox.ID(), "json", []string{})
	assert.NoError(err)
}
