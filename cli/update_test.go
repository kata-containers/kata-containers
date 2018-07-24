// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"io/ioutil"
	"os"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func TestUpdateCLIAction(t *testing.T) {
	assert := assert.New(t)

	flagSet := flag.NewFlagSet("update", flag.ContinueOnError)
	flagSet.Parse([]string{"resources"})

	// create a new fake context
	ctx := cli.NewContext(&cli.App{}, flagSet, nil)

	// get Action function
	actionFunc, ok := updateCLICommand.Action.(func(ctx *cli.Context) error)
	assert.True(ok)

	err := actionFunc(ctx)
	assert.Error(err, "Missing container ID")
}

func TestUpdateCLIFailure(t *testing.T) {
	assert := assert.New(t)

	flagSet := flag.NewFlagSet("update", flag.ContinueOnError)
	ctx := cli.NewContext(&cli.App{}, flagSet, nil)

	actionFunc, ok := updateCLICommand.Action.(func(ctx *cli.Context) error)
	assert.True(ok)

	// missing container ID
	err := actionFunc(ctx)
	assert.Error(err)

	// container info
	flagSet.Parse([]string{testContainerID})
	ctx = cli.NewContext(&cli.App{}, flagSet, nil)
	err = actionFunc(ctx)
	assert.Error(err)

	// not running
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
	err = actionFunc(ctx)
	assert.Error(err)

	// resources file does not exist
	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
			},
			State: vc.State{
				State: vc.StateRunning,
			},
		}, nil
	}
	testingImpl.UpdateContainerFunc = func(sandboxID, containerID string, resources specs.LinuxResources) error {
		return nil
	}
	defer func() {
		testingImpl.UpdateContainerFunc = nil
	}()
	flagSet.String("resources", "/abc/123/xyz/rgb", "")
	ctx = cli.NewContext(&cli.App{}, flagSet, nil)
	err = actionFunc(ctx)
	assert.Error(err)

	// json decode error
	f, err := ioutil.TempFile("", "resources")
	defer os.Remove(f.Name())
	assert.NoError(err)
	assert.NotNil(f)
	f.WriteString("no json")
	f.Close()
	flagSet.Set("resources", f.Name())
	ctx = cli.NewContext(&cli.App{}, flagSet, nil)
	err = actionFunc(ctx)
	assert.Error(err)

	// ParseUint Error
	flagSet = flag.NewFlagSet("update", flag.ContinueOnError)
	flagSet.Parse([]string{testContainerID})
	flagSet.String("cpu-period", "abcxyz", "")
	ctx = cli.NewContext(&cli.App{}, flagSet, nil)
	err = actionFunc(ctx)
	assert.Error(err)

	// ParseInt Error
	flagSet = flag.NewFlagSet("update", flag.ContinueOnError)
	flagSet.Parse([]string{testContainerID})
	flagSet.String("cpu-quota", "abcxyz", "")
	ctx = cli.NewContext(&cli.App{}, flagSet, nil)
	err = actionFunc(ctx)
	assert.Error(err)

	// RAMInBytes Error
	flagSet = flag.NewFlagSet("update", flag.ContinueOnError)
	flagSet.Parse([]string{testContainerID})
	flagSet.String("memory", "abcxyz", "")
	ctx = cli.NewContext(&cli.App{}, flagSet, nil)
	err = actionFunc(ctx)
	assert.Error(err)
}

func TestUpdateCLISuccessful(t *testing.T) {
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

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
			},
			State: vc.State{
				State: vc.StateRunning,
			},
		}, nil
	}
	testingImpl.UpdateContainerFunc = func(sandboxID, containerID string, resources specs.LinuxResources) error {
		return nil
	}
	defer func() {
		testingImpl.StatusContainerFunc = nil
		testingImpl.UpdateContainerFunc = nil
	}()

	path, err := createTempContainerIDMapping(sandbox.ID(), sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)
	actionFunc, ok := updateCLICommand.Action.(func(ctx *cli.Context) error)
	assert.True(ok)

	flagSet := flag.NewFlagSet("update", flag.ContinueOnError)
	flagSet.Parse([]string{testContainerID})
	flagSet.Int("blkio-weight", 20, "")
	flagSet.String("cpuset-cpus", "0-5", "")
	flagSet.String("cpuset-mems", "0-5", "")
	flagSet.String("cpu-period", "1000", "")
	flagSet.String("cpu-rt-period", "1000", "")
	flagSet.String("cpu-share", "1000", "")
	flagSet.String("cpu-quota", "1000", "")
	flagSet.String("cpu-rt-runtime", "1000", "")
	flagSet.String("memory", "100M", "")
	flagSet.String("memory-swap", "100M", "")
	flagSet.String("kernel-memory", "100M", "")
	flagSet.String("kernel-memory-tcp", "100M", "")
	flagSet.String("memory-reservation", "100M", "")
	ctx := cli.NewContext(&cli.App{}, flagSet, nil)
	err = actionFunc(ctx)
	assert.NoError(err)
}
