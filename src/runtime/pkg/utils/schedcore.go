// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"golang.org/x/sys/unix"
)

// PidType is the type of provided pid value and how it should be treated
type PidType int

const (
	pidTypePid            = 0
	pidTypeThreadGroupId  = 1
	pidTypeProcessGroupId = 2

	// Pid affects the current pid
	Pid PidType = pidTypePid
	// ThreadGroup affects all threads in the group
	ThreadGroup PidType = pidTypeThreadGroupId
	// ProcessGroup affects all processes in the group
	ProcessGroup PidType = pidTypeProcessGroupId
)

// Create a new sched core domain
func Create(t PidType) error {
	return unix.Prctl(unix.PR_SCHED_CORE, unix.PR_SCHED_CORE_CREATE, 0, uintptr(t), 0)
}

// ShareFrom shares the sched core domain from the provided pid
func ShareFrom(pid uint64, t PidType) error {
	return unix.Prctl(unix.PR_SCHED_CORE, unix.PR_SCHED_CORE_SHARE_FROM, uintptr(pid), uintptr(t), 0)
}
