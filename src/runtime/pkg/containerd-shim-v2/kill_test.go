// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

package containerdshim

import (
	"context"
	"syscall"
	"testing"
	"time"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	"github.com/containerd/containerd/api/types/task"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
	"github.com/stretchr/testify/require"
	otelTrace "go.opentelemetry.io/otel/trace"
)

const killTestTimeout = 5 * time.Second

type signalProcessCall struct {
	containerID string
	processID   string
	signal      syscall.Signal
	all         bool
}

func newKillTestService(t *testing.T, signalProcessFunc func(context.Context, string, string, syscall.Signal, bool) error) (*service, *container) {
	t.Helper()

	s := &service{
		id: testSandboxID,
		sandbox: &vcmock.Sandbox{
			MockID:            testSandboxID,
			SignalProcessFunc: signalProcessFunc,
		},
		rootCtx:    context.Background(),
		containers: make(map[string]*container),
	}

	c, err := newContainer(s, &taskAPI.CreateTaskRequest{ID: testContainerID}, vc.PodContainer, nil, false)
	require.NoError(t, err)
	c.status = task.Status_RUNNING
	s.containers[c.id] = c

	return s, c
}

func addKillTestExec(c *container, execID, processID string, status task.Status) {
	c.setExec(execID, &exec{
		container: c,
		id:        processID,
		status:    status,
		tty:       &tty{},
	})
}

func TestKillDoesNotHoldServiceLockDuringSignalProcess(t *testing.T) {
	const otherContainerID = "other-container"

	signalCalls := make(chan signalProcessCall, 2)
	releaseSignal := make(chan struct{})
	released := false
	defer func() {
		if !released {
			close(releaseSignal)
		}
	}()

	s, target := newKillTestService(t, func(ctx context.Context, containerID, processID string, signal syscall.Signal, all bool) error {
		signalCalls <- signalProcessCall{
			containerID: containerID,
			processID:   processID,
			signal:      signal,
			all:         all,
		}
		<-releaseSignal
		return nil
	})

	// Treat the target as the sandbox container so Delete only removes the
	// shim-side state and cannot itself block in a mock agent operation.
	target.cType = vc.PodSandbox

	other, err := newContainer(s, &taskAPI.CreateTaskRequest{ID: otherContainerID}, vc.PodContainer, nil, false)
	require.NoError(t, err)
	other.status = task.Status_RUNNING
	s.containers[other.id] = other

	killDone := make(chan error, 2)
	for range 2 {
		go func() {
			_, err := s.Kill(context.Background(), &taskAPI.KillRequest{
				ID:     target.id,
				Signal: uint32(syscall.SIGTERM),
			})
			killDone <- err
		}()
	}

	for range 2 {
		select {
		case call := <-signalCalls:
			require.Equal(t, target.id, call.containerID)
			require.Equal(t, target.id, call.processID)
			require.Equal(t, syscall.SIGTERM, call.signal)
			require.True(t, call.all)
		case <-time.After(killTestTimeout):
			t.Fatal("concurrent Kill calls did not reach SignalProcess")
		}
	}

	stateDone := make(chan error, 1)
	go func() {
		_, err := s.State(context.Background(), &taskAPI.StateRequest{ID: otherContainerID})
		stateDone <- err
	}()

	deleteDone := make(chan error, 1)
	go func() {
		_, err := s.Delete(context.Background(), &taskAPI.DeleteRequest{ID: target.id})
		deleteDone <- err
	}()

	select {
	case err := <-stateDone:
		require.NoError(t, err)
	case <-time.After(killTestTimeout):
		t.Fatal("State remained blocked behind SignalProcess")
	}

	select {
	case err := <-deleteDone:
		require.NoError(t, err)
	case <-time.After(killTestTimeout):
		t.Fatal("Delete remained blocked behind SignalProcess")
	}

	close(releaseSignal)
	released = true

	for range 2 {
		select {
		case err := <-killDone:
			require.NoError(t, err)
		case <-time.After(killTestTimeout):
			t.Fatal("Kill did not return after SignalProcess completed")
		}
	}
}

func TestKillPropagatesRequestContextAndPreservesRootTrace(t *testing.T) {
	signalContext := make(chan context.Context, 1)
	releaseSignal := make(chan struct{})
	defer close(releaseSignal)

	rootSpanContext := otelTrace.NewSpanContext(otelTrace.SpanContextConfig{
		TraceID:    otelTrace.TraceID{1},
		SpanID:     otelTrace.SpanID{1},
		TraceFlags: otelTrace.FlagsSampled,
	})
	rootCtx := otelTrace.ContextWithRemoteSpanContext(context.Background(), rootSpanContext)

	s, _ := newKillTestService(t, func(ctx context.Context, containerID, processID string, signal syscall.Signal, all bool) error {
		signalContext <- ctx
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-releaseSignal:
			return nil
		}
	})
	s.rootCtx = rootCtx

	ctx, cancel := context.WithCancel(context.Background())
	killDone := make(chan error, 1)
	go func() {
		_, err := s.Kill(ctx, &taskAPI.KillRequest{
			ID:     testContainerID,
			Signal: uint32(syscall.SIGTERM),
		})
		killDone <- err
	}()

	var receivedCtx context.Context
	select {
	case receivedCtx = <-signalContext:
	case <-time.After(killTestTimeout):
		t.Fatal("Kill did not call SignalProcess")
	}

	require.Equal(t, rootSpanContext.TraceID(), otelTrace.SpanContextFromContext(receivedCtx).TraceID())

	cancel()

	select {
	case <-receivedCtx.Done():
		require.ErrorIs(t, receivedCtx.Err(), context.Canceled)
	case <-time.After(killTestTimeout):
		t.Fatal("SignalProcess context was not canceled")
	}

	select {
	case err := <-killDone:
		require.ErrorIs(t, err, context.Canceled)
	case <-time.After(killTestTimeout):
		t.Fatal("Kill did not return after its context was canceled")
	}
}

func TestKillProcessTargeting(t *testing.T) {
	const (
		execID        = "exec-id"
		execProcessID = "guest-exec-id"
	)

	tests := []struct {
		name          string
		execID        string
		requestAll    bool
		wantProcessID string
		wantAll       bool
	}{
		{
			name:          "init process forces all",
			wantProcessID: testContainerID,
			wantAll:       true,
		},
		{
			name:          "exec process",
			execID:        execID,
			wantProcessID: execProcessID,
		},
		{
			name:          "exec process preserves all",
			execID:        execID,
			requestAll:    true,
			wantProcessID: execProcessID,
			wantAll:       true,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			signalCalls := make(chan signalProcessCall, 1)
			s, c := newKillTestService(t, func(ctx context.Context, containerID, processID string, signal syscall.Signal, all bool) error {
				signalCalls <- signalProcessCall{
					containerID: containerID,
					processID:   processID,
					signal:      signal,
					all:         all,
				}
				return nil
			})
			addKillTestExec(c, execID, execProcessID, task.Status_RUNNING)

			request := &taskAPI.KillRequest{
				ID:     testContainerID,
				ExecID: test.execID,
				Signal: uint32(syscall.SIGUSR1),
				All:    test.requestAll,
			}

			_, err := s.Kill(context.Background(), request)
			require.NoError(t, err)

			call := <-signalCalls
			require.Equal(t, testContainerID, call.containerID)
			require.Equal(t, test.wantProcessID, call.processID)
			require.Equal(t, syscall.SIGUSR1, call.signal)
			require.Equal(t, test.wantAll, call.all)
			require.Equal(t, test.requestAll, request.All, "Kill mutated its request")
		})
	}
}

func TestKillStoppedProcessIsIdempotent(t *testing.T) {
	const (
		execID        = "exec-id"
		execProcessID = "guest-exec-id"
	)

	tests := []struct {
		name       string
		execID     string
		signal     syscall.Signal
		wantCalled bool
	}{
		{name: "init SIGTERM", signal: syscall.SIGTERM},
		{name: "init SIGKILL", signal: syscall.SIGKILL},
		{name: "exec SIGTERM", execID: execID, signal: syscall.SIGTERM},
		{name: "exec SIGKILL", execID: execID, signal: syscall.SIGKILL},
		{name: "other signal is forwarded", execID: execID, signal: syscall.SIGUSR1, wantCalled: true},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			called := false
			s, c := newKillTestService(t, func(ctx context.Context, containerID, processID string, signal syscall.Signal, all bool) error {
				called = true
				return nil
			})
			c.status = task.Status_STOPPED
			addKillTestExec(c, execID, execProcessID, task.Status_STOPPED)

			_, err := s.Kill(context.Background(), &taskAPI.KillRequest{
				ID:     testContainerID,
				ExecID: test.execID,
				Signal: uint32(test.signal),
			})
			require.NoError(t, err)
			require.Equal(t, test.wantCalled, called)
		})
	}
}
