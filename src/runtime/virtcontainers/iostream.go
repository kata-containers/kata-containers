// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"
	"io"
	"sync/atomic"
)

type iostream struct {
	sandbox   *Sandbox
	container *Container
	process   string

	// stdinClosed is set once the guest side of the process' stdin has
	// been closed.  It only gates the stdin writer: the process keeps
	// running and keeps producing output after its stdin is gone, so
	// closing stdin must not stop the stdout/stderr readers.
	//
	// This matters for exec sessions that don't ask for a stdin stream:
	// containerd then closes the process' stdin as soon as it is started
	// (the k8s WebSocket exec API hands it a stdin reader that returns
	// EOF right away), and any output produced afterwards would be lost.
	stdinClosed atomic.Bool
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
	if s.stdinClosed.Load() {
		return 0, errors.New("stream closed")
	}

	// can not pass context to Write(), so use background context
	return s.sandbox.agent.writeProcessStdin(context.Background(), s.container, s.process, data)
}

func (s *stdinStream) Close() error {
	if s.stdinClosed.Load() {
		return errors.New("stream closed")
	}

	// can not pass context to Close(), so use background context
	err := s.sandbox.agent.closeProcessStdin(context.Background(), s.container, s.process)
	if err == nil {
		s.stdinClosed.Store(true)
	}

	return err
}

func (s *stdoutStream) Read(data []byte) (n int, err error) {
	// can not pass context to Read(), so use background context
	return s.sandbox.agent.readProcessStdout(context.Background(), s.container, s.process, data)
}

func (s *stderrStream) Read(data []byte) (n int, err error) {
	// can not pass context to Read(), so use background context
	return s.sandbox.agent.readProcessStderr(context.Background(), s.container, s.process, data)
}
