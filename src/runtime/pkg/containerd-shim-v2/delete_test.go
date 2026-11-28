// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"testing"
	"testing/synctest"
	"time"

	"github.com/containerd/containerd/api/events"
	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	"github.com/containerd/containerd/api/types/task"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
)

func TestWaitForContainerTeardown(t *testing.T) {
	t.Run("waits for completion", func(t *testing.T) {
		synctest.Test(t, func(t *testing.T) {
			done := make(chan struct{})
			go func() {
				time.Sleep(time.Second)
				close(done)
			}()

			start := time.Now()
			require.NoError(t, waitForContainerTeardown(t.Context(), done))
			require.Equal(t, time.Second, time.Since(start))
		})
	})

	t.Run("times out", func(t *testing.T) {
		synctest.Test(t, func(t *testing.T) {
			start := time.Now()
			require.ErrorIs(t, waitForContainerTeardown(t.Context(), make(chan struct{})), context.DeadlineExceeded)
			require.Equal(t, containerTeardownTimeout, time.Since(start))
		})
	})

	t.Run("prefers completed teardown to canceled request", func(t *testing.T) {
		ctx, cancel := context.WithCancel(t.Context())
		cancel()
		done := make(chan struct{})
		close(done)

		require.NoError(t, waitForContainerTeardown(ctx, done))
	})
}

func TestDeleteContinuesCleanupAfterTeardownWaitEnds(t *testing.T) {
	for _, tc := range []struct {
		name            string
		cancelRPC       bool
		expectedElapsed time.Duration
	}{
		{name: "canceled request", cancelRPC: true},
		{name: "teardown timeout", expectedElapsed: containerTeardownTimeout},
	} {
		t.Run(tc.name, func(t *testing.T) {
			synctest.Test(t, func(t *testing.T) {
				ctx, cancel := context.WithCancel(t.Context())
				defer cancel()
				if tc.cancelRPC {
					cancel()
				}

				c := &container{
					id:           testContainerID,
					cType:        vc.PodContainer,
					status:       task.Status_STOPPED,
					spec:         &specs.Spec{},
					teardownDone: make(chan struct{}),
				}
				s := &service{
					rootCtx:    t.Context(),
					sandbox:    &vcmock.Sandbox{MockID: testSandboxID},
					containers: map[string]*container{c.id: c},
					events:     make(chan interface{}, 1),
				}

				start := time.Now()
				_, err := s.Delete(ctx, &taskAPI.DeleteRequest{ID: c.id})
				require.NoError(t, err)
				require.Equal(t, tc.expectedElapsed, time.Since(start))
				require.NotContains(t, s.containers, c.id)
				require.Len(t, s.events, 1)
				require.IsType(t, &events.TaskDelete{}, <-s.events)
			})
		})
	}
}

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
