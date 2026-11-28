// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"os"
	"strings"
	"testing"
	"time"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	"github.com/containerd/containerd/api/types/task"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/stretchr/testify/assert"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
)

func newTestService(id string) (*service, error) {
	ctx := context.Background()

	ctx, cancel := context.WithCancel(ctx)

	s := &service{
		id:         id,
		pid:        uint32(os.Getpid()),
		ctx:        ctx,
		containers: make(map[string]*container),
		events:     make(chan interface{}, chSize),
		ec:         make(chan exit, bufferSize),
		cancel:     cancel,
	}

	return s, nil
}

func TestServiceCreate(t *testing.T) {
	const badCIDErrorPrefix = "invalid container/sandbox ID"
	const blankCIDError = "ID cannot be blank"

	assert := assert.New(t)

	_, bundleDir, _ := ktu.SetupOCIConfigFile(t)

	ctx := context.Background()

	s, err := newTestService("foo")
	assert.NoError(err)

	for i, d := range ktu.ContainerIDTestData {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		// Only consider error scenarios as we are only testing invalid CIDs here.
		if d.Valid {
			continue
		}

		task := taskAPI.CreateTaskRequest{
			ID:     d.ID,
			Bundle: bundleDir,
		}

		_, err = s.Create(ctx, &task)
		assert.Error(err, msg)

		if d.ID == "" {
			assert.Equal(err.Error(), blankCIDError, msg)
		} else {
			assert.True(strings.HasPrefix(err.Error(), badCIDErrorPrefix), msg)
		}
	}
}

// TestServiceDeleteWaitsForTeardown drives the Delete RPC end to end with
// the post-exit teardown still in flight: the RPC handler holds s.mu while
// deleteContainer waits on c.teardownDone, which must not deadlock and must
// only call into virtcontainers after the teardown finished.
func TestServiceDeleteWaitsForTeardown(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	deleted := make(chan struct{})
	sandbox.DeleteContainerFunc = func(contID string) (vc.VCContainer, error) {
		close(deleted)
		return &vcmock.Container{}, nil
	}

	s, err := newService("foo")
	assert.NoError(err)
	s.sandbox = sandbox

	reqCreate := &taskAPI.CreateTaskRequest{
		ID: testContainerID,
	}
	c, err := newContainer(s, reqCreate, vc.PodContainer, nil, false)
	assert.NoError(err)
	c.status = task.Status_STOPPED
	s.containers[testContainerID] = c

	respCh := make(chan *taskAPI.DeleteResponse, 1)
	errCh := make(chan error, 1)
	go func() {
		resp, err := s.Delete(context.Background(), &taskAPI.DeleteRequest{ID: testContainerID})
		if err != nil {
			errCh <- err
			return
		}
		respCh <- resp
	}()

	// The teardown is in flight (teardownDone open): the Delete RPC must
	// not call into virtcontainers and must not return.
	select {
	case <-deleted:
		t.Fatal("DeleteContainer called before the teardown finished")
	case resp := <-respCh:
		t.Fatalf("Delete returned before the teardown finished: %+v", resp)
	case err := <-errCh:
		t.Fatalf("Delete failed before the teardown finished: %v", err)
	case <-time.After(100 * time.Millisecond):
	}

	// Finish the teardown: the Delete RPC must complete.
	close(c.teardownDone)

	select {
	case <-deleted:
	case <-time.After(5 * time.Second):
		t.Fatal("DeleteContainer was not called after the teardown finished")
	}

	select {
	case resp := <-respCh:
		assert.Equal(c.exit, resp.ExitStatus)
	case err := <-errCh:
		t.Fatalf("Delete failed: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("Delete did not return after the teardown finished")
	}

	assert.NotContains(s.containers, testContainerID)
}
