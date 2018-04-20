// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"os"
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

	path, err := createTempContainerIDMapping(sandbox.ID(), sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
			},
		}, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	// inexistent container
	err = ps("xyz123abc", "json", []string{"-ef"})
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

	path, err := createTempContainerIDMapping(sandbox.ID(), sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			State: vc.State{
				State: vc.StateRunning,
			},
			ID: sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
			},
		}, nil
	}

	testingImpl.ProcessListContainerFunc = func(sandboxID, containerID string, options vc.ProcessListOptions) (vc.ProcessList, error) {
		return []byte("echo,sleep,grep"), nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
		testingImpl.ProcessListContainerFunc = nil
	}()

	err = ps(sandbox.ID(), "json", []string{})
	assert.NoError(err)
}
