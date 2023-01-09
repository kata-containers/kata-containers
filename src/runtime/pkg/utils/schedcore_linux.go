// Copyright (c) 2023 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"golang.org/x/sys/unix"
)

// Create a new sched core domain
func Create(t PidType) error {
	return unix.Prctl(unix.PR_SCHED_CORE, unix.PR_SCHED_CORE_CREATE, 0, uintptr(t), 0)
}

// ShareFrom shares the sched core domain from the provided pid
func ShareFrom(pid uint64, t PidType) error {
	return unix.Prctl(unix.PR_SCHED_CORE, unix.PR_SCHED_CORE_SHARE_FROM, uintptr(pid), uintptr(t), 0)
}
