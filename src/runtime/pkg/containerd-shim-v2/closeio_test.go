// SPDX-License-Identifier: Apache-2.0

package containerdshim

import (
	"bytes"
	"context"
	"io"
	"os"
	"path/filepath"
	"syscall"
	"testing"
	"time"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/require"
)

func makeFIFO(t *testing.T, name string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), name)
	require.NoError(t, syscall.Mkfifo(path, 0600))
	return path
}

func awaitIO(t *testing.T, done <-chan struct{}) {
	t.Helper()
	select {
	case <-done:
	case <-time.After(5 * time.Second):
		t.Fatal("IO worker did not complete")
	}
}

func awaitIOError(t *testing.T, result <-chan error) error {
	t.Helper()
	select {
	case err := <-result:
		return err
	case <-time.After(5 * time.Second):
		t.Fatal("IO operation did not complete")
		return nil
	}
}

type testStdin struct {
	bytes.Buffer
}

func (*testStdin) Close() error { return nil }

func fifoTestProcess(t *testing.T, path string, guest io.WriteCloser, execID string) (*service, *taskAPI.CloseIORequest, *ttyIO, <-chan struct{}) {
	t.Helper()
	s, err := newService("fifo-test")
	require.NoError(t, err)
	t.Cleanup(s.cancel)
	s.rootCtx = context.Background()
	s.sandbox = &vcmock.Sandbox{MockID: "fifo-test"}
	c, err := newContainer(s, &taskAPI.CreateTaskRequest{ID: "container"}, vc.PodContainer, nil, false)
	require.NoError(t, err)
	s.containers[c.id] = c
	host, err := newTtyIO(context.Background(), "", "", path, "", "", false)
	require.NoError(t, err)
	done, stdinDone := make(chan struct{}), make(chan struct{})
	if execID == "" {
		c.ttyio, c.stdinPipe, c.stdinCloser = host, guest, stdinDone
	} else {
		c.setExec(execID, &exec{
			container: c, id: execID, tty: &tty{stdin: path}, ttyio: host,
			stdinPipe: guest, stdinCloser: stdinDone, exitIOch: done,
		})
	}
	stdin := host.io.Stdin()
	go ioCopy(logrus.NewEntry(logrus.New()), done, stdinDone, host, guest, nil, nil)
	t.Cleanup(func() {
		require.NoError(t, stdin.Close())
		awaitIO(t, done)
	})
	return s, &taskAPI.CloseIORequest{ID: c.id, ExecID: execID, Stdin: true}, host, done
}

func closeTestIO(s *service, request *taskAPI.CloseIORequest) <-chan error {
	result := make(chan error, 1)
	go func() {
		_, err := s.CloseIO(context.Background(), request)
		result <- err
	}()
	return result
}

// This test only uses baseline APIs, so it can also reproduce the bug without the fix.
func TestCloseIOEarlyWriterExit(t *testing.T) {
	for _, execID := range []string{"", "exec"} {
		t.Run("process-"+execID, func(t *testing.T) {
			path := makeFIFO(t, "stdin")
			writer, err := os.OpenFile(path, syscall.O_RDWR|syscall.O_NONBLOCK, 0)
			require.NoError(t, err)
			require.NoError(t, writer.Close())
			guest := &testStdin{}
			s, request, host, done := fifoTestProcess(t, path, guest, execID)
			result := closeTestIO(s, request)
			select {
			case err := <-result:
				require.NoError(t, err)
			case <-time.After(2 * time.Second):
				locked := !s.mu.TryLock()
				if !locked {
					s.mu.Unlock()
				}
				state := make(chan error, 1)
				go func() {
					_, err := s.State(context.Background(), &taskAPI.StateRequest{ID: request.ID})
					state <- err
				}()
				stateBlocked := false
				select {
				case err := <-state:
					t.Logf("State returned while CloseIO was pending: %v", err)
				case <-time.After(100 * time.Millisecond):
					stateBlocked = true
				}
				// Explicit teardown lets the unfixed baseline fail without leaking workers.
				require.NoError(t, host.io.Stdin().Close())
				awaitIO(t, done)
				require.NoError(t, awaitIOError(t, result))
				if stateBlocked {
					require.NoError(t, awaitIOError(t, state))
				}
				t.Fatalf("CloseIO blocked after producer exit; service lock held=%v; State blocked=%v", locked, stateBlocked)
			}
			awaitIO(t, done)
			require.Zero(t, guest.Len())
			state := make(chan error, 1)
			go func() {
				_, err := s.State(context.Background(), &taskAPI.StateRequest{ID: request.ID})
				state <- err
			}()
			require.NoError(t, awaitIOError(t, state))
		})
	}
}
