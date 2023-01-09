// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"io"
	"syscall"

	"github.com/containerd/fifo"
	"github.com/hashicorp/go-multierror"
)

var (
	_ IO = &pipeIO{}
)

type pipeIO struct {
	in   io.ReadCloser
	outw io.WriteCloser
	errw io.WriteCloser
}

func newPipeIO(ctx context.Context, stdio *stdio) (*pipeIO, error) {
	var in io.ReadCloser
	var outw io.WriteCloser
	var errw io.WriteCloser
	var err error

	if stdio.Stdin != "" {
		in, err = fifo.OpenFifo(ctx, stdio.Stdin, syscall.O_RDONLY|syscall.O_NONBLOCK, 0)
		if err != nil {
			return nil, err
		}
	}

	if stdio.Stdout != "" {
		outw, err = fifo.OpenFifo(ctx, stdio.Stdout, syscall.O_RDWR, 0)
		if err != nil {
			return nil, err
		}
	}

	if !stdio.Console && stdio.Stderr != "" {
		errw, err = fifo.OpenFifo(ctx, stdio.Stderr, syscall.O_RDWR, 0)
		if err != nil {
			return nil, err
		}
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
	var result *multierror.Error

	if pi.in != nil {
		if err := pi.in.Close(); err != nil {
			result = multierror.Append(result, fmt.Errorf("failed to close stdin: %w", err))
		}
		pi.in = nil
	}

	if err := wc(pi.outw); err != nil {
		result = multierror.Append(result, fmt.Errorf("failed to close stdout: %w", err))
	}

	if err := wc(pi.errw); err != nil {
		result = multierror.Append(result, fmt.Errorf("failed to close stderr: %w", err))
	}

	return result.ErrorOrNil()
}
