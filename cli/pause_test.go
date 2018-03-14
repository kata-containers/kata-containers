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
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcMock"
	"github.com/stretchr/testify/assert"
)

var (
	testPausePodFuncReturnNil = func(podID string) (vc.VCPod, error) {
		return &vcMock.Pod{}, nil
	}

	testResumePodFuncReturnNil = func(podID string) (vc.VCPod, error) {
		return &vcMock.Pod{}, nil
	}
)

func TestPauseCLIFunctionSuccessful(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.PausePodFunc = testPausePodFuncReturnNil
	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.PausePodFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, pauseCLICommand, set, false)
}

func TestPauseCLIFunctionContainerNotExistFailure(t *testing.T) {
	assert := assert.New(t)

	testingImpl.PausePodFunc = testPausePodFuncReturnNil
	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{}, nil
	}
	defer func() {
		testingImpl.PausePodFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, pauseCLICommand, set, true)
}

func TestPauseCLIFunctionPausePodFailure(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.ListPodFunc = nil
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

	testingImpl.ResumePodFunc = testResumePodFuncReturnNil
	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.ResumePodFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, resumeCLICommand, set, false)
}

func TestResumeCLIFunctionContainerNotExistFailure(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ResumePodFunc = testResumePodFuncReturnNil
	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{}, nil
	}
	defer func() {
		testingImpl.ResumePodFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, resumeCLICommand, set, true)
}

func TestResumeCLIFunctionPausePodFailure(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, resumeCLICommand, set, true)
}
