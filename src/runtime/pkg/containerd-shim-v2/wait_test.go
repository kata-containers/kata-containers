// Copyright (c) 2026 Red Hat, Inc
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

// TestWaitClosesTeardownDone verifies that wait() closes c.teardownDone
// only after the post-exit teardown finished, for both the regular-container
// branch (StopContainer) and the sandbox-container branch (sandbox
// Stop/Delete inside teardownOnce).  deleteContainer relies on this to wait
// out the window between the early TaskExit publication and the
// virtcontainers state flip to StateStopped.
func TestWaitClosesTeardownDone(t *testing.T) {
	type testData struct {
		name  string
		cType vc.ContainerType
	}

	data := []testData{
		{
			name:  "regular container",
			cType: vc.PodContainer,
		},
		{
			name:  "sandbox container",
			cType: vc.PodSandbox,
		},
	}

	for _, d := range data {
		t.Run(d.name, func(t *testing.T) {
			assert := assert.New(t)

			sandbox := &vcmock.Sandbox{
				MockID: testSandboxID,
			}

			// entered is signalled once the teardown's StopContainer call
			// is in flight; release lets it finish.  Only used by the
			// regular-container branch.
			entered := make(chan struct{})
			release := make(chan struct{})
			sandbox.StopContainerFunc = func(contID string, force bool) (vc.VCContainer, error) {
				close(entered)
				<-release
				return &vcmock.Container{}, nil
			}

			s := &service{
				id:         testSandboxID,
				sandbox:    sandbox,
				containers: make(map[string]*container),
				// ec is buffered, so the cReap goroutine spawned by
				// wait() does not leak on an undrained channel.
				ec: make(chan exit, bufferSize),
			}

			reqCreate := &taskAPI.CreateTaskRequest{
				ID: testContainerID,
			}
			c, err := newContainer(s, reqCreate, d.cType, nil, false)
			assert.NoError(err)
			s.containers[testContainerID] = c

			waitReturned := make(chan struct{})
			ctx := context.Background()
			close(c.exitIOch)
			go func() {
				_, _ = wait(ctx, s, c, "")
				close(waitReturned)
			}()

			// teardownDone must stay open while the teardown is in flight.
			teardownStillOpen := func() bool {
				select {
				case <-c.teardownDone:
					return false
				default:
					return true
				}
			}

			if d.cType == vc.PodContainer {
				// Wait until the teardown's StopContainer is blocked on
				// release: the exit has been published (TaskExit is out)
				// but the virtcontainers state flip has not happened yet.
				select {
				case <-entered:
				case <-time.After(5 * time.Second):
					t.Fatal("teardown StopContainer was never called")
				}

				assert.True(teardownStillOpen(), "teardownDone closed while the teardown was still in flight")

				// Finish the teardown: teardownDone must close and wait()
				// must return.
				close(release)
			}

			select {
			case <-c.teardownDone:
			case <-time.After(5 * time.Second):
				t.Fatal("teardownDone was not closed after the teardown finished")
			}

			select {
			case <-waitReturned:
			case <-time.After(5 * time.Second):
				t.Fatal("wait did not return after the teardown finished")
			}
		})
	}
}
