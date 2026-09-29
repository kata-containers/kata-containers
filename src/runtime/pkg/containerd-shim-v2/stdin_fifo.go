// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0

package containerdshim

import (
	"context"
	"fmt"
	"io"
	"os"
	"sync"
	"syscall"

	"github.com/containerd/fifo"
	"golang.org/x/sys/unix"
)

// stdinFIFO pins the original FIFO and signals completion of the actual reader
// open, rather than containerd/fifo's asynchronous O_NONBLOCK wrapper creation.
type stdinFIFO struct {
	anchor *os.File
	reader io.ReadWriteCloser
	err    error
	ready  chan struct{}
	cancel context.CancelFunc

	mu        sync.Mutex
	closed    bool
	closeOnce sync.Once
	closeErr  error
}

func newStdinFIFO(ctx context.Context, path string) (*stdinFIFO, error) {
	anchor, err := os.OpenFile(path, unix.O_PATH|unix.O_CLOEXEC, 0)
	if err != nil {
		return nil, err
	}
	ctx, cancel := context.WithCancel(ctx)
	f := &stdinFIFO{anchor: anchor, ready: make(chan struct{}), cancel: cancel}
	pinnedPath := fmt.Sprintf("/proc/self/fd/%d", anchor.Fd())
	go func() {
		reader, err := fifo.OpenFifo(ctx, pinnedPath, syscall.O_RDONLY, 0)
		// On failure the library can return a typed-nil interface.
		if err == nil {
			f.reader = reader
		}
		f.err = err
		close(f.ready)
	}()
	return f, nil
}

func (f *stdinFIFO) Read(p []byte) (int, error) {
	<-f.ready
	if f.err != nil {
		return 0, f.err
	}
	return f.reader.Read(p)
}

// finishOpen supplies a temporary writer only until the real reader is open,
// leaving that reader alive to drain input.
func (f *stdinFIFO) finishOpen() error {
	f.mu.Lock()
	defer f.mu.Unlock()
	if f.closed {
		return nil
	}
	select {
	case <-f.ready:
		return f.err
	default:
	}
	writer, err := os.OpenFile(fmt.Sprintf("/proc/self/fd/%d", f.anchor.Fd()), syscall.O_RDWR|syscall.O_NONBLOCK, 0)
	if err != nil {
		return err
	}
	<-f.ready
	if err := writer.Close(); err != nil {
		return err
	}
	return f.err
}

func (f *stdinFIFO) Close() error {
	f.closeOnce.Do(func() {
		f.cancel()
		<-f.ready
		f.mu.Lock()
		defer f.mu.Unlock()
		f.closed = true
		if f.reader != nil {
			f.closeErr = f.reader.Close()
		}
		if err := f.anchor.Close(); f.closeErr == nil {
			f.closeErr = err
		}
	})
	return f.closeErr
}
