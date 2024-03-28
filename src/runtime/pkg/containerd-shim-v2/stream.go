// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"io"
	"net/url"
	"sync"

	"github.com/sirupsen/logrus"
)

const (
	// The buffer size used to specify the buffer for IO streams copy
	bufSize = 32 << 10

	shimLogPluginBinary = "binary"
	shimLogPluginFifo   = "fifo"
	shimLogPluginFile   = "file"
)

var (
	bufPool = sync.Pool{
		New: func() interface{} {
			buffer := make([]byte, bufSize)
			return &buffer
		},
	}
)

type stdio struct {
	Stdin   string
	Stdout  string
	Stderr  string
	Console bool
}
type IO interface {
	io.Closer
	Stdin() io.ReadCloser
	Stdout() io.Writer
	Stderr() io.Writer
}

type ttyIO struct {
	io  IO
	raw *stdio
}

func (tty *ttyIO) close() {
	tty.io.Close()
}

// newTtyIO creates a new ttyIO struct.
// ns(namespace)/id(container ID) are used for containerd binary IO.
// containerd will pass the ns/id as ENV to the binary log driver,
// and the binary log driver will use ns/id to get the log options config file.
// for example nerdctl: https://github.com/containerd/nerdctl/blob/v0.21.0/pkg/logging/logging.go#L102
func newTtyIO(ctx context.Context, ns, id, stdin, stdout, stderr string, console bool) (*ttyIO, error) {
	var err error
	var io IO

	raw := &stdio{
		Stdin:   stdin,
		Stdout:  stdout,
		Stderr:  stderr,
		Console: console,
	}

	uri, err := url.Parse(stdout)
	if err != nil {
		return nil, fmt.Errorf("unable to parse stdout uri: %w", err)
	}

	if uri.Scheme == "" {
		uri.Scheme = "fifo"
	}

	switch uri.Scheme {
	case shimLogPluginFifo:
		io, err = newPipeIO(ctx, raw)
	case shimLogPluginBinary:
		io, err = newBinaryIO(ctx, ns, id, uri)
	case shimLogPluginFile:
		io, err = newFileIO(ctx, raw, uri)
	default:
		return nil, fmt.Errorf("unknown STDIO scheme %s", uri.Scheme)
	}

	if err != nil {
		return nil, fmt.Errorf("failed to creat io stream: %w", err)
	}

	return &ttyIO{
		io:  io,
		raw: raw,
	}, nil
}

func ioCopy(shimLog *logrus.Entry, exitch, stdinCloser chan struct{}, tty *ttyIO, stdinPipe io.WriteCloser, stdoutPipe, stderrPipe io.Reader) {
	var wg sync.WaitGroup

	if tty.io.Stdin() != nil {
		wg.Add(1)
		go func() {
			shimLog.Debug("stdin io stream copy started")
			p := bufPool.Get().(*[]byte)
			defer bufPool.Put(p)
			io.CopyBuffer(stdinPipe, tty.io.Stdin(), *p)
			// notify that we can close process's io safely.
			close(stdinCloser)
			wg.Done()
			shimLog.Debug("stdin io stream copy exited")
		}()
	}

	if tty.io.Stdout() != nil {
		wg.Add(1)

		go func() {
			shimLog.Debug("stdout io stream copy started")
			p := bufPool.Get().(*[]byte)
			defer bufPool.Put(p)
			io.CopyBuffer(tty.io.Stdout(), stdoutPipe, *p)
			if tty.io.Stdin() != nil {
				// close stdin to make the other routine stop
				tty.io.Stdin().Close()
			}
			wg.Done()
			shimLog.Debug("stdout io stream copy exited")
		}()
	}

	if tty.io.Stderr() != nil && stderrPipe != nil {
		wg.Add(1)
		go func() {
			shimLog.Debug("stderr io stream copy started")
			p := bufPool.Get().(*[]byte)
			defer bufPool.Put(p)
			io.CopyBuffer(tty.io.Stderr(), stderrPipe, *p)
			wg.Done()
			shimLog.Debug("stderr io stream copy exited")
		}()
	}

	wg.Wait()
	tty.close()
	close(exitch)
	shimLog.Debug("all io stream copy goroutines exited")
}

func wc(w io.WriteCloser) error {
	if w == nil {
		return nil
	}
	return w.Close()
}
