// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

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
	"github.com/stretchr/testify/require"
	otelTrace "go.opentelemetry.io/otel/trace"
)

const statsTestTimeout = 5 * time.Second

func newStatsTestService(
	t *testing.T,
	statsContainerFunc func(context.Context, string) (vc.ContainerStats, error),
	signalProcessFunc func(context.Context, string, string, syscall.Signal, bool) error,
) (*service, *container) {
	t.Helper()

	s := &service{
		id: testSandboxID,
		sandbox: &vcmock.Sandbox{
			MockID:             testSandboxID,
			StatsContainerFunc: statsContainerFunc,
			SignalProcessFunc:  signalProcessFunc,
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

func TestStatsDoesNotHoldServiceLockDuringStatsContainer(t *testing.T) {
	statsStarted := make(chan string, 1)
	releaseStats := make(chan struct{})
	released := false
	defer func() {
		if !released {
			close(releaseStats)
		}
	}()

	statsErr := errors.New("stats released")
	signalCalls := make(chan signalProcessCall, 1)
	s, target := newStatsTestService(
		t,
		func(ctx context.Context, containerID string) (vc.ContainerStats, error) {
			statsStarted <- containerID
			select {
			case <-releaseStats:
				return vc.ContainerStats{}, statsErr
			case <-ctx.Done():
				return vc.ContainerStats{}, ctx.Err()
			}
		},
		func(ctx context.Context, containerID, processID string, signal syscall.Signal, all bool) error {
			signalCalls <- signalProcessCall{
				containerID: containerID,
				processID:   processID,
				signal:      signal,
				all:         all,
			}
			return nil
		},
	)

	// Deleting a sandbox container only removes shim-side state, keeping this
	// regression focused on whether Stats monopolizes the service mutex.
	target.cType = vc.PodSandbox

	statsDone := make(chan error, 1)
	go func() {
		_, err := s.Stats(context.Background(), &taskAPI.StatsRequest{ID: target.id})
		statsDone <- err
	}()

	select {
	case containerID := <-statsStarted:
		require.Equal(t, target.id, containerID)
	case <-time.After(statsTestTimeout):
		t.Fatal("Stats did not reach StatsContainer")
	}

	stateDone := make(chan error, 1)
	go func() {
		_, err := s.State(context.Background(), &taskAPI.StateRequest{ID: target.id})
		stateDone <- err
	}()

	select {
	case err := <-stateDone:
		require.NoError(t, err)
	case <-time.After(statsTestTimeout):
		t.Fatal("State remained blocked behind StatsContainer")
	}

	killDone := make(chan error, 1)
	go func() {
		_, err := s.Kill(context.Background(), &taskAPI.KillRequest{
			ID:     target.id,
			Signal: uint32(syscall.SIGTERM),
		})
		killDone <- err
	}()

	select {
	case call := <-signalCalls:
		require.Equal(t, target.id, call.containerID)
		require.Equal(t, target.id, call.processID)
		require.Equal(t, syscall.SIGTERM, call.signal)
		require.True(t, call.all)
	case <-time.After(statsTestTimeout):
		t.Fatal("Kill did not reach SignalProcess while StatsContainer was blocked")
	}

	select {
	case err := <-killDone:
		require.NoError(t, err)
	case <-time.After(statsTestTimeout):
		t.Fatal("Kill remained blocked behind StatsContainer")
	}

	deleteDone := make(chan error, 1)
	go func() {
		_, err := s.Delete(context.Background(), &taskAPI.DeleteRequest{ID: target.id})
		deleteDone <- err
	}()

	select {
	case err := <-deleteDone:
		require.NoError(t, err)
	case <-time.After(statsTestTimeout):
		t.Fatal("Delete remained blocked behind StatsContainer")
	}

	close(releaseStats)
	released = true

	select {
	case err := <-statsDone:
		require.ErrorIs(t, err, statsErr)
	case <-time.After(statsTestTimeout):
		t.Fatal("Stats did not return after StatsContainer completed")
	}
}

func TestStatsPropagatesRequestContextAndPreservesRootTrace(t *testing.T) {
	statsContext := make(chan context.Context, 1)

	rootSpanContext := otelTrace.NewSpanContext(otelTrace.SpanContextConfig{
		TraceID:    otelTrace.TraceID{1},
		SpanID:     otelTrace.SpanID{1},
		TraceFlags: otelTrace.FlagsSampled,
	})
	rootCtx := otelTrace.ContextWithRemoteSpanContext(context.Background(), rootSpanContext)

	s, _ := newStatsTestService(
		t,
		func(ctx context.Context, containerID string) (vc.ContainerStats, error) {
			statsContext <- ctx
			<-ctx.Done()
			return vc.ContainerStats{}, ctx.Err()
		},
		nil,
	)
	s.rootCtx = rootCtx

	ctx, cancel := context.WithCancel(context.Background())
	statsDone := make(chan error, 1)
	go func() {
		_, err := s.Stats(ctx, &taskAPI.StatsRequest{ID: testContainerID})
		statsDone <- err
	}()

	var receivedCtx context.Context
	select {
	case receivedCtx = <-statsContext:
	case <-time.After(statsTestTimeout):
		t.Fatal("Stats did not call StatsContainer")
	}

	require.Equal(t, rootSpanContext.TraceID(), otelTrace.SpanContextFromContext(receivedCtx).TraceID())

	cancel()

	select {
	case <-receivedCtx.Done():
		require.ErrorIs(t, receivedCtx.Err(), context.Canceled)
	case <-time.After(statsTestTimeout):
		t.Fatal("StatsContainer context was not canceled")
	}

	select {
	case err := <-statsDone:
		require.ErrorIs(t, err, context.Canceled)
	case <-time.After(statsTestTimeout):
		t.Fatal("Stats did not return after its context was canceled")
	}
}
