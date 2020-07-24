// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"fmt"
	"testing"

	"github.com/stretchr/testify/assert"
)

func testContainerStateTransition(t *testing.T, state StateString, newState StateString) error {
	s := ContainerState{
		State: state,
	}

	return s.ValidTransition(state, newState)
}

func TestContainerStateReadyRunning(t *testing.T) {
	err := testContainerStateTransition(t, StateReady, StateRunning)
	assert.NoError(t, err)
}

func TestContainerStateRunningPaused(t *testing.T) {
	err := testContainerStateTransition(t, StateRunning, StatePaused)
	assert.NoError(t, err)
}

func TestContainerStatePausedRunning(t *testing.T) {
	err := testContainerStateTransition(t, StatePaused, StateRunning)
	assert.NoError(t, err)
}

func TestContainerStatePausedStopped(t *testing.T) {
	err := testContainerStateTransition(t, StatePaused, StateStopped)
	assert.NoError(t, err)
}

func TestContainerStateRunningStopped(t *testing.T) {
	err := testContainerStateTransition(t, StateRunning, StateStopped)
	assert.NoError(t, err)
}

func TestContainerStateReadyStopped(t *testing.T) {
	err := testContainerStateTransition(t, StateReady, StateStopped)
	assert.NoError(t, err)
}

func TestContainerStateStoppedRunning(t *testing.T) {
	err := testContainerStateTransition(t, StateStopped, StateRunning)
	assert.NoError(t, err)
}

func TestContainerStateStoppedReady(t *testing.T) {
	err := testContainerStateTransition(t, StateStopped, StateReady)
	assert.Error(t, err)
}

func TestContainerStatePausedPaused(t *testing.T) {
	err := testContainerStateTransition(t, StatePaused, StatePaused)
	assert.Error(t, err)
}

func testContainerStateValid(t *testing.T, stateStr StateString, expected bool) {
	state := &ContainerState{
		State: stateStr,
	}

	ok := state.Valid()
	msg := fmt.Sprintf("state: %+v, expected: %v", stateStr, expected)
	assert.Equal(t, ok, expected, msg)
}

func TestContainerStateValidSuccessful(t *testing.T) {
	testContainerStateValid(t, StateReady, true)
	testContainerStateValid(t, StateRunning, true)
	testContainerStateValid(t, StatePaused, true)
	testContainerStateValid(t, StateStopped, true)
}

func TestContainerStateValidFailing(t *testing.T) {
	testContainerStateValid(t, "", false)
}
