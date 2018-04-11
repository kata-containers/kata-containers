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
