// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import "os"

var atexitFuncs []func()

var exitFunc = os.Exit

// atexit registers a function f that will be run when exit is called. The
// handlers so registered will be called the in reverse order of their
// registration.
func atexit(f func()) {
	atexitFuncs = append(atexitFuncs, f)
}

// exit calls all atexit handlers before exiting the process with status.
func exit(status int) {
	for i := len(atexitFuncs) - 1; i >= 0; i-- {
		f := atexitFuncs[i]
		f()
	}
	exitFunc(status)
}
