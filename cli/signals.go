// Copyright 2018 Intel Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"fmt"
	"os/signal"
	"runtime/pprof"
	"strings"
	"syscall"
)

// List of fatal signals
var sigFatal = map[syscall.Signal]bool{
	syscall.SIGABRT:   true,
	syscall.SIGBUS:    true,
	syscall.SIGILL:    true,
	syscall.SIGQUIT:   true,
	syscall.SIGSEGV:   true,
	syscall.SIGSTKFLT: true,
	syscall.SIGSYS:    true,
	syscall.SIGTRAP:   true,
}

func handlePanic() {
	r := recover()

	if r != nil {
		msg := fmt.Sprintf("%s", r)
		kataLog.WithField("panic", msg).Error("fatal error")

		die()
	}
}

func backtrace() {
	profiles := pprof.Profiles()

	buf := &bytes.Buffer{}

	for _, p := range profiles {
		// The magic number requests a full stacktrace. See
		// https://golang.org/pkg/runtime/pprof/#Profile.WriteTo.
		pprof.Lookup(p.Name()).WriteTo(buf, 2)
	}

	for _, line := range strings.Split(buf.String(), "\n") {
		kataLog.Error(line)
	}
}

func fatalSignal(sig syscall.Signal) bool {
	return sigFatal[sig]
}

func fatalSignals() []syscall.Signal {
	var signals []syscall.Signal

	for sig := range sigFatal {
		signals = append(signals, sig)

	}

	return signals
}

func die() {
	backtrace()

	if crashOnError {
		signal.Reset(syscall.SIGABRT)
		syscall.Kill(0, syscall.SIGABRT)
	}

	exit(1)
}
