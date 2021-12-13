// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"io"
	"sync"
	"time"

	"github.com/containerd/containerd/api/types/task"
	"github.com/containerd/containerd/errdefs"
	taskAPI "github.com/containerd/containerd/runtime/v2/task"
	"github.com/opencontainers/runtime-spec/specs-go"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
)

type container struct {
	s           *service
	ttyio       *ttyIO
	spec        *specs.Spec
	exitTime    time.Time
	execs       map[string]*exec
	exitIOch    chan struct{}
	stdinPipe   io.WriteCloser
	stdinCloser chan struct{}
	exitCh      chan containerExit
	id          string
	stdin       string
	stdout      string
	stderr      string
	bundle      string
	cType       vc.ContainerType
	status      containerStatusMutex
	exit        uint32
	terminal    bool
	mounted     bool
}

type containerStatusMutex struct {
	_err    error
	_mutex  sync.Mutex
	_status task.Status
}

func (s *containerStatusMutex) Get() (task.Status, error) {
	s._mutex.Lock()
	defer s._mutex.Unlock()
	return s._status, s._err
}

func (s *containerStatusMutex) Set(status task.Status) {
	s._mutex.Lock()
	defer s._mutex.Unlock()
	s._status = status
}

func (s *containerStatusMutex) SetError(err error) {
	s._mutex.Lock()
	defer s._mutex.Unlock()
	s._err = err
}

type containerExit struct {
	err  error
	code uint32
}

func newContainer(s *service, r *taskAPI.CreateTaskRequest, containerType vc.ContainerType, spec *specs.Spec, mounted bool) (*container, error) {
	if r == nil {
		return nil, errdefs.ToGRPCf(errdefs.ErrInvalidArgument, " CreateTaskRequest points to nil")
	}

	// in order to avoid deferencing a nil pointer in test
	if spec == nil {
		spec = &specs.Spec{}
	}

	c := &container{
		s:           s,
		spec:        spec,
		id:          r.ID,
		bundle:      r.Bundle,
		stdin:       r.Stdin,
		stdout:      r.Stdout,
		stderr:      r.Stderr,
		terminal:    r.Terminal,
		cType:       containerType,
		execs:       make(map[string]*exec),
		exitIOch:    make(chan struct{}),
		exitCh:      make(chan containerExit, 1),
		stdinCloser: make(chan struct{}),
		mounted:     mounted,
	}
	c.status.Set(task.StatusCreated)
	return c, nil
}
