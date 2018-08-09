// Copyright 2018 Intel Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"context"
	"fmt"
	"os/signal"
	"runtime/pprof"
	"strings"
	"syscall"
)

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

func handlePanic(ctx context.Context) {
	r := recover()

	if r != nil {
		msg := fmt.Sprintf("%s", r)
		kataLog.WithField("panic", msg).Error("fatal error")

		die(ctx)
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
	s, exists := handledSignalsMap[sig]
	if !exists {
		return false
	}

	return s
}

func nonFatalSignal(sig syscall.Signal) bool {
	s, exists := handledSignalsMap[sig]
	if !exists {
		return false
	}

	return !s
}

func handledSignals() []syscall.Signal {
	var signals []syscall.Signal

	for sig := range handledSignalsMap {
		signals = append(signals, sig)
	}

	return signals
}

func die(ctx context.Context) {
	stopTracing(ctx)

	backtrace()

	if crashOnError {
		signal.Reset(syscall.SIGABRT)
		syscall.Kill(0, syscall.SIGABRT)
	}

	exit(1)
}
