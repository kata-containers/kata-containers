// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"errors"
	"fmt"
	"io"
	"syscall"
	"testing"
	"time"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	"github.com/containerd/containerd/api/types/task"
	"github.com/containerd/containerd/errdefs"
	"github.com/containerd/containerd/namespaces"
	"github.com/containerd/containerd/protobuf"
	specs "github.com/opencontainers/runtime-spec/specs-go"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestStartStartSandboxSuccess(t *testing.T) {
	assert := assert.New(t)
	var err error

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	sandbox.StatusContainerFunc = func(contID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
			},
		}, nil
	}

	defer func() {
		sandbox.StatusContainerFunc = nil
	}()

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
		ctx:        namespaces.WithNamespace(context.Background(), "UnitTest"),
	}

	reqCreate := &taskAPI.CreateTaskRequest{
		ID: testSandboxID,
	}
	s.containers[testSandboxID], err = newContainer(s, reqCreate, vc.PodSandbox, nil, false)
	assert.NoError(err)

	reqStart := &taskAPI.StartRequest{
		ID: testSandboxID,
	}

	sandbox.StartFunc = func() error {
		return nil
	}

	defer func() {
		sandbox.StartFunc = nil
	}()

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Start(ctx, reqStart)
	assert.NoError(err)
}

func TestStartMissingAnnotation(t *testing.T) {
	assert := assert.New(t)
	var err error

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	sandbox.StatusContainerFunc = func(contID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID:          sandbox.ID(),
			Annotations: map[string]string{},
		}, nil
	}

	defer func() {
		sandbox.StatusContainerFunc = nil
	}()

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
		ctx:        namespaces.WithNamespace(context.Background(), "UnitTest"),
	}

	reqCreate := &taskAPI.CreateTaskRequest{
		ID: testSandboxID,
	}
	s.containers[testSandboxID], err = newContainer(s, reqCreate, "", nil, false)
	assert.NoError(err)

	reqStart := &taskAPI.StartRequest{
		ID: testSandboxID,
	}

	sandbox.StartFunc = func() error {
		return nil
	}

	defer func() {
		sandbox.StartFunc = nil
	}()

	_, err = s.Start(s.ctx, reqStart)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestStartStartContainerSucess(t *testing.T) {
	assert := assert.New(t)
	var err error

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	sandbox.MockContainers = []*vcmock.Container{
		{
			MockID:      testContainerID,
			MockSandbox: sandbox,
		},
	}

	sandbox.StatusContainerFunc = func(contID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID: testContainerID,
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
			},
		}, nil
	}

	defer func() {
		sandbox.StatusContainerFunc = nil
	}()

	sandbox.StartContainerFunc = func(contID string) (vc.VCContainer, error) {
		return sandbox.MockContainers[0], nil
	}

	defer func() {
		sandbox.StartContainerFunc = nil
	}()

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
		ctx:        namespaces.WithNamespace(context.Background(), "UnitTest"),
	}

	reqCreate := &taskAPI.CreateTaskRequest{
		ID: testContainerID,
	}
	s.containers[testContainerID], err = newContainer(s, reqCreate, vc.PodContainer, nil, false)
	assert.NoError(err)

	reqStart := &taskAPI.StartRequest{
		ID: testContainerID,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Start(ctx, reqStart)
	assert.NoError(err)
}

// shimCallTimeout bounds every shim call in the start-failure tests: before the
// fix, a repeated failed Start blocked forever while holding s.mu.
const shimCallTimeout = 5 * time.Second

func callShim[T any](t *testing.T, name string, call func() (T, error)) (T, error) {
	t.Helper()
	type result struct {
		value T
		err   error
	}
	done := make(chan result, 1)
	go func() {
		value, err := call()
		done <- result{value, err}
	}()
	select {
	case r := <-done:
		return r.value, r.err
	case <-time.After(shimCallTimeout):
		t.Fatalf("%s did not return within %s: shim service is deadlocked", name, shimCallTimeout)
		var zero T
		return zero, nil
	}
}

func newStartFailureService(t *testing.T, sandbox *vcmock.Sandbox) *service {
	t.Helper()
	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
		ctx:        namespaces.WithNamespace(context.Background(), "UnitTest"),
	}
	c, err := newContainer(s, &taskAPI.CreateTaskRequest{ID: testContainerID}, vc.PodContainer, nil, false)
	require.NoError(t, err)
	s.containers[testContainerID] = c
	return s
}

// TestStartExecFailureRepeatedStart covers a failed exec that is started again,
// with or without a Wait() in between. The failed Start filled the exec's
// single-slot exit channel, and Wait() puts the code it reads back, so the slot
// stayed full; the repeated Start then blocked on it while holding s.mu,
// hanging every later request for the sandbox.
func TestStartExecFailureRepeatedStart(t *testing.T) {
	for _, waitBetween := range []bool{false, true} {
		t.Run(fmt.Sprintf("waitBetween=%t", waitBetween), func(t *testing.T) {
			testStartExecFailureRepeatedStart(t, waitBetween)
		})
	}
}

func testStartExecFailureRepeatedStart(t *testing.T, waitBetween bool) {
	require := require.New(t)
	const execID = "missing-binary"

	sandbox := &vcmock.Sandbox{MockID: testSandboxID}
	enterCalls := 0
	sandbox.EnterContainerFunc = func(containerID string, cmd types.Cmd) (vc.VCContainer, *vc.Process, error) {
		enterCalls++
		return nil, nil, errors.New("the file /bin/missing was not found")
	}
	s := newStartFailureService(t, sandbox)
	c := s.containers[testContainerID]
	c.status = task.Status_RUNNING

	spec, err := protobuf.MarshalAnyToProto(&specs.Process{Args: []string{"/bin/missing"}, Cwd: "/"})
	require.NoError(err)
	c.execs[execID], err = newExec(c, "", "", "", false, spec)
	require.NoError(err)

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	start := &taskAPI.StartRequest{ID: testContainerID, ExecID: execID}
	waitExec := func(name string) {
		wait, err := callShim(t, name, func() (*taskAPI.WaitResponse, error) {
			return s.Wait(ctx, &taskAPI.WaitRequest{ID: testContainerID, ExecID: execID})
		})
		require.NoError(err)
		require.Equal(uint32(exitCode255), wait.ExitStatus)
	}

	_, err = callShim(t, "first Start", func() (*taskAPI.StartResponse, error) { return s.Start(ctx, start) })
	require.ErrorContains(err, "cannot enter container")

	if waitBetween {
		waitExec("exec Wait before repeated Start")
	}

	_, err = callShim(t, "repeated Start", func() (*taskAPI.StartResponse, error) { return s.Start(ctx, start) })
	require.True(errdefs.IsFailedPrecondition(errdefs.FromGRPC(err)), "repeated Start: %v", err)
	require.Equal(1, enterCalls, "a stopped exec must not enter the container again")

	state, err := callShim(t, "exec State", func() (*taskAPI.StateResponse, error) {
		return s.State(ctx, &taskAPI.StateRequest{ID: testContainerID, ExecID: execID})
	})
	require.NoError(err)
	require.Equal(task.Status_STOPPED, state.Status)
	require.Equal(uint32(exitCode255), state.ExitStatus)

	waitExec("exec Wait")

	// No guest process exists, so Kill reports NotFound (unchanged behaviour,
	// which containerd's cleanup accepts); it must not block.
	_, err = callShim(t, "exec Kill", func() (any, error) {
		return s.Kill(ctx, &taskAPI.KillRequest{ID: testContainerID, ExecID: execID, Signal: uint32(syscall.SIGKILL)})
	})
	require.True(err == nil || errdefs.IsNotFound(errdefs.FromGRPC(err)), "exec Kill: %v", err)

	deleted, err := callShim(t, "exec Delete", func() (*taskAPI.DeleteResponse, error) {
		return s.Delete(ctx, &taskAPI.DeleteRequest{ID: testContainerID, ExecID: execID})
	})
	require.NoError(err)
	require.Equal(uint32(exitCode255), deleted.ExitStatus)
}

// A failed start is final even if the guest would accept a later retry.
// Exercise a full exit channel, a completed Wait, and the window while Wait
// has consumed the exit code but has not put it back yet.
func TestStartContainerFailureRepeatedStart(t *testing.T) {
	for _, containerType := range []vc.ContainerType{vc.PodContainer, vc.PodSandbox} {
		for _, retrySucceeds := range []bool{false, true} {
			for _, waitMode := range []string{"none", "before", "during"} {
				t.Run(fmt.Sprintf("%s/retrySucceeds=%t/wait=%s", containerType, retrySucceeds, waitMode), func(t *testing.T) {
					testStartContainerFailureRepeatedStart(t, containerType, retrySucceeds, waitMode)
				})
			}
		}
	}
}

func testStartContainerFailureRepeatedStart(t *testing.T, containerType vc.ContainerType, retrySucceeds bool, waitMode string) {
	require := require.New(t)
	sandbox := &vcmock.Sandbox{MockID: testSandboxID}
	startCalls := 0
	startGuest := func() error {
		startCalls++
		if retrySucceeds && startCalls > 1 {
			return nil
		}
		return errors.New("start failed")
	}
	sandbox.StartFunc = startGuest
	sandbox.StartContainerFunc = func(string) (vc.VCContainer, error) {
		return &vcmock.Container{}, startGuest()
	}
	s := newStartFailureService(t, sandbox)
	c := s.containers[testContainerID]
	c.cType = containerType
	ctx := s.ctx
	start := &taskAPI.StartRequest{ID: testContainerID}
	waitContainer := func() (*taskAPI.WaitResponse, error) {
		return s.Wait(ctx, &taskAPI.WaitRequest{ID: testContainerID})
	}
	_, err := callShim(t, "first Start", func() (*taskAPI.StartResponse, error) { return s.Start(ctx, start) })
	require.ErrorContains(err, "start failed")
	exitTime := c.exitTime
	require.False(exitTime.IsZero())

	switch waitMode {
	case "before":
		response, err := callShim(t, "Wait before retry", waitContainer)
		require.NoError(err)
		require.Equal(uint32(exitCode255), response.ExitStatus)
	case "during":
		// Reproduce Wait's receive/refill interleaving deterministically. Wait
		// releases s.mu before reading this channel, so Start can run here.
		code, err := callShim(t, "Wait receive", func() (uint32, error) { return <-c.exitCh, nil })
		require.NoError(err)
		require.Equal(uint32(exitCode255), code)
	}

	_, err = callShim(t, "repeated Start", func() (*taskAPI.StartResponse, error) { return s.Start(ctx, start) })
	require.True(errdefs.IsFailedPrecondition(errdefs.FromGRPC(err)), "repeated Start: %v", err)
	require.Equal(1, startCalls, "a failed start must never be retried in the guest")
	require.Equal(exitTime, c.exitTime, "rejected retries must not replace the exit result")
	if waitMode == "during" {
		require.Empty(c.exitCh, "retry must not refill the slot held by Wait")
		c.exitCh <- exitCode255
	}
	response, err := callShim(t, "Wait after retry", waitContainer)
	require.NoError(err)
	require.Equal(uint32(exitCode255), response.ExitStatus)
	require.True(exitTime.Equal(response.ExitedAt.AsTime()))
	state, err := callShim(t, "State after retry", func() (*taskAPI.StateResponse, error) {
		return s.State(ctx, &taskAPI.StateRequest{ID: testContainerID})
	})
	require.NoError(err)
	require.Equal(task.Status_CREATED, state.Status, "retain the state needed for guest cleanup")
	require.Equal(uint32(exitCode255), state.ExitStatus)

	if containerType == vc.PodContainer {
		stopped := false
		sandbox.StopContainerFunc = func(string, bool) (vc.VCContainer, error) {
			stopped = true
			return &vcmock.Container{}, nil
		}
		deleted, err := callShim(t, "Delete after failure", func() (*taskAPI.DeleteResponse, error) {
			return s.Delete(ctx, &taskAPI.DeleteRequest{ID: testContainerID})
		})
		require.NoError(err)
		require.True(stopped, "Delete must still stop a partially started container")
		require.Equal(uint32(exitCode255), deleted.ExitStatus)
	}
}

// A failure after the guest started must reject retries without marking the
// live guest STOPPED, which would cause Kill and Delete to skip cleanup.
func TestStartContainerIOFailure(t *testing.T) {
	require := require.New(t)
	sandbox := &vcmock.Sandbox{MockID: testSandboxID}
	sandbox.IOStreamFunc = func(string, string) (io.WriteCloser, io.Reader, io.Reader, error) {
		return nil, nil, nil, errors.New("IO stream failed")
	}
	s := newStartFailureService(t, sandbox)
	req := &taskAPI.StartRequest{ID: testContainerID}
	_, err := callShim(t, "Start with failed IO", func() (*taskAPI.StartResponse, error) { return s.Start(s.ctx, req) })
	require.ErrorContains(err, "IO stream failed")
	require.Equal(task.Status_RUNNING, s.containers[testContainerID].status)
	_, err = callShim(t, "retry after failed IO", func() (*taskAPI.StartResponse, error) { return s.Start(s.ctx, req) })
	require.True(errdefs.IsFailedPrecondition(errdefs.FromGRPC(err)))
	signalled := false
	sandbox.SignalProcessFunc = func(string, string, syscall.Signal, bool) error {
		signalled = true
		return nil
	}
	_, err = callShim(t, "Kill after failed IO", func() (any, error) {
		return s.Kill(s.ctx, &taskAPI.KillRequest{ID: testContainerID, Signal: uint32(syscall.SIGKILL)})
	})
	require.NoError(err)
	require.True(signalled, "the live guest must still be signalled")
	stopped := false
	sandbox.StopContainerFunc = func(string, bool) (vc.VCContainer, error) {
		stopped = true
		return &vcmock.Container{}, nil
	}
	_, err = callShim(t, "Delete after failed IO", func() (*taskAPI.DeleteResponse, error) {
		return s.Delete(s.ctx, &taskAPI.DeleteRequest{ID: testContainerID})
	})
	require.NoError(err)
	require.True(stopped)
}

// Concurrent Wait calls must all observe the same failure while Start retries
// are rejected. There must never be a second producer racing their refills.
func TestStartContainerFailureConcurrentWait(t *testing.T) {
	require := require.New(t)
	sandbox := &vcmock.Sandbox{MockID: testSandboxID}
	sandbox.StartContainerFunc = func(string) (vc.VCContainer, error) { return nil, errors.New("start failed") }
	s := newStartFailureService(t, sandbox)
	req := &taskAPI.StartRequest{ID: testContainerID}
	_, err := callShim(t, "first Start", func() (*taskAPI.StartResponse, error) { return s.Start(s.ctx, req) })
	require.ErrorContains(err, "start failed")
	const waiters = 16
	results := make(chan error, waiters)
	for i := 0; i < waiters; i++ {
		go func() {
			response, err := s.Wait(s.ctx, &taskAPI.WaitRequest{ID: testContainerID})
			if err == nil && response.ExitStatus != exitCode255 {
				err = fmt.Errorf("unexpected exit status %d", response.ExitStatus)
			}
			results <- err
		}()
		_, err := callShim(t, "concurrent retry", func() (*taskAPI.StartResponse, error) { return s.Start(s.ctx, req) })
		require.True(errdefs.IsFailedPrecondition(errdefs.FromGRPC(err)))
	}
	for i := 0; i < waiters; i++ {
		_, err := callShim(t, "concurrent Wait", func() (any, error) { return nil, <-results })
		require.NoError(err)
	}
}

// TestStartRunningExecRejected checks that an exec which already started is
// not entered into the container a second time. This does not deadlock without
// the fix (the exit channel is still empty); it fails because the repeated
// Start calls EnterContainer again.
func TestStartRunningExecRejected(t *testing.T) {
	require := require.New(t)
	const execID = "running"

	sandbox := &vcmock.Sandbox{MockID: testSandboxID}
	entered := false
	sandbox.EnterContainerFunc = func(containerID string, cmd types.Cmd) (vc.VCContainer, *vc.Process, error) {
		entered = true
		return nil, nil, errors.New("unexpected EnterContainer")
	}
	s := newStartFailureService(t, sandbox)
	c := s.containers[testContainerID]
	c.status = task.Status_RUNNING

	spec, err := protobuf.MarshalAnyToProto(&specs.Process{Args: []string{"/bin/true"}, Cwd: "/"})
	require.NoError(err)
	c.execs[execID], err = newExec(c, "", "", "", false, spec)
	require.NoError(err)
	c.execs[execID].status = task.Status_RUNNING
	c.execs[execID].id = "guest-process"

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = callShim(t, "Start", func() (*taskAPI.StartResponse, error) {
		return s.Start(ctx, &taskAPI.StartRequest{ID: testContainerID, ExecID: execID})
	})
	require.True(errdefs.IsFailedPrecondition(errdefs.FromGRPC(err)), "Start of running exec: %v", err)
	require.False(entered, "a running exec must not enter the container again")
	require.Equal(task.Status_RUNNING, c.execs[execID].status)
}

// Starting an exited process again must not create a second IO copier that
// closes the already closed exitIOch and panics in the shim.
func TestStartExitedProcessRejected(t *testing.T) {
	for _, execID := range []string{"", "completed-exec"} {
		t.Run(fmt.Sprintf("exec=%s", execID), func(t *testing.T) {
			require := require.New(t)
			sandbox := &vcmock.Sandbox{MockID: testSandboxID}
			startCalls := 0
			sandbox.StartContainerFunc = func(string) (vc.VCContainer, error) {
				startCalls++
				return &vcmock.Container{}, nil
			}
			sandbox.EnterContainerFunc = func(string, types.Cmd) (vc.VCContainer, *vc.Process, error) {
				startCalls++
				return &vcmock.Container{}, &vc.Process{Token: "guest-process"}, nil
			}
			s := newStartFailureService(t, sandbox)
			s.ec = make(chan exit, 1)
			c := s.containers[testContainerID]
			if execID != "" {
				c.status = task.Status_RUNNING
				spec, err := protobuf.MarshalAnyToProto(&specs.Process{Args: []string{"/bin/true"}, Cwd: "/"})
				require.NoError(err)
				c.execs[execID], err = newExec(c, "", "", "", false, spec)
				require.NoError(err)
			}
			req := &taskAPI.StartRequest{ID: testContainerID, ExecID: execID}
			_, err := callShim(t, "successful Start", func() (*taskAPI.StartResponse, error) { return s.Start(s.ctx, req) })
			require.NoError(err)
			_, err = callShim(t, "process exit", func() (exit, error) { return <-s.ec, nil })
			require.NoError(err)
			_, err = callShim(t, "Start after exit", func() (*taskAPI.StartResponse, error) { return s.Start(s.ctx, req) })
			require.True(errdefs.IsFailedPrecondition(errdefs.FromGRPC(err)))
			require.Equal(1, startCalls)
			response, err := callShim(t, "Wait after rejected Start", func() (*taskAPI.WaitResponse, error) {
				return s.Wait(s.ctx, &taskAPI.WaitRequest{ID: testContainerID, ExecID: execID})
			})
			require.NoError(err)
			require.Zero(response.ExitStatus, "rejected Start must not overwrite the successful exit")
			s.teardownWg.Wait()
		})
	}
}
