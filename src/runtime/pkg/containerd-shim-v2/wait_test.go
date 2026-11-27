// Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"errors"
	"syscall"
	"testing"
	"time"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	"github.com/containerd/containerd/api/types/task"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
	specs "github.com/opencontainers/runtime-spec/specs-go"
)

func assertServiceMutexAvailable(t *testing.T, s *service) {
	t.Helper()

	acquired := make(chan struct{})
	go func() {
		s.mu.Lock()
		close(acquired)
		s.mu.Unlock()
	}()

	select {
	case <-acquired:
	case <-time.After(time.Second):
		t.Fatal("service mutex remained locked during sandbox teardown")
	}
}

func TestWaitDoesNotHoldServiceMutexDuringContainerTeardown(t *testing.T) {
	teardownStarted := make(chan struct{})
	releaseTeardown := make(chan struct{})

	sandbox := &vcmock.Sandbox{
		MockID: "sandbox",
		WaitProcessFunc: func(containerID, processID string) (int32, error) {
			return 0, nil
		},
		StopContainerFunc: func(contID string, force bool) (vc.VCContainer, error) {
			close(teardownStarted)
			<-releaseTeardown
			return &vcmock.Container{}, nil
		},
	}

	s, err := newTestService("sandbox")
	if err != nil {
		t.Fatal(err)
	}
	defer s.cancel()
	s.sandbox = sandbox

	exitIOch := make(chan struct{})
	close(exitIOch)
	c := &container{
		id:       "container",
		cType:    vc.PodContainer,
		status:   task.Status_RUNNING,
		exitIOch: exitIOch,
		exitCh:   make(chan uint32, 1),
	}
	s.containers[c.id] = c

	waitDone := make(chan error, 1)
	go func() {
		_, err := wait(context.Background(), s, c, "")
		waitDone <- err
	}()

	select {
	case <-teardownStarted:
	case <-time.After(time.Second):
		t.Fatal("container teardown did not start")
	}

	assertServiceMutexAvailable(t, s)
	close(releaseTeardown)

	select {
	case err := <-waitDone:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("wait did not finish after teardown was released")
	}
}

func TestWaitPublishesExitWithoutWaitingForOtherTeardown(t *testing.T) {
	teardownStarted := make(chan struct{})
	releaseTeardown := make(chan struct{})
	deleteContainerCalled := make(chan struct{})
	defer func() {
		select {
		case <-releaseTeardown:
		default:
			close(releaseTeardown)
		}
	}()

	sandbox := &vcmock.Sandbox{
		MockID: "sandbox",
		WaitProcessFunc: func(containerID, processID string) (int32, error) {
			return 0, nil
		},
		StopContainerFunc: func(contID string, force bool) (vc.VCContainer, error) {
			close(teardownStarted)
			<-releaseTeardown
			return &vcmock.Container{}, nil
		},
		DeleteContainerFunc: func(contID string) (vc.VCContainer, error) {
			close(deleteContainerCalled)
			return &vcmock.Container{}, nil
		},
	}

	s, err := newTestService("sandbox")
	if err != nil {
		t.Fatal(err)
	}
	defer s.cancel()
	s.sandbox = sandbox

	exitIOch := make(chan struct{})
	close(exitIOch)
	c := &container{
		id:       "container",
		cType:    vc.PodContainer,
		spec:     &specs.Spec{},
		status:   task.Status_RUNNING,
		exitIOch: exitIOch,
		exitCh:   make(chan uint32, 1),
	}
	s.containers[c.id] = c

	// Another container's cleanup holds teardownMu. This container's exit
	// must still be published; only its own cleanup has to wait.
	s.teardownMu.Lock()
	teardownLocked := true
	defer func() {
		if teardownLocked {
			s.teardownMu.Unlock()
		}
	}()

	waitDone := make(chan error, 1)
	go func() {
		_, err := wait(context.Background(), s, c, "")
		waitDone <- err
	}()

	select {
	case <-c.exitCh:
	case <-time.After(time.Second):
		t.Fatal("container exit waited for the teardown of another container")
	}

	select {
	case <-teardownStarted:
		t.Fatal("container teardown ran concurrently with another one")
	case <-time.After(100 * time.Millisecond):
	}

	// The exit is visible, but Delete must still wait until StopContainer
	// has finished, or it would delete a container that is being stopped.
	deleteDone := make(chan error, 1)
	go func() {
		_, err := s.Delete(context.Background(), &taskAPI.DeleteRequest{ID: c.id})
		deleteDone <- err
	}()

	s.teardownMu.Unlock()
	teardownLocked = false

	select {
	case <-teardownStarted:
	case <-time.After(time.Second):
		t.Fatal("container teardown did not start")
	}

	select {
	case <-deleteContainerCalled:
		t.Fatal("DeleteContainer ran before StopContainer completed")
	case <-time.After(100 * time.Millisecond):
	}

	close(releaseTeardown)

	select {
	case err := <-waitDone:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("wait did not finish after teardown was released")
	}

	select {
	case <-deleteContainerCalled:
	case <-time.After(time.Second):
		t.Fatal("DeleteContainer did not run after StopContainer completed")
	}

	select {
	case err := <-deleteDone:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("Delete did not finish")
	}
}

func TestRPCsDoNotEnterSandboxDuringTeardown(t *testing.T) {
	teardownStarted := make(chan struct{})
	releaseTeardown := make(chan struct{})
	entered := make(chan string, 2)

	sandbox := &vcmock.Sandbox{
		MockID: "sandbox",
		StopFunc: func(force bool) error {
			close(teardownStarted)
			<-releaseTeardown
			return nil
		},
		SignalProcessFunc: func(containerID, processID string, signal syscall.Signal, all bool) error {
			entered <- "SignalProcess"
			return nil
		},
		PauseContainerFunc: func(contID string) error {
			entered <- "PauseContainer"
			return nil
		},
	}

	s, err := newTestService("sandbox")
	if err != nil {
		t.Fatal(err)
	}
	defer s.cancel()
	s.sandbox = sandbox
	monitor := make(chan error, 1)
	s.monitor = monitor

	c := &container{
		id:     "container",
		cType:  vc.PodContainer,
		status: task.Status_RUNNING,
	}
	s.containers[c.id] = c

	watchDone := make(chan struct{})
	go func() {
		watchSandbox(context.Background(), s)
		close(watchDone)
	}()
	monitor <- errors.New("agent is dead")

	select {
	case <-teardownStarted:
	case <-time.After(time.Second):
		t.Fatal("sandbox teardown did not start")
	}

	// StopPodSandbox kills every container while the teardown runs; that
	// must succeed without reaching into the sandbox being torn down.
	if _, err := s.Kill(context.Background(), &taskAPI.KillRequest{ID: c.id, Signal: uint32(syscall.SIGKILL)}); err != nil {
		t.Fatalf("Kill failed during sandbox teardown: %v", err)
	}
	if _, err := s.Pause(context.Background(), &taskAPI.PauseRequest{ID: c.id}); err == nil {
		t.Fatal("Pause succeeded during sandbox teardown")
	}

	select {
	case call := <-entered:
		t.Fatalf("%s reached the sandbox during its teardown", call)
	default:
	}

	close(releaseTeardown)

	select {
	case <-watchDone:
	case <-time.After(time.Second):
		t.Fatal("sandbox watcher did not finish after teardown was released")
	}
}

func TestWatchSandboxDoesNotHoldServiceMutexDuringTeardown(t *testing.T) {
	teardownStarted := make(chan struct{})
	releaseTeardown := make(chan struct{})

	sandbox := &vcmock.Sandbox{
		MockID: "sandbox",
		StopFunc: func(force bool) error {
			close(teardownStarted)
			<-releaseTeardown
			return nil
		},
	}

	s, err := newTestService("sandbox")
	if err != nil {
		t.Fatal(err)
	}
	defer s.cancel()
	s.sandbox = sandbox
	monitor := make(chan error, 1)
	s.monitor = monitor

	watchDone := make(chan struct{})
	go func() {
		watchSandbox(context.Background(), s)
		close(watchDone)
	}()
	monitor <- errors.New("agent is dead")

	select {
	case <-teardownStarted:
	case <-time.After(time.Second):
		t.Fatal("sandbox teardown did not start")
	}

	assertServiceMutexAvailable(t, s)
	close(releaseTeardown)

	select {
	case <-watchDone:
	case <-time.After(time.Second):
		t.Fatal("sandbox watcher did not finish after teardown was released")
	}
}

func TestDeleteDoesNotActOnReplacedContainer(t *testing.T) {
	vcCalled := make(chan string, 2)

	sandbox := &vcmock.Sandbox{
		MockID: "sandbox",
		StopContainerFunc: func(contID string, force bool) (vc.VCContainer, error) {
			vcCalled <- "StopContainer"
			return &vcmock.Container{}, nil
		},
		DeleteContainerFunc: func(contID string) (vc.VCContainer, error) {
			vcCalled <- "DeleteContainer"
			return &vcmock.Container{}, nil
		},
	}

	s, err := newTestService("sandbox")
	if err != nil {
		t.Fatal(err)
	}
	defer s.cancel()
	s.sandbox = sandbox

	old := &container{
		id:     "container",
		cType:  vc.PodContainer,
		spec:   &specs.Spec{},
		status: task.Status_STOPPED,
	}
	s.containers[old.id] = old

	// Another teardown holds teardownMu, so Delete stops between its two
	// lookups: it has found and locked the old container, and waits here.
	s.teardownMu.Lock()
	teardownLocked := true
	defer func() {
		if teardownLocked {
			s.teardownMu.Unlock()
		}
	}()

	deleteDone := make(chan error, 1)
	go func() {
		_, err := s.Delete(context.Background(), &taskAPI.DeleteRequest{ID: old.id})
		deleteDone <- err
	}()

	// Nothing else takes the old container's lock, so failing to get it
	// means Delete holds it.
	deadline := time.Now().Add(time.Second)
	for old.teardownMu.TryLock() {
		old.teardownMu.Unlock()
		if time.Now().After(deadline) {
			t.Fatal("Delete did not lock the container it found")
		}
		time.Sleep(time.Millisecond)
	}

	// Meanwhile, the container is deleted and a new one is created with the
	// same ID.
	replacement := &container{
		id:     old.id,
		cType:  vc.PodContainer,
		spec:   &specs.Spec{},
		status: task.Status_CREATED,
	}
	s.mu.Lock()
	s.containers[old.id] = replacement
	s.mu.Unlock()
	s.teardownMu.Unlock()
	teardownLocked = false

	select {
	case err := <-deleteDone:
		if err == nil || !isNotFound(err) {
			t.Fatalf("Delete of a replaced container returned %v, want NotFound", err)
		}
	case <-time.After(time.Second):
		t.Fatal("Delete did not finish")
	}

	select {
	case call := <-vcCalled:
		t.Fatalf("Delete called %s on the replacement container", call)
	default:
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if s.containers[old.id] != replacement {
		t.Fatal("Delete removed the replacement container")
	}
}
