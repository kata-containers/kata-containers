// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"testing"
	"time"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	"github.com/stretchr/testify/assert"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
)

// TestWaitTeardownOrdering covers which side of the container exit the
// sandbox teardown lands on.
func TestWaitTeardownOrdering(t *testing.T) {
	for _, tc := range []struct {
		name                string
		hasPhysicalEndpoint bool
		// Whether the exit is expected to have been published by the time
		// Stop() and Delete() respectively run.
		exitSeenByStop   bool
		exitSeenByDelete bool
	}{
		{
			name:                "a passed-through netdev is restored before the exit",
			hasPhysicalEndpoint: true,
			exitSeenByStop:      false,
			exitSeenByDelete:    true,
		},
		{
			name:                "everything else publishes the exit first",
			hasPhysicalEndpoint: false,
			exitSeenByStop:      true,
			exitSeenByDelete:    true,
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			assert := assert.New(t)

			s := &service{
				id:         testSandboxID,
				containers: make(map[string]*container),
				ec:         make(chan exit, 1),
			}

			c, err := newContainer(s, &taskAPI.CreateTaskRequest{ID: testSandboxID}, vc.PodSandbox, nil, false)
			assert.NoError(err)
			s.containers[testSandboxID] = c

			var (
				stopped, deleted bool
				exitSeenByStop   bool
				exitSeenByDelete bool
			)

			// c.exitCh is buffered, so a published exit is still sitting
			// in it while the teardown runs.
			exitPublished := func() bool { return len(c.exitCh) == 1 }

			s.sandbox = &vcmock.Sandbox{
				MockID: testSandboxID,
				WaitProcessFunc: func(containerID, processID string) (int32, error) {
					return 0, nil
				},
				HasPhysicalEndpointFunc: func() bool {
					return tc.hasPhysicalEndpoint
				},
				StopFunc: func(force bool) error {
					stopped = true
					exitSeenByStop = exitPublished()
					return nil
				},
				DeleteFunc: func() error {
					deleted = true
					exitSeenByDelete = exitPublished()
					return nil
				},
			}

			// wait() starts out waiting for the container's io streams.
			close(c.exitIOch)

			ret, err := wait(context.Background(), s, c, "")
			assert.NoError(err)
			assert.Equal(int32(0), ret)

			assert.True(stopped, "the sandbox was never stopped")
			assert.True(deleted, "the sandbox was never deleted")
			assert.Equal(tc.exitSeenByStop, exitSeenByStop, "exit published before Stop()")
			assert.Equal(tc.exitSeenByDelete, exitSeenByDelete, "exit published before Delete()")

			// Either way the exit is published by the time wait() is done.
			select {
			case code := <-c.exitCh:
				assert.Equal(uint32(0), code)
			case <-time.After(time.Second):
				t.Fatal("the container exit was never published")
			}
		})
	}
}
