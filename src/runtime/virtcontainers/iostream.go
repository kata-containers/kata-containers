// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"
	"io"
)

type iostream struct {
	sandbox   *Sandbox
	container *Container
	process   string
	closed    bool
}

// io.WriteCloser
type stdinStream struct {
	*iostream
}

// io.Reader
type stdoutStream struct {
	*iostream
}

// io.Reader
type stderrStream struct {
	*iostream
}

func newIOStream(s *Sandbox, c *Container, proc string) *iostream {
	return &iostream{
		sandbox:   s,
		container: c,
		process:   proc,
		closed:    false, // needed to workaround buggy structcheck
	}
}

func (s *iostream) stdin() io.WriteCloser {
	return &stdinStream{s}
}

func (s *iostream) stdout() io.Reader {
	return &stdoutStream{s}
}

func (s *iostream) stderr() io.Reader {
	return &stderrStream{s}
}

func (s *stdinStream) Write(data []byte) (n int, err error) {
	if s.closed {
		return 0, errors.New("stream closed")
	}

	// can not pass context to Write(), so use background context
	return s.sandbox.agent.writeProcessStdin(context.Background(), s.container, s.process, data)
}

func (s *stdinStream) Close() error {
	if s.closed {
		return errors.New("stream closed")
	}

	// can not pass context to Close(), so use background context
	err := s.sandbox.agent.closeProcessStdin(context.Background(), s.container, s.process)
	if err == nil {
		s.closed = true
	}

	return err
}

func (s *stdoutStream) Read(data []byte) (n int, err error) {
	if s.closed {
		return 0, errors.New("stream closed")
	}

	// can not pass context to Read(), so use background context
	return s.sandbox.agent.readProcessStdout(context.Background(), s.container, s.process, data)
}

func (s *stderrStream) Read(data []byte) (n int, err error) {
	if s.closed {
		return 0, errors.New("stream closed")
	}

	// can not pass context to Read(), so use background context
	return s.sandbox.agent.readProcessStderr(context.Background(), s.container, s.process, data)
}
