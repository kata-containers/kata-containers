// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"sync"
	"time"

	"github.com/containerd/containerd/api/types/task"
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
