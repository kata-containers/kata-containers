// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func testSandboxStateTransition(t *testing.T, state StateString, newState StateString) error {
	s := SandboxState{
		State: state,
	}

	return s.ValidTransition(state, newState)
}

func TestSandboxStateReadyRunning(t *testing.T) {
	err := testSandboxStateTransition(t, StateReady, StateRunning)
	assert.NoError(t, err)
}

func TestSandboxStateRunningPaused(t *testing.T) {
	err := testSandboxStateTransition(t, StateRunning, StatePaused)
	assert.NoError(t, err)
}

func TestSandboxStatePausedRunning(t *testing.T) {
	err := testSandboxStateTransition(t, StatePaused, StateRunning)
	assert.NoError(t, err)
}

func TestSandboxStatePausedStopped(t *testing.T) {
	err := testSandboxStateTransition(t, StatePaused, StateStopped)
	assert.NoError(t, err)
}

func TestSandboxStateRunningStopped(t *testing.T) {
	err := testSandboxStateTransition(t, StateRunning, StateStopped)
	assert.NoError(t, err)
}

func TestSandboxStateReadyStopped(t *testing.T) {
	err := testSandboxStateTransition(t, StateReady, StateStopped)
	assert.NoError(t, err)
}

func TestSandboxStateStoppedRunning(t *testing.T) {
	err := testSandboxStateTransition(t, StateStopped, StateRunning)
	assert.NoError(t, err)
}

func TestSandboxStateStoppedReady(t *testing.T) {
	err := testSandboxStateTransition(t, StateStopped, StateReady)
	assert.Error(t, err)
}

func testStateValid(t *testing.T, stateStr StateString, expected bool) {
	state := &SandboxState{
		State: stateStr,
	}

	ok := state.Valid()
	assert.Equal(t, ok, expected)
}

func TestStateValidSuccessful(t *testing.T) {
	testStateValid(t, StateReady, true)
	testStateValid(t, StateRunning, true)
	testStateValid(t, StatePaused, true)
	testStateValid(t, StateStopped, true)
}

func TestStateValidFailing(t *testing.T) {
	testStateValid(t, "", false)
}

func TestValidTransitionFailingOldStateMismatch(t *testing.T) {
	state := &SandboxState{
		State: StateReady,
	}

	err := state.ValidTransition(StateRunning, StateStopped)
	assert.Error(t, err)
}

func TestVolumesSetSuccessful(t *testing.T) {
	volumes := &Volumes{}

	volStr := "mountTag1:hostPath1 mountTag2:hostPath2"

	expected := Volumes{
		{
			MountTag: "mountTag1",
			HostPath: "hostPath1",
		},
		{
			MountTag: "mountTag2",
			HostPath: "hostPath2",
		},
	}

	err := volumes.Set(volStr)
	assert.NoError(t, err)
	assert.Exactly(t, *volumes, expected)
}

func TestVolumesSetFailingEmptyString(t *testing.T) {
	volumes := &Volumes{}

	volStr := ""

	err := volumes.Set(volStr)
	assert.Error(t, err)
}

func TestVolumesSetFailingTooFewArguments(t *testing.T) {
	volumes := &Volumes{}

	volStr := "mountTag1 mountTag2"

	err := volumes.Set(volStr)
	assert.Error(t, err)
}

func TestVolumesSetFailingTooManyArguments(t *testing.T) {
	volumes := &Volumes{}

	volStr := "mountTag1:hostPath1:Foo1 mountTag2:hostPath2:Foo2"

	err := volumes.Set(volStr)
	assert.Error(t, err)
}

func TestVolumesSetFailingVoidArguments(t *testing.T) {
	volumes := &Volumes{}

	volStr := ": : :"

	err := volumes.Set(volStr)
	assert.Error(t, err)
}

func TestVolumesStringSuccessful(t *testing.T) {
	volumes := &Volumes{
		{
			MountTag: "mountTag1",
			HostPath: "hostPath1",
		},
		{
			MountTag: "mountTag2",
			HostPath: "hostPath2",
		},
	}

	expected := "mountTag1:hostPath1 mountTag2:hostPath2"

	result := volumes.String()
	assert.Equal(t, result, expected)
}

func TestSocketsSetSuccessful(t *testing.T) {
	sockets := &Sockets{}

	sockStr := "devID1:id1:hostPath1:Name1 devID2:id2:hostPath2:Name2"

	expected := Sockets{
		{
			DeviceID: "devID1",
			ID:       "id1",
			HostPath: "hostPath1",
			Name:     "Name1",
		},
		{
			DeviceID: "devID2",
			ID:       "id2",
			HostPath: "hostPath2",
			Name:     "Name2",
		},
	}

	err := sockets.Set(sockStr)
	assert.NoError(t, err)
	assert.Exactly(t, *sockets, expected)
}

func TestSocketsSetFailingEmptyString(t *testing.T) {
	sockets := &Sockets{}

	sockStr := ""

	err := sockets.Set(sockStr)
	assert.Error(t, err)
}

func TestSocketsSetFailingWrongArgsAmount(t *testing.T) {
	sockets := &Sockets{}

	sockStr := "devID1:id1:hostPath1"

	err := sockets.Set(sockStr)
	assert.Error(t, err)
}

func TestSocketsSetFailingVoidArguments(t *testing.T) {
	sockets := &Sockets{}

	sockStr := ":::"

	err := sockets.Set(sockStr)
	assert.Error(t, err)
}

func TestSocketsStringSuccessful(t *testing.T) {
	sockets := &Sockets{
		{
			DeviceID: "devID1",
			ID:       "id1",
			HostPath: "hostPath1",
			Name:     "Name1",
		},
		{
			DeviceID: "devID2",
			ID:       "id2",
			HostPath: "hostPath2",
			Name:     "Name2",
		},
	}

	expected := "devID1:id1:hostPath1:Name1 devID2:id2:hostPath2:Name2"

	result := sockets.String()
	assert.Equal(t, result, expected)
}
