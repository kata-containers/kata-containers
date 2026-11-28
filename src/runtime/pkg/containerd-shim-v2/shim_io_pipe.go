// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"io"
	"sync"
	"syscall"

	"github.com/containerd/fifo"
	"github.com/hashicorp/go-multierror"
)

var (
	_ IO = &pipeIO{}
)

type pipeIO struct {
	in        io.ReadCloser
	outw      io.WriteCloser
	errw      io.WriteCloser
	closeOnce sync.Once
	closeErr  error
}

func newPipeIO(ctx context.Context, stdio *stdio) (_ *pipeIO, retErr error) {
	var in io.ReadCloser
	var outw io.WriteCloser
	var errw io.WriteCloser
	defer func() {
		if retErr != nil {
			for _, c := range []io.Closer{in, outw, errw} {
				if c != nil {
					if err := c.Close(); err != nil {
						retErr = multierror.Append(retErr, err)
					}
				}
			}
		}
	}()

	if stdio.Stdin != "" {
		stdin, err := newStdinFIFO(ctx, stdio.Stdin)
		if err != nil {
			return nil, err
		}
		in = stdin
	}

	if stdio.Stdout != "" {
		out, err := fifo.OpenFifo(ctx, stdio.Stdout, syscall.O_RDWR, 0)
		if err != nil {
			return nil, err
		}
		outw = out
	}

	if !stdio.Console && stdio.Stderr != "" {
		out, err := fifo.OpenFifo(ctx, stdio.Stderr, syscall.O_RDWR, 0)
		if err != nil {
			return nil, err
		}
		errw = out
	}

	pipeIO := &pipeIO{
		in:   in,
		outw: outw,
		errw: errw,
	}

	return pipeIO, nil
}

func (pi *pipeIO) Stdin() io.ReadCloser {
	return pi.in
}

func (pi *pipeIO) Stdout() io.Writer {
	return pi.outw
}

func (pi *pipeIO) Stderr() io.Writer {
	return pi.errw
}

func (pi *pipeIO) Close() error {
	pi.closeOnce.Do(func() { pi.closeErr = pi.close() })
	return pi.closeErr
}

func (pi *pipeIO) close() error {
	var result *multierror.Error

	if pi.in != nil {
		if err := pi.in.Close(); err != nil {
			result = multierror.Append(result, fmt.Errorf("failed to close stdin: %w", err))
		}
	}

	if err := wc(pi.outw); err != nil {
		result = multierror.Append(result, fmt.Errorf("failed to close stdout: %w", err))
	}

	if err := wc(pi.errw); err != nil {
		result = multierror.Append(result, fmt.Errorf("failed to close stderr: %w", err))
	}

	return result.ErrorOrNil()
}
