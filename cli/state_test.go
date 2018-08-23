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
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
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
	ctx := createCLIContext(flagSet)
	err := actionFunc(ctx)
	assert.Error(err)

	// with container id
	flagSet.Parse([]string{"runtime", testContainerID})
	ctx = createCLIContext(flagSet)
	err = actionFunc(ctx)
	assert.Error(err)
}

func TestStateSuccessful(t *testing.T) {
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

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	// trying with an inexistent id
	err = state(context.Background(), "123456789")
	assert.Error(err)

	path, err = createTempContainerIDMapping(testContainerID, sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: testContainerID,
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
			},
		}, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	err = state(context.Background(), testContainerID)
	assert.NoError(err)
}
