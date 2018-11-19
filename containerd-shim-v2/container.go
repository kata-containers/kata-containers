// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"sync"
	"time"

	"github.com/containerd/containerd/api/types/task"
	"github.com/containerd/containerd/errdefs"
	taskAPI "github.com/containerd/containerd/runtime/v2/task"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
)

type container struct {
	s        *service
	ttyio    *ttyIO
	spec     *oci.CompatOCISpec
	time     time.Time
	execs    map[string]*exec
	exitIOch chan struct{}
	exitCh   chan uint32
	id       string
	stdin    string
	stdout   string
	stderr   string
	bundle   string
	cType    vc.ContainerType
	mu       sync.Mutex
	exit     uint32
	status   task.Status
	terminal bool
}

func newContainer(s *service, r *taskAPI.CreateTaskRequest, containerType vc.ContainerType, spec *oci.CompatOCISpec) (*container, error) {
	if r == nil {
		return nil, errdefs.ToGRPCf(errdefs.ErrInvalidArgument, " CreateTaskRequest points to nil")
	}

	// in order to avoid deferencing a nil pointer in test
	if spec == nil {
		spec = &oci.CompatOCISpec{}
	}

	c := &container{
		s:        s,
		spec:     spec,
		id:       r.ID,
		bundle:   r.Bundle,
		stdin:    r.Stdin,
		stdout:   r.Stdout,
		stderr:   r.Stderr,
		terminal: r.Terminal,
		cType:    containerType,
		execs:    make(map[string]*exec),
		status:   task.StatusCreated,
		exitIOch: make(chan struct{}),
		exitCh:   make(chan uint32, 1),
		time:     time.Now(),
	}
	return c, nil
}
