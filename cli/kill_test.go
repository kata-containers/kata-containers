// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"fmt"
	"os"
	"syscall"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
	"github.com/stretchr/testify/assert"
)

var (
	testKillContainerFuncReturnNil = func(sandboxID, containerID string, signal syscall.Signal, all bool) error {
		return nil
	}

	testStopContainerFuncReturnNil = func(sandboxID, containerID string) (vc.VCContainer, error) {
		return &vcmock.Container{}, nil
	}

	testStopSandboxFuncReturnNil = func(sandboxID string) (vc.VCSandbox, error) {
		return &vcmock.Sandbox{}, nil
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

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
	}

	testingImpl.KillContainerFunc = testKillContainerFuncReturnNil
	testingImpl.StopContainerFunc = testStopContainerFuncReturnNil

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.StopContainerFunc = nil
		testingImpl.StatusContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID, sig})

	execCLICommandFunc(assert, killCLICommand, set, false)

	annotations = map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
	}

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	testingImpl.StopContainerFunc = nil
	testingImpl.StopSandboxFunc = testStopSandboxFuncReturnNil
	defer func() {
		testingImpl.StopSandboxFunc = nil
	}()

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

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, map[string]string{}), nil
	}

	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.StatusContainerFunc = nil
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

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
	}

	testingImpl.KillContainerFunc = testKillContainerFuncReturnNil
	testingImpl.StopContainerFunc = testStopContainerFuncReturnNil

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.StopContainerFunc = nil
		testingImpl.StatusContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, killCLICommand, set, false)

	annotations = map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
	}

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	testingImpl.StopContainerFunc = nil
	testingImpl.StopSandboxFunc = testStopSandboxFuncReturnNil
	defer func() {
		testingImpl.StopSandboxFunc = nil
	}()

	execCLICommandFunc(assert, killCLICommand, set, false)
}

func TestKillCLIFunctionEnableAllSuccessful(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
	}

	testingImpl.KillContainerFunc = func(sandboxID, containerID string, signal syscall.Signal, all bool) error {
		if !all {
			return fmt.Errorf("Expecting -all flag = true, Got false")
		}

		return nil
	}
	testingImpl.StopContainerFunc = testStopContainerFuncReturnNil

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.StopContainerFunc = nil
		testingImpl.StatusContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Bool("all", true, "")
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, killCLICommand, set, false)

	annotations = map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
	}

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	testingImpl.StopContainerFunc = nil
	testingImpl.StopSandboxFunc = testStopSandboxFuncReturnNil
	defer func() {
		testingImpl.StopSandboxFunc = nil
	}()

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

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{}, nil
	}

	defer func() {
		testingImpl.KillContainerFunc = nil
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

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, map[string]string{}), nil
	}

	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.StatusContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID, "SIGINVALID"})

	execCLICommandFunc(assert, killCLICommand, set, true)
}

func TestKillCLIFunctionStatePausedSuccessful(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StatePaused,
	}

	testingImpl.KillContainerFunc = testKillContainerFuncReturnNil
	testingImpl.StopContainerFunc = testStopContainerFuncReturnNil

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state,
			map[string]string{string(vcAnnotations.ContainerTypeKey): string(vc.PodContainer)}), nil
	}

	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.StatusContainerFunc = nil
		testingImpl.StopContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, killCLICommand, set, false)
}

func TestKillCLIFunctionInvalidStateStoppedFailure(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateStopped,
	}

	testingImpl.KillContainerFunc = testKillContainerFuncReturnNil

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, map[string]string{}), nil
	}

	defer func() {
		testingImpl.KillContainerFunc = nil
		testingImpl.StatusContainerFunc = nil
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

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, map[string]string{}), nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	set.Parse([]string{testContainerID})

	execCLICommandFunc(assert, killCLICommand, set, true)
}
