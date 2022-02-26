// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package syscall

import (
	"syscall"
)

func Gettid() int {
	// There is no equivalent to a thread ID on Darwin.
	// We use the PID instead.
	return syscall.Getpid()
}
