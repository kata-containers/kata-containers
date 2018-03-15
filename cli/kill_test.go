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
	"fmt"
	"syscall"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcMock"
	"github.com/stretchr/testify/assert"
)

var (
	testKillContainerFuncReturnNil = func(podID, containerID string, signal syscall.Signal, all bool) error {
		return nil
	}

	testStopContainerFuncReturnNil = func(podID, containerID string) (vc.VCContainer, error) {
		return &vcMock.Container{}, nil
	}
)

func TestProcessSignal(t *testing.T) {
	tests := []struct {
		signal string
		valid  bool
		signum syscall.Signal
	}{
		{"SIGDCKBY", false, 0}, //invalid signal
		{"DCKBY", false, 0},    //invalid signal
		{"99999", false, 0},    //invalid signal
		{"SIGTERM", true, syscall.SIGTERM},
		{"TERM", true, syscall.SIGTERM},
		{"15", true, syscall.SIGTERM},
	}

	for _, test := range tests {
		signum, err := processSignal(test.signal)
		if signum != test.signum {
			t.Fatalf("signal received: %d expected signal: %d\n", signum, test.signum)
		}
		if test.valid && err != nil {
			t.Fatalf("signal %s is a valid but a error was received: %s\n", test.signal, err)
		}
		if !test.valid && err == nil {
			t.Fatalf("signal %s is not a valid signal and no error was reported\n", test.signal)
		}
	}
}

func testKillCLIFunctionTerminationSignalSuccessful(t *testing.T, sig string) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.KillContainerFunc = testKillContainerFuncReturnNil
	testingImpl.StopContainerFunc = testStopContainerFuncReturnNil
	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID, sig})

	execCLICommandFunc(assert, killCLICommand, set, false)
}

func TestKillCLIFunctionSigkillSuccessful(t *testing.T) {
	testKillCLIFunctionTerminationSignalSuccessful(t, "SIGKILL")
}

func TestKillCLIFunctionSigtermSuccessful(t *testing.T) {
	testKillCLIFunctionTerminationSignalSuccessful(t, "SIGTERM")
}

func TestKillCLIFunctionNotTerminationSignalSuccessful(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.KillContainerFunc = testKillContainerFuncReturnNil
	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID, "SIGUSR1"})

	execCLICommandFunc(assert, killCLICommand, set, false)
}

func TestKillCLIFunctionNoSignalSuccessful(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.KillContainerFunc = testKillContainerFuncReturnNil
	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, killCLICommand, set, false)
}

func TestKillCLIFunctionEnableAllSuccessful(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.KillContainerFunc = func(podID, containerID string, signal syscall.Signal, all bool) error {
		if !all {
			return fmt.Errorf("Expecting -all flag = true, Got false")
		}

		return nil
	}
	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Bool("all", true, "")
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, killCLICommand, set, false)
}

func TestKillCLIFunctionNoContainerIDFailure(t *testing.T) {
	assert := assert.New(t)

	set := flag.NewFlagSet("", 0)

	execCLICommandFunc(assert, killCLICommand, set, true)
}

func TestKillCLIFunctionContainerNotExistFailure(t *testing.T) {
	assert := assert.New(t)

	testingImpl.KillContainerFunc = testKillContainerFuncReturnNil
	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{}, nil
	}
	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, killCLICommand, set, true)
}

func TestKillCLIFunctionInvalidSignalFailure(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.KillContainerFunc = testKillContainerFuncReturnNil
	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID, "SIGINVALID"})

	execCLICommandFunc(assert, killCLICommand, set, true)
}

func TestKillCLIFunctionInvalidStatePausedFailure(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StatePaused,
	}

	testingImpl.KillContainerFunc = testKillContainerFuncReturnNil
	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, killCLICommand, set, true)
}

func TestKillCLIFunctionInvalidStateStoppedFailure(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateStopped,
	}

	testingImpl.KillContainerFunc = testKillContainerFuncReturnNil
	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, map[string]string{}), nil
	}
	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, killCLICommand, set, true)
}

func TestKillCLIFunctionKillContainerFailure(t *testing.T) {
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

	execCLICommandFunc(assert, killCLICommand, set, true)
}
