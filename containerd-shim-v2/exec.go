// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"time"

	"github.com/containerd/containerd/api/types/task"
	vc "github.com/kata-containers/runtime/virtcontainers"
)

type exec struct {
	container *container
	cmds      *vc.Cmd
	tty       *tty
	ttyio     *ttyIO
	id        string

	exitCode int32

	status task.Status

	exitIOch chan struct{}
	exitCh   chan uint32

	exitTime time.Time
}

type tty struct {
	stdin    string
	stdout   string
	stderr   string
	height   uint32
	width    uint32
	terminal bool
}
