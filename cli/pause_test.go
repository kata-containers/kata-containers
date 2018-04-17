// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
	"github.com/stretchr/testify/assert"
)

var (
	testPauseSandboxFuncReturnNil = func(sandboxID string) (vc.VCSandbox, error) {
		return &vcmock.Sandbox{}, nil
	}

	testResumeSandboxFuncReturnNil = func(sandboxID string) (vc.VCSandbox, error) {
		return &vcmock.Sandbox{}, nil
	}
)

func TestPauseCLIFunctionSuccessful(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.PauseSandboxFunc = testPauseSandboxFuncReturnNil
	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return newSingleContainerSandboxStatusList(testSandboxID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.PauseSandboxFunc = nil
		testingImpl.ListSandboxFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, pauseCLICommand, set, false)
}

func TestPauseCLIFunctionContainerNotExistFailure(t *testing.T) {
	assert := assert.New(t)

	testingImpl.PauseSandboxFunc = testPauseSandboxFuncReturnNil
	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{}, nil
	}
	defer func() {
		testingImpl.PauseSandboxFunc = nil
		testingImpl.ListSandboxFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, pauseCLICommand, set, true)
}

func TestPauseCLIFunctionPauseSandboxFailure(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return newSingleContainerSandboxStatusList(testSandboxID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.ListSandboxFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, pauseCLICommand, set, true)
}

func TestResumeCLIFunctionSuccessful(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.ResumeSandboxFunc = testResumeSandboxFuncReturnNil
	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return newSingleContainerSandboxStatusList(testSandboxID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.ResumeSandboxFunc = nil
		testingImpl.ListSandboxFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, resumeCLICommand, set, false)
}

func TestResumeCLIFunctionContainerNotExistFailure(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ResumeSandboxFunc = testResumeSandboxFuncReturnNil
	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{}, nil
	}
	defer func() {
		testingImpl.ResumeSandboxFunc = nil
		testingImpl.ListSandboxFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, resumeCLICommand, set, true)
}

func TestResumeCLIFunctionPauseSandboxFailure(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return newSingleContainerSandboxStatusList(testSandboxID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.ListSandboxFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, resumeCLICommand, set, true)
}
