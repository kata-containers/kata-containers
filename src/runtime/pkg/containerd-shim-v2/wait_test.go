// Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"errors"
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
		s.mu.Unlock()
		close(acquired)
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

	s, err := newService("sandbox")
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

func TestWaitSerializesExitPublicationWithDelete(t *testing.T) {
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

	s, err := newService("sandbox")
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

	// Holding teardownMu must prevent the shim-visible exit transition. If
	// exit is published before wait owns teardown, Delete can take the lock
	// first and observe inconsistent shim and virtcontainers states.
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
		t.Fatal("container exit became visible before wait acquired teardown ownership")
	case <-time.After(100 * time.Millisecond):
	}

	s.teardownMu.Unlock()
	teardownLocked = false

	select {
	case <-c.exitCh:
	case <-time.After(time.Second):
		t.Fatal("container exit was not published")
	}

	select {
	case <-teardownStarted:
	case <-time.After(time.Second):
		t.Fatal("container teardown did not start")
	}

	deleteStarted := make(chan struct{})
	deleteDone := make(chan error, 1)
	go func() {
		close(deleteStarted)
		_, err := s.Delete(context.Background(), &taskAPI.DeleteRequest{ID: c.id})
		deleteDone <- err
	}()
	<-deleteStarted

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

	s, err := newService("sandbox")
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
