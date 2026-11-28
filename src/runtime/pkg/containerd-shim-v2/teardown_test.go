// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"errors"
	"os"
	sysexec "os/exec"
	"path/filepath"
	"strings"
	"sync/atomic"
	"syscall"
	"testing"
	"time"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/stretchr/testify/require"
)

type teardownTestSandbox struct {
	vc.VCSandbox
	stop   func(context.Context, bool) error
	delete func(context.Context) error
}

func (sandbox *teardownTestSandbox) ID() string { return "teardown-test" }

func (sandbox *teardownTestSandbox) Stop(ctx context.Context, force bool) error {
	return sandbox.stop(ctx, force)
}

func (sandbox *teardownTestSandbox) Delete(ctx context.Context) error {
	return sandbox.delete(ctx)
}

func (sandbox *teardownTestSandbox) WaitProcess(context.Context, string, string) (int32, error) {
	return 0, nil
}

func TestTeardownSandboxConcurrent(t *testing.T) {
	var stopCalls, deleteCalls atomic.Int32
	started := make(chan struct{})
	release := make(chan struct{})
	t.Cleanup(func() { close(release) })
	service := &service{sandbox: &teardownTestSandbox{
		stop: func(ctx context.Context, force bool) error {
			stopCalls.Add(1)
			close(started)
			<-release
			return nil
		},
		delete: func(ctx context.Context) error {
			deleteCalls.Add(1)
			return nil
		},
	}}
	firstDone := make(chan struct{})
	go func() {
		service.teardownSandbox(context.Background())
		close(firstDone)
	}()
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("teardown did not start")
	}
	secondStarted := make(chan struct{})
	secondDone := make(chan struct{})
	go func() {
		close(secondStarted)
		service.teardownSandbox(context.Background())
		close(secondDone)
	}()
	<-secondStarted
	select {
	case <-secondDone:
		t.Fatal("concurrent teardown returned before cleanup completed")
	case <-time.After(20 * time.Millisecond):
	}
	release <- struct{}{}
	for _, done := range []chan struct{}{firstDone, secondDone} {
		select {
		case <-done:
		case <-time.After(time.Second):
			t.Fatal("teardown did not complete")
		}
	}
	service.teardownSandbox(context.Background())
	require.EqualValues(t, 1, stopCalls.Load())
	require.EqualValues(t, 1, deleteCalls.Load())
}

func TestTeardownSandboxCanceledContext(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	deleted := false
	service := &service{sandbox: &teardownTestSandbox{
		stop: func(ctx context.Context, force bool) error {
			require.NoError(t, ctx.Err())
			deadline, ok := ctx.Deadline()
			require.True(t, ok)
			require.WithinDuration(t, time.Now().Add(cleanupTimeout), deadline, time.Second)
			require.True(t, force)
			return context.DeadlineExceeded
		},
		delete: func(ctx context.Context) error {
			require.NoError(t, ctx.Err())
			deadline, ok := ctx.Deadline()
			require.True(t, ok)
			require.WithinDuration(t, time.Now().Add(cleanupTimeout), deadline, time.Second)
			deleted = true
			return nil
		},
	}}
	service.teardownSandbox(ctx)
	require.True(t, deleted)
}

func TestTeardownSandboxUninitialized(t *testing.T) {
	service := &service{}
	require.NotPanics(t, func() { service.teardownSandbox(context.Background()) })
}

func TestWaitSandboxTeardown(t *testing.T) {
	var calls []string
	service := &service{
		ec: make(chan exit, 1),
		sandbox: &teardownTestSandbox{
			stop: func(context.Context, bool) error {
				calls = append(calls, "stop")
				return nil
			},
			delete: func(context.Context) error {
				calls = append(calls, "delete")
				return nil
			},
		},
	}
	container := &container{
		id:       "sandbox",
		cType:    vc.PodSandbox,
		exitIOch: make(chan struct{}),
		exitCh:   make(chan uint32, 1),
	}
	close(container.exitIOch)
	status, err := wait(context.Background(), service, container, "")
	require.NoError(t, err)
	require.Zero(t, status)
	service.teardownWg.Wait()
	service.teardownSandbox(context.Background())
	require.Equal(t, []string{"stop", "delete"}, calls)
	select {
	case <-service.ec:
	case <-time.After(time.Second):
		t.Fatal("waiter did not publish the exit event")
	}
}

func TestWatchSandboxTeardown(t *testing.T) {
	var calls []string
	service := &service{
		monitor: make(chan error, 1),
		sandbox: &teardownTestSandbox{
			stop: func(context.Context, bool) error {
				calls = append(calls, "stop")
				return nil
			},
			delete: func(context.Context) error {
				calls = append(calls, "delete")
				return errors.New("cleanup failed")
			},
		},
	}
	service.monitor <- errors.New("VMM exited")
	watchSandbox(context.Background(), service)
	service.teardownSandbox(context.Background())
	require.Nil(t, service.monitor)
	require.Equal(t, []string{"stop", "delete"}, calls)
}

func TestShutdownSandboxTeardown(t *testing.T) {
	if mode := os.Getenv("KATA_SHUTDOWN_TEST_MODE"); mode != "" {
		var calls []string
		service := &service{
			rootCtx: context.Background(),
			hpid:    1<<31 - 1,
			cancel: func() {
				err := os.WriteFile(os.Getenv("KATA_SHUTDOWN_TEST_RESULT"), []byte(strings.Join(calls, ",")), 0600)
				require.NoError(t, err)
			},
		}
		if mode != "uninitialized" {
			service.sandbox = &teardownTestSandbox{
				stop: func(ctx context.Context, force bool) error {
					calls = append(calls, "stop")
					return nil
				},
				delete: func(ctx context.Context) error {
					calls = append(calls, "delete")
					return nil
				},
			}
		}
		switch mode {
		case "waiter-completed":
			service.teardownSandbox(context.Background())
		case "waiter-in-progress":
			started := make(chan struct{})
			service.teardownWg.Add(1)
			go func() {
				defer service.teardownWg.Done()
				close(started)
				service.teardownSandbox(context.Background())
			}()
			<-started
		case "workload-waiter":
			service.teardownWg.Add(1)
			go func() {
				calls = append(calls, "workload")
				service.teardownWg.Done()
			}()
		}
		_, err := service.Shutdown(context.Background(), &taskAPI.ShutdownRequest{})
		require.NoError(t, err)
		t.Fatal("Shutdown returned instead of exiting")
	}

	for _, mode := range []string{"shutdown-only", "waiter-completed", "waiter-in-progress", "workload-waiter", "uninitialized"} {
		t.Run(mode, func(t *testing.T) {
			result := filepath.Join(t.TempDir(), "cleanup")
			ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
			defer cancel()
			command := sysexec.CommandContext(ctx, os.Args[0], "-test.run=^TestShutdownSandboxTeardown$")
			command.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
			command.Env = append(os.Environ(), "KATA_SHUTDOWN_TEST_MODE="+mode, "KATA_SHUTDOWN_TEST_RESULT="+result)
			output, err := command.CombinedOutput()
			require.NoError(t, err, "%s", output)
			cleanup, err := os.ReadFile(result)
			require.NoError(t, err)
			expected := "stop,delete"
			switch mode {
			case "uninitialized":
				expected = ""
			case "workload-waiter":
				expected = "workload," + expected
			}
			require.Equal(t, expected, string(cleanup))
		})
	}
}
