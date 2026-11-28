// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"errors"
	"sync"
	"sync/atomic"
	"syscall"
	"testing"
	"time"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	"github.com/containerd/containerd/api/types/task"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/stretchr/testify/require"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

type cleanupTestSandbox struct {
	vc.VCSandbox
	waitProcess     func(string, string) (int32, error)
	stopContainer   func(context.Context, string, bool) (vc.VCContainer, error)
	deleteContainer func(string) (vc.VCContainer, error)
}

func (sandbox *cleanupTestSandbox) ID() string { return "sandbox" }

func (sandbox *cleanupTestSandbox) WaitProcess(_ context.Context, containerID, processID string) (int32, error) {
	return sandbox.waitProcess(containerID, processID)
}

func (sandbox *cleanupTestSandbox) StopContainer(ctx context.Context, containerID string, force bool) (vc.VCContainer, error) {
	return sandbox.stopContainer(ctx, containerID, force)
}

func (sandbox *cleanupTestSandbox) DeleteContainer(_ context.Context, containerID string) (vc.VCContainer, error) {
	return sandbox.deleteContainer(containerID)
}

func newCleanupTestService(t *testing.T, sandbox *cleanupTestSandbox) (*service, *container) {
	t.Helper()
	service := &service{
		rootCtx:    context.Background(),
		containers: make(map[string]*container),
		ec:         make(chan exit, 1),
		sandbox:    sandbox,
	}
	container, err := newContainer(service, &taskAPI.CreateTaskRequest{ID: "workload"}, vc.PodContainer, nil, false)
	require.NoError(t, err)
	service.containers[container.id] = container
	return service, container
}

func TestWaitContainerCleanupBeforeDelete(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	cleanupStarted := make(chan struct{})
	releaseCleanup := make(chan struct{})
	var releaseOnce sync.Once
	release := func() { releaseOnce.Do(func() { close(releaseCleanup) }) }
	t.Cleanup(release)
	var cleanupFinished atomic.Bool
	var stopCalls, deleteCalls atomic.Int32
	service := &service{
		rootCtx:    ctx,
		containers: make(map[string]*container),
		ec:         make(chan exit, 1),
		sandbox: &cleanupTestSandbox{
			waitProcess: func(string, string) (int32, error) {
				return 0, nil
			},
			stopContainer: func(context.Context, string, bool) (vc.VCContainer, error) {
				stopCalls.Add(1)
				close(cleanupStarted)
				<-releaseCleanup
				cleanupFinished.Store(true)
				return nil, nil
			},
			deleteContainer: func(string) (vc.VCContainer, error) {
				deleteCalls.Add(1)
				if !cleanupFinished.Load() {
					return nil, context.DeadlineExceeded
				}
				return nil, nil
			},
		},
	}
	container, err := newContainer(service, &taskAPI.CreateTaskRequest{ID: "workload"}, vc.PodContainer, nil, false)
	require.NoError(t, err)
	service.containers[container.id] = container
	close(container.exitIOch)
	waitDone := make(chan error, 1)
	go func() {
		_, err := wait(ctx, service, container, "")
		waitDone <- err
	}()
	exitReported := make(chan *taskAPI.WaitResponse, 1)
	waitRPCErr := make(chan error, 1)
	go func() {
		response, err := service.Wait(ctx, &taskAPI.WaitRequest{ID: container.id})
		exitReported <- response
		waitRPCErr <- err
	}()
	select {
	case response := <-exitReported:
		require.NoError(t, <-waitRPCErr)
		require.Zero(t, response.ExitStatus)
	case <-ctx.Done():
		t.Fatal("exit status was not published before cleanup")
	}
	select {
	case event := <-service.ec:
		require.Equal(t, container.id, event.id)
	case <-ctx.Done():
		t.Fatal("exit event was not published before cleanup")
	}
	select {
	case <-cleanupStarted:
	case <-ctx.Done():
		t.Fatal("waiter cleanup did not start")
	}
	deleteStarted := make(chan struct{})
	deleteDone := make(chan error, 1)
	go func() {
		close(deleteStarted)
		_, err := service.Delete(ctx, &taskAPI.DeleteRequest{ID: container.id})
		deleteDone <- err
	}()
	<-deleteStarted
	select {
	case err := <-deleteDone:
		t.Fatalf("Delete returned before cleanup completed: %v", err)
	case <-time.After(20 * time.Millisecond):
	}
	require.Zero(t, deleteCalls.Load())
	release()
	select {
	case err := <-waitDone:
		require.NoError(t, err)
	case <-ctx.Done():
		t.Fatal("waiter did not complete")
	}
	select {
	case err := <-deleteDone:
		require.NoError(t, err)
	case <-ctx.Done():
		t.Fatal("Delete did not complete after cleanup")
	}
	require.EqualValues(t, 1, stopCalls.Load())
	require.EqualValues(t, 1, deleteCalls.Load())
	require.Empty(t, service.containers)
}

func TestDeleteContainerCleanupCancellation(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	waitCtx, cancelWait := context.WithCancel(ctx)
	defer cancelWait()
	cleanupStarted := make(chan struct{})
	releaseCleanup := make(chan struct{})
	var releaseOnce sync.Once
	release := func() { releaseOnce.Do(func() { close(releaseCleanup) }) }
	t.Cleanup(release)
	cleanupContextErr := make(chan error, 1)
	var stopCalls, deleteCalls atomic.Int32
	service, container := newCleanupTestService(t, &cleanupTestSandbox{
		waitProcess: func(string, string) (int32, error) { return 0, nil },
		stopContainer: func(ctx context.Context, _ string, _ bool) (vc.VCContainer, error) {
			deadline, ok := ctx.Deadline()
			require.True(t, ok)
			require.WithinDuration(t, time.Now().Add(cleanupTimeout), deadline, time.Second)
			stopCalls.Add(1)
			close(cleanupStarted)
			<-releaseCleanup
			cleanupContextErr <- ctx.Err()
			return nil, nil
		},
		deleteContainer: func(string) (vc.VCContainer, error) {
			deleteCalls.Add(1)
			return nil, nil
		},
	})
	close(container.exitIOch)
	waitDone := make(chan error, 1)
	go func() {
		_, err := wait(waitCtx, service, container, "")
		waitDone <- err
	}()
	select {
	case <-cleanupStarted:
	case <-ctx.Done():
		t.Fatal("waiter cleanup did not start")
	}
	cancelWait()
	deleteCtx, cancelDelete := context.WithCancel(ctx)
	cancelDelete()
	deleteDone := make(chan error, 1)
	go func() {
		_, err := service.Delete(deleteCtx, &taskAPI.DeleteRequest{ID: container.id})
		deleteDone <- err
	}()
	select {
	case err := <-deleteDone:
		require.ErrorIs(t, err, context.Canceled)
	case <-ctx.Done():
		t.Fatal("Delete ignored request cancellation")
	}
	require.Contains(t, service.containers, container.id)
	require.Zero(t, deleteCalls.Load())
	release()
	select {
	case err := <-waitDone:
		require.NoError(t, err)
	case <-ctx.Done():
		t.Fatal("waiter cleanup did not complete")
	}
	require.NoError(t, <-cleanupContextErr)
	_, err := service.Delete(ctx, &taskAPI.DeleteRequest{ID: container.id})
	require.NoError(t, err)
	require.EqualValues(t, 1, stopCalls.Load())
	require.EqualValues(t, 1, deleteCalls.Load())
	require.Empty(t, service.containers)
}

func TestDeleteContainerCleanupDeadline(t *testing.T) {
	service, container := newCleanupTestService(t, &cleanupTestSandbox{})
	container.status = task.Status_STOPPED
	container.cleanupDone = make(chan struct{})
	defer close(container.cleanupDone)
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
	defer cancel()
	deleteDone := make(chan error, 1)
	go func() {
		_, err := service.Delete(ctx, &taskAPI.DeleteRequest{ID: container.id})
		deleteDone <- err
	}()
	select {
	case err := <-deleteDone:
		require.ErrorIs(t, err, context.DeadlineExceeded)
	case <-time.After(time.Second):
		t.Fatal("Delete ignored the request deadline")
	}
	require.Contains(t, service.containers, container.id)
}

func TestWaitContainerCleanupFailureBeforeDelete(t *testing.T) {
	cleanupError := errors.New("waiter cleanup failed")
	stopCalls, deleteCalls := 0, 0
	service, container := newCleanupTestService(t, &cleanupTestSandbox{
		waitProcess: func(string, string) (int32, error) { return 0, nil },
		stopContainer: func(ctx context.Context, containerID string, force bool) (vc.VCContainer, error) {
			stopCalls++
			require.True(t, force)
			if stopCalls == 1 {
				return nil, cleanupError
			}
			return nil, nil
		},
		deleteContainer: func(string) (vc.VCContainer, error) {
			deleteCalls++
			return nil, nil
		},
	})
	close(container.exitIOch)
	_, err := wait(context.Background(), service, container, "")
	require.NoError(t, err)
	require.ErrorIs(t, container.cleanupErr, cleanupError)
	select {
	case <-container.cleanupDone:
	default:
		t.Fatal("cleanup failure did not signal completion")
	}
	_, err = service.Delete(context.Background(), &taskAPI.DeleteRequest{ID: container.id})
	require.NoError(t, err)
	require.Equal(t, 2, stopCalls)
	require.Equal(t, 1, deleteCalls)
	require.Empty(t, service.containers)
}

func TestDeleteContainerCleanupResult(t *testing.T) {
	cleanupError := errors.New("cleanup failed")
	retryError := errors.New("cleanup retry failed")
	deleteError := errors.New("delete failed")
	for _, testCase := range []struct {
		name        string
		cleanupErr  error
		retryErr    error
		deleteErr   error
		expectedErr error
		stopCalls   int
		deleteCalls int
	}{
		{name: "completed", deleteCalls: 1},
		{name: "already removed", cleanupErr: syscall.ENOENT, deleteErr: syscall.ENOENT, deleteCalls: 1},
		{name: "retry stop", cleanupErr: cleanupError, stopCalls: 1, deleteCalls: 1},
		{name: "retry already removed", cleanupErr: cleanupError, retryErr: syscall.ENOENT, stopCalls: 1, deleteCalls: 1},
		{name: "retry failed", cleanupErr: cleanupError, retryErr: retryError, expectedErr: retryError, stopCalls: 1},
		{name: "delete failed", deleteErr: deleteError, expectedErr: deleteError, deleteCalls: 1},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			stopCalls, deleteCalls := 0, 0
			service, container := newCleanupTestService(t, &cleanupTestSandbox{
				stopContainer: func(ctx context.Context, containerID string, force bool) (vc.VCContainer, error) {
					stopCalls++
					require.Equal(t, "workload", containerID)
					require.True(t, force)
					return nil, testCase.retryErr
				},
				deleteContainer: func(containerID string) (vc.VCContainer, error) {
					deleteCalls++
					require.Equal(t, "workload", containerID)
					return nil, testCase.deleteErr
				},
			})
			container.status = task.Status_STOPPED
			container.cleanupDone = make(chan struct{})
			container.cleanupErr = testCase.cleanupErr
			close(container.cleanupDone)
			_, err := service.Delete(context.Background(), &taskAPI.DeleteRequest{ID: container.id})
			require.ErrorIs(t, err, testCase.expectedErr)
			require.Equal(t, testCase.stopCalls, stopCalls)
			require.Equal(t, testCase.deleteCalls, deleteCalls)
			if testCase.expectedErr == nil {
				require.Empty(t, service.containers)
			} else {
				require.Contains(t, service.containers, container.id)
			}
		})
	}
}

func TestDeleteContainerWithoutCleanupWaiter(t *testing.T) {
	for _, testCase := range []struct {
		name      string
		status    task.Status
		stopCalls int
	}{
		{name: "created", status: task.Status_CREATED, stopCalls: 1},
		{name: "running", status: task.Status_RUNNING, stopCalls: 1},
		{name: "stopped", status: task.Status_STOPPED},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			stopCalls, deleteCalls := 0, 0
			service, container := newCleanupTestService(t, &cleanupTestSandbox{
				stopContainer: func(context.Context, string, bool) (vc.VCContainer, error) {
					stopCalls++
					return nil, nil
				},
				deleteContainer: func(string) (vc.VCContainer, error) {
					deleteCalls++
					return nil, nil
				},
			})
			container.status = testCase.status
			require.Nil(t, container.cleanupDone)
			ctx, cancel := context.WithTimeout(context.Background(), time.Second)
			defer cancel()
			_, err := service.Delete(ctx, &taskAPI.DeleteRequest{ID: container.id})
			require.NoError(t, err)
			require.Equal(t, testCase.stopCalls, stopCalls)
			require.Equal(t, 1, deleteCalls)
			require.Empty(t, service.containers)
		})
	}
}

func TestDeleteContainerCleanupRetryFailure(t *testing.T) {
	cleanupError := errors.New("cleanup failed")
	retryError := errors.New("cleanup retry failed")
	stopCalls, deleteCalls := 0, 0
	service, container := newCleanupTestService(t, &cleanupTestSandbox{
		stopContainer: func(context.Context, string, bool) (vc.VCContainer, error) {
			stopCalls++
			if stopCalls == 1 {
				return nil, retryError
			}
			return nil, nil
		},
		deleteContainer: func(string) (vc.VCContainer, error) {
			deleteCalls++
			return nil, nil
		},
	})
	container.status = task.Status_STOPPED
	container.cleanupDone = make(chan struct{})
	container.cleanupErr = cleanupError
	close(container.cleanupDone)
	_, err := service.Delete(context.Background(), &taskAPI.DeleteRequest{ID: container.id})
	require.ErrorIs(t, err, retryError)
	require.Contains(t, service.containers, container.id)
	require.ErrorIs(t, container.cleanupErr, cleanupError)
	require.Zero(t, deleteCalls)
	_, err = service.Delete(context.Background(), &taskAPI.DeleteRequest{ID: container.id})
	require.NoError(t, err)
	require.Equal(t, 2, stopCalls)
	require.Equal(t, 1, deleteCalls)
	require.Empty(t, service.containers)
}

func TestDeleteContainerCleanupReleasesServiceLock(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	requestCtx := &cleanupWaitingContext{Context: ctx, waiting: make(chan struct{})}
	service, container := newCleanupTestService(t, &cleanupTestSandbox{
		deleteContainer: func(string) (vc.VCContainer, error) { return nil, nil },
	})
	service.events = make(chan interface{}, 1)
	container.status = task.Status_STOPPED
	container.cleanupDone = make(chan struct{})
	var releaseOnce sync.Once
	release := func() { releaseOnce.Do(func() { close(container.cleanupDone) }) }
	t.Cleanup(release)
	deleteDone := make(chan error, 1)
	go func() {
		_, err := service.Delete(requestCtx, &taskAPI.DeleteRequest{ID: container.id})
		deleteDone <- err
	}()
	select {
	case <-requestCtx.waiting:
	case <-ctx.Done():
		t.Fatal("Delete did not reach the cleanup wait")
	}
	otherOperation := make(chan struct{})
	go func() {
		service.checkProcesses(exit{id: "other"})
		close(otherOperation)
	}()
	select {
	case <-otherOperation:
	case <-time.After(time.Second):
		t.Fatal("waiting for cleanup blocked another container's exit publication")
	}
	<-service.events
	service.events = nil
	deleteCtx, cancelDelete := context.WithCancel(ctx)
	cancelDelete()
	canceledDone := make(chan error, 1)
	go func() {
		_, err := service.Delete(deleteCtx, &taskAPI.DeleteRequest{ID: container.id})
		canceledDone <- err
	}()
	select {
	case err := <-canceledDone:
		require.ErrorIs(t, err, context.Canceled)
	case <-time.After(time.Second):
		t.Fatal("another Delete could not observe request cancellation")
	}
	release()
	select {
	case err := <-deleteDone:
		require.NoError(t, err)
	case <-ctx.Done():
		t.Fatal("Delete did not finish after cleanup")
	}
}

type cleanupWaitingContext struct {
	context.Context
	waiting chan struct{}
	once    sync.Once
}

func (ctx *cleanupWaitingContext) Done() <-chan struct{} {
	ctx.once.Do(func() { close(ctx.waiting) })
	return ctx.Context.Done()
}

func TestDeleteContainerCleanupRevalidatesContainer(t *testing.T) {
	for _, testCase := range []struct {
		name    string
		replace bool
	}{
		{name: "removed"},
		{name: "replaced", replace: true},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			defer cancel()
			requestCtx := &cleanupWaitingContext{Context: ctx, waiting: make(chan struct{})}
			service, workload := newCleanupTestService(t, &cleanupTestSandbox{})
			workload.status = task.Status_STOPPED
			workload.cleanupDone = make(chan struct{})
			var releaseOnce sync.Once
			release := func() { releaseOnce.Do(func() { close(workload.cleanupDone) }) }
			t.Cleanup(release)
			deleteDone := make(chan error, 1)
			go func() {
				_, err := service.Delete(requestCtx, &taskAPI.DeleteRequest{ID: workload.id})
				deleteDone <- err
			}()
			select {
			case <-requestCtx.waiting:
			case <-ctx.Done():
				t.Fatal("Delete did not reach the cleanup wait")
			}
			service.mu.Lock()
			delete(service.containers, workload.id)
			if testCase.replace {
				service.containers[workload.id] = &container{id: workload.id}
			}
			service.mu.Unlock()
			release()
			select {
			case err := <-deleteDone:
				require.Equal(t, codes.NotFound, status.Code(err))
			case <-ctx.Done():
				t.Fatal("Delete did not finish after cleanup")
			}
			if testCase.replace {
				require.Contains(t, service.containers, workload.id)
			}
		})
	}
}
