// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"testing"
	"time"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	"github.com/containerd/containerd/api/types/task"
	"github.com/stretchr/testify/assert"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
)

func TestDeleteContainerSuccessAndFail(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	_, bundlePath, _ := ktu.SetupOCIConfigFile(t)

	_, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
	}

	reqCreate := &taskAPI.CreateTaskRequest{
		ID: testContainerID,
	}
	s.containers[testContainerID], err = newContainer(s, reqCreate, "", nil, true)
	assert.NoError(err)
}

// newDeleteTestContainer builds a service and container the delete tests can
// use, with vcmock hooks recording calls into the sandbox.
func newDeleteTestContainer(t *testing.T, cType vc.ContainerType, status task.Status) (*service, *container, *[]string) {
	t.Helper()

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	var calls []string
	sandbox.StopContainerFunc = func(contID string, force bool) (vc.VCContainer, error) {
		calls = append(calls, "StopContainer")
		return &vcmock.Container{}, nil
	}
	sandbox.DeleteContainerFunc = func(contID string) (vc.VCContainer, error) {
		calls = append(calls, "DeleteContainer")
		return &vcmock.Container{}, nil
	}

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
	}

	reqCreate := &taskAPI.CreateTaskRequest{
		ID: testContainerID,
	}
	c, err := newContainer(s, reqCreate, cType, nil, false)
	assert.NoError(t, err)
	c.status = status
	s.containers[testContainerID] = c

	return s, c, &calls
}

// TestDeleteContainerWaitsForTeardown verifies that deleteContainer waits
// for the post-exit teardown started by wait() (signalled by closing
// c.teardownDone) before calling into virtcontainers, and that it does not
// stop the container again once the shim-level status is already STOPPED.
func TestDeleteContainerWaitsForTeardown(t *testing.T) {
	assert := assert.New(t)

	s, c, calls := newDeleteTestContainer(t, vc.PodContainer, task.Status_STOPPED)

	done := make(chan error, 1)
	ctx := context.Background()
	go func() {
		done <- deleteContainer(ctx, s, c)
	}()

	// While the teardown is in flight (teardownDone still open), neither
	// DeleteContainer nor the return may happen.
	select {
	case err := <-done:
		t.Fatalf("deleteContainer returned before the teardown finished: %v", err)
	case <-time.After(100 * time.Millisecond):
	}

	// Finish the teardown: deleteContainer must proceed and call
	// DeleteContainer afterwards.
	close(c.teardownDone)

	select {
	case err := <-done:
		assert.NoError(err)
	case <-time.After(5 * time.Second):
		t.Fatal("deleteContainer did not return after the teardown finished")
	}

	assert.ElementsMatch([]string{"DeleteContainer"}, *calls)
	assert.NotContains(s.containers, testContainerID)
}

// TestDeleteContainerTeardownTimeout verifies that deleteContainer does not
// block forever when the teardown never finishes: after
// teardownWaitTimeout it proceeds as before the fix (DeleteContainer runs
// anyway and the isNotFound tolerance decides the outcome).
func TestDeleteContainerTeardownTimeout(t *testing.T) {
	assert := assert.New(t)

	s, c, calls := newDeleteTestContainer(t, vc.PodContainer, task.Status_STOPPED)

	oldTimeout := teardownWaitTimeout
	teardownWaitTimeout = 50 * time.Millisecond
	defer func() {
		teardownWaitTimeout = oldTimeout
	}()

	// teardownDone is never closed.
	err := deleteContainer(context.Background(), s, c)
	assert.NoError(err)

	assert.ElementsMatch([]string{"DeleteContainer"}, *calls)
	assert.NotContains(s.containers, testContainerID)
}

func TestDeleteContainerScenarios(t *testing.T) {
	type testData struct {
		name string

		cType        vc.ContainerType
		status       task.Status
		teardownDone bool // close c.teardownDone before the call

		expectCalls []string // expected vcmock calls, in any order
	}

	data := []testData{
		{
			// The shim-level status is still RUNNING, so deleteContainer
			// must stop the container first; it never waits on
			// teardownDone (which stays open).
			name:         "running container stops first",
			cType:        vc.PodContainer,
			status:       task.Status_RUNNING,
			teardownDone: false,
			expectCalls:  []string{"StopContainer", "DeleteContainer"},
		},
		{
			// Teardown already finished: no stop, no wait.
			name:         "stopped container with finished teardown",
			cType:        vc.PodContainer,
			status:       task.Status_STOPPED,
			teardownDone: true,
			expectCalls:  []string{"DeleteContainer"},
		},
		{
			// The sandbox container never goes through virtcontainers in
			// deleteContainer, so an open teardownDone channel (the whole
			// sandbox teardown still in flight) must not block its delete.
			name:         "sandbox container skips virtcontainers entirely",
			cType:        vc.PodSandbox,
			status:       task.Status_STOPPED,
			teardownDone: false,
			expectCalls:  nil,
		},
	}

	for _, d := range data {
		t.Run(d.name, func(t *testing.T) {
			assert := assert.New(t)

			s, c, calls := newDeleteTestContainer(t, d.cType, d.status)
			if d.teardownDone {
				close(c.teardownDone)
			}

			// Not table-driven timing: give every scenario a generous
			// bound so a regression to an unbounded wait fails the test
			// instead of hanging it.
			done := make(chan error, 1)
			go func() {
				done <- deleteContainer(context.Background(), s, c)
			}()

			select {
			case err := <-done:
				assert.NoError(err)
			case <-time.After(5 * time.Second):
				t.Fatal("deleteContainer did not return")
			}

			assert.ElementsMatch(d.expectCalls, *calls)
			assert.NotContains(s.containers, testContainerID)
		})
	}
}

// TestDeleteContainerDoesNotRaceTeardown reproduces the Delete/teardown
// race end to end through the real wait() and deleteContainer, coordinating
// only via the vcmock hooks.  It deliberately does not reference the fix's
// teardownDone channel so that it also compiles against pre-fix code, where
// it fails: the Delete issued right after the early exit publication reaches
// DeleteContainer while the teardown's StopContainer is still in flight.
func TestDeleteContainerDoesNotRaceTeardown(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	// The teardown's StopContainer blocks until released, standing in for
	// the in-flight agent RPC of a real guest stop.
	entered := make(chan struct{})
	release := make(chan struct{})
	sandbox.StopContainerFunc = func(contID string, force bool) (vc.VCContainer, error) {
		close(entered)
		<-release
		return &vcmock.Container{}, nil
	}

	// Records when the delete reached virtcontainers.
	deleted := make(chan struct{})
	sandbox.DeleteContainerFunc = func(contID string) (vc.VCContainer, error) {
		close(deleted)
		return &vcmock.Container{}, nil
	}

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
		// ec is buffered, so the cReap goroutine spawned by wait() does
		// not leak on an undrained channel.
		ec: make(chan exit, bufferSize),
	}

	reqCreate := &taskAPI.CreateTaskRequest{
		ID: testContainerID,
	}
	c, err := newContainer(s, reqCreate, vc.PodContainer, nil, false)
	assert.NoError(err)
	s.containers[testContainerID] = c

	// Run the real wait(): it sets the shim-level status to STOPPED,
	// publishes the exit, then runs the teardown (blocked above).
	close(c.exitIOch)
	waitReturned := make(chan struct{})
	go func() {
		_, _ = wait(context.Background(), s, c, "")
		close(waitReturned)
	}()

	// A consumer (CRI-O, containerd's CRI plugin) issues the Delete as
	// soon as the exit is published.
	<-c.exitCh

	deleteDone := make(chan error, 1)
	go func() {
		deleteDone <- deleteContainer(context.Background(), s, c)
	}()

	// Once the teardown is observed in flight, the delete must not have
	// reached virtcontainers.
	<-entered
	select {
	case <-deleted:
		t.Fatal("DeleteContainer was called while the teardown was still in flight")
	case <-time.After(100 * time.Millisecond):
	}

	// Let the teardown finish; the delete must complete afterwards.
	close(release)
	<-waitReturned

	select {
	case <-deleted:
	case <-time.After(5 * time.Second):
		t.Fatal("DeleteContainer was not called after the teardown finished")
	}

	select {
	case err := <-deleteDone:
		assert.NoError(err)
	case <-time.After(5 * time.Second):
		t.Fatal("deleteContainer did not return after the teardown finished")
	}

	assert.NotContains(s.containers, testContainerID)
}
