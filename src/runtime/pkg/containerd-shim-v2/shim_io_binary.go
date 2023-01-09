// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"io"
	"net/url"
	"os"
	"syscall"
	"time"

	"golang.org/x/sys/execabs"

	"github.com/hashicorp/go-multierror"
)

const (
	binaryIOProcTermTimeout = 12 * time.Second // Give logger process solid 10 seconds for cleanup
)

var (
	_ IO = &binaryIO{}
)

// binaryIO related code is from https://github.com/containerd/containerd/blob/v1.6.6/pkg/process/io.go#L311
type binaryIO struct {
	cmd      *execabs.Cmd
	out, err *pipe
}

// https://github.com/containerd/containerd/blob/v1.6.6/pkg/process/io.go#L248
func newBinaryIO(ctx context.Context, ns, id string, uri *url.URL) (bio *binaryIO, err error) {
	var closers []func() error
	defer func() {
		if err == nil {
			return
		}
		result := multierror.Append(err)
		for _, fn := range closers {
			result = multierror.Append(result, fn())
		}
		err = multierror.Flatten(result)
	}()

	out, err := newPipe()
	if err != nil {
		return nil, fmt.Errorf("failed to create stdout pipes: %w", err)
	}
	closers = append(closers, out.Close)

	serr, err := newPipe()
	if err != nil {
		return nil, fmt.Errorf("failed to create stderr pipes: %w", err)
	}
	closers = append(closers, serr.Close)

	r, w, err := os.Pipe()
	if err != nil {
		return nil, err
	}
	closers = append(closers, r.Close, w.Close)

	cmd := newBinaryCmd(uri, id, ns)
	cmd.ExtraFiles = append(cmd.ExtraFiles, out.r, serr.r, w)
	// don't need to register this with the reaper or wait when
	// running inside a shim
	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("failed to start binary process: %w", err)
	}
	closers = append(closers, func() error { return cmd.Process.Kill() })

	// close our side of the pipe after start
	if err := w.Close(); err != nil {
		return nil, fmt.Errorf("failed to close write pipe after start: %w", err)
	}

	// wait for the logging binary to be ready
	b := make([]byte, 1)
	if _, err := r.Read(b); err != nil && err != io.EOF {
		return nil, fmt.Errorf("failed to read from logging binary: %w", err)
	}

	return &binaryIO{
		cmd: cmd,
		out: out,
		err: serr,
	}, nil
}

// newBinaryCmd returns a Cmd to be used to start a logging binary.
// The Cmd is generated from the provided uri, and the container ID and
// namespace are appended to the Cmd environment.
func newBinaryCmd(binaryURI *url.URL, id, ns string) *execabs.Cmd {
	var args []string
	for k, vs := range binaryURI.Query() {
		args = append(args, k)
		if len(vs) > 0 {
			args = append(args, vs[0])
		}
	}

	cmd := execabs.Command(binaryURI.Path, args...)

	cmd.Env = append(cmd.Env,
		"CONTAINER_ID="+id,
		"CONTAINER_NAMESPACE="+ns,
	)

	return cmd
}

func (bi *binaryIO) Stdin() io.ReadCloser {
	return nil
}

func (bi *binaryIO) Stdout() io.Writer {
	return bi.out.w
}

func (bi *binaryIO) Stderr() io.Writer {
	return bi.err.w
}

func (bi *binaryIO) Close() error {
	var (
		result *multierror.Error
	)

	for _, v := range []*pipe{bi.out, bi.err} {
		if v != nil {
			if err := v.Close(); err != nil {
				result = multierror.Append(result, err)
			}
		}
	}

	if err := bi.cancel(); err != nil {
		result = multierror.Append(result, err)
	}

	return result.ErrorOrNil()
}

func (bi *binaryIO) cancel() error {
	if bi.cmd == nil || bi.cmd.Process == nil {
		return nil
	}

	// Send SIGTERM first, so logger process has a chance to flush and exit properly
	if err := bi.cmd.Process.Signal(syscall.SIGTERM); err != nil {
		result := multierror.Append(fmt.Errorf("failed to send SIGTERM: %w", err))

		shimLog.WithError(err).Warn("failed to send SIGTERM signal, killing logging shim")

		if err := bi.cmd.Process.Kill(); err != nil {
			result = multierror.Append(result, fmt.Errorf("failed to kill process after faulty SIGTERM: %w", err))
		}

		return result.ErrorOrNil()
	}

	done := make(chan error, 1)
	go func() {
		done <- bi.cmd.Wait()
	}()

	select {
	case err := <-done:
		return err
	case <-time.After(binaryIOProcTermTimeout):
		shimLog.Warn("failed to wait for shim logger process to exit, killing")

		err := bi.cmd.Process.Kill()
		if err != nil {
			return fmt.Errorf("failed to kill shim logger process: %w", err)
		}

		return nil
	}
}

func newPipe() (*pipe, error) {
	r, w, err := os.Pipe()
	if err != nil {
		return nil, err
	}
	return &pipe{
		r: r,
		w: w,
	}, nil
}

type pipe struct {
	r *os.File
	w *os.File
}

// https://github.com/containerd/containerd/blob/v1.6.6/vendor/github.com/containerd/go-runc/io.go#L71
func (p *pipe) Close() error {
	var result *multierror.Error

	if err := p.w.Close(); err != nil {
		result = multierror.Append(result, fmt.Errorf("failed to close write pipe: %w", err))
	}

	if err := p.r.Close(); err != nil {
		result = multierror.Append(result, fmt.Errorf("failed to close read pipe: %w", err))
	}

	return multierror.Prefix(result.ErrorOrNil(), "pipe:")
}
