// Copyright 2018 Intel Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package signals

import "syscall"

// List of handled signals.
//
// The value is true if receiving the signal should be fatal.
var handledSignalsMap = map[syscall.Signal]bool{
	syscall.SIGABRT:   true,
	syscall.SIGBUS:    true,
	syscall.SIGILL:    true,
	syscall.SIGQUIT:   true,
	syscall.SIGSEGV:   true,
	syscall.SIGSTKFLT: true,
	syscall.SIGSYS:    true,
	syscall.SIGTRAP:   true,
	syscall.SIGUSR1:   false,
}
