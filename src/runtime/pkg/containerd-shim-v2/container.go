// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"io"
	"time"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	"github.com/containerd/containerd/api/types/task"
	"github.com/containerd/containerd/errdefs"
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
	exitCh      chan uint32
	id          string
	stdin       string
	stdout      string
	stderr      string
	bundle      string
	cType       vc.ContainerType
	exit        uint32
	status      task.Status
	terminal    bool
	mounted     bool
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
		status:      task.Status_CREATED,
		exitIOch:    make(chan struct{}),
		exitCh:      make(chan uint32, 1),
		stdinCloser: make(chan struct{}),
		mounted:     mounted,
	}
	return c, nil
}
