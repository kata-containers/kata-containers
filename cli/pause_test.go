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
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

var (
	testPauseContainerFuncReturnNil = func(ctx context.Context, sandboxID, containerID string) error {
		return nil
	}

	testResumeContainerFuncReturnNil = func(ctx context.Context, sandboxID, containerID string) error {
		return nil
	}
)

func TestPauseCLIFunctionSuccessful(t *testing.T) {
	assert := assert.New(t)

	state := types.State{
		State: types.StateRunning,
	}

	testingImpl.PauseContainerFunc = testPauseContainerFuncReturnNil

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, map[string]string{}), nil
	}

	defer func() {
		testingImpl.PauseContainerFunc = nil
		testingImpl.StatusContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, pauseCLICommand, set, false)
}

func TestPauseCLIFunctionContainerNotExistFailure(t *testing.T) {
	assert := assert.New(t)

	testingImpl.PauseContainerFunc = testPauseContainerFuncReturnNil

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	defer func() {
		testingImpl.PauseContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, pauseCLICommand, set, true)
}

func TestPauseCLIFunctionPauseContainerFailure(t *testing.T) {
	assert := assert.New(t)

	state := types.State{
		State: types.StateRunning,
	}

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, map[string]string{}), nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, pauseCLICommand, set, true)
}

func TestResumeCLIFunctionSuccessful(t *testing.T) {
	assert := assert.New(t)

	state := types.State{
		State: types.StateRunning,
	}

	testingImpl.ResumeContainerFunc = testResumeContainerFuncReturnNil

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, map[string]string{}), nil
	}

	defer func() {
		testingImpl.ResumeContainerFunc = nil
		testingImpl.StatusContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, resumeCLICommand, set, false)
}

func TestResumeCLIFunctionContainerNotExistFailure(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ResumeContainerFunc = testResumeContainerFuncReturnNil

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	defer func() {
		testingImpl.ResumeContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, resumeCLICommand, set, true)
}

func TestResumeCLIFunctionPauseContainerFailure(t *testing.T) {
	assert := assert.New(t)

	state := types.State{
		State: types.StateRunning,
	}

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, map[string]string{}), nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, resumeCLICommand, set, true)
}
