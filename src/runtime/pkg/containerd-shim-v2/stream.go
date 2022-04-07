// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"io"
	"sync"
	"syscall"

	"github.com/containerd/fifo"
	"github.com/sirupsen/logrus"
)

// The buffer size used to specify the buffer for IO streams copy
const bufSize = 32 << 10

var (
	bufPool = sync.Pool{
		New: func() interface{} {
			buffer := make([]byte, bufSize)
			return &buffer
		},
	}
)

type ttyIO struct {
	Stdin  io.ReadCloser
	Stdout io.Writer
	Stderr io.Writer
}

func (tty *ttyIO) close() {

	if tty.Stdin != nil {
		tty.Stdin.Close()
		tty.Stdin = nil
	}
	cf := func(w io.Writer) {
		if w == nil {
			return
		}
		if c, ok := w.(io.WriteCloser); ok {
			c.Close()
		}
	}
	cf(tty.Stdout)
	cf(tty.Stderr)
}

func newTtyIO(ctx context.Context, stdin, stdout, stderr string, console bool) (*ttyIO, error) {
	var in io.ReadCloser
	var outw io.Writer
	var errw io.Writer
	var err error

	if stdin != "" {
		in, err = fifo.OpenFifo(ctx, stdin, syscall.O_RDONLY|syscall.O_NONBLOCK, 0)
		if err != nil {
			return nil, err
		}
	}

	if stdout != "" {
		outw, err = fifo.OpenFifo(ctx, stdout, syscall.O_RDWR, 0)
		if err != nil {
			return nil, err
		}
	}

	if !console && stderr != "" {
		errw, err = fifo.OpenFifo(ctx, stderr, syscall.O_RDWR, 0)
		if err != nil {
			return nil, err
		}
	}

	ttyIO := &ttyIO{
		Stdin:  in,
		Stdout: outw,
		Stderr: errw,
	}

	return ttyIO, nil
}

func ioCopy(shimLog *logrus.Entry, exitch, stdinCloser chan struct{}, tty *ttyIO, stdinPipe io.WriteCloser, stdoutPipe, stderrPipe io.Reader) {
	var wg sync.WaitGroup

	if tty.Stdin != nil {
		wg.Add(1)
		go func() {
			shimLog.Debug("stdin io stream copy started")
			p := bufPool.Get().(*[]byte)
			defer bufPool.Put(p)
			io.CopyBuffer(stdinPipe, tty.Stdin, *p)
			// notify that we can close process's io safely.
			close(stdinCloser)
			wg.Done()
			shimLog.Debug("stdin io stream copy exited")
		}()
	}

	if tty.Stdout != nil {
		wg.Add(1)

		go func() {
			shimLog.Debug("stdout io stream copy started")
			p := bufPool.Get().(*[]byte)
			defer bufPool.Put(p)
			io.CopyBuffer(tty.Stdout, stdoutPipe, *p)
			wg.Done()
			if tty.Stdin != nil {
				// close stdin to make the other routine stop
				tty.Stdin.Close()
			}
			shimLog.Debug("stdout io stream copy exited")
		}()
	}

	if tty.Stderr != nil && stderrPipe != nil {
		wg.Add(1)
		go func() {
			shimLog.Debug("stderr io stream copy started")
			p := bufPool.Get().(*[]byte)
			defer bufPool.Put(p)
			io.CopyBuffer(tty.Stderr, stderrPipe, *p)
			wg.Done()
			shimLog.Debug("stderr io stream copy exited")
		}()
	}

	wg.Wait()
	tty.close()
	close(exitch)
	shimLog.Debug("all io stream copy goroutines exited")
}
