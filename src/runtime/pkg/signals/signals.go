// Copyright 2018 Intel Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package signals

import (
	"bytes"
	"fmt"
	"os"
	"os/signal"
	"runtime/pprof"
	"strings"
	"syscall"

	"github.com/sirupsen/logrus"
)

var signalLog = logrus.WithField("default-signal-logger", true)

// CrashOnError causes a coredump to be produced when an internal error occurs
// or a fatal signal is received.
var CrashOnError = false

// DieCb is the callback function type that needs to be defined for every call
// into the Die() function. This callback will be run as the first function of
// the Die() implementation.
type DieCb func()

// SetLogger sets the custom logger to be used by this package. If not called,
// the package will create its own logger.
func SetLogger(logger *logrus.Entry) {
	signalLog = logger
}

// HandlePanic writes a message to the logger and then calls Die().
func HandlePanic(dieCb DieCb) {
	r := recover()

	if r != nil {
		msg := fmt.Sprintf("%s", r)
		signalLog.WithField("panic", msg).Error("fatal error")

		Die(dieCb)
	}
}

// Backtrace writes a multi-line backtrace to the logger.
func Backtrace() {
	profiles := pprof.Profiles()

	buf := &bytes.Buffer{}

	for _, p := range profiles {
		// The magic number requests a full stacktrace. See
		// https://golang.org/pkg/runtime/pprof/#Profile.WriteTo.
		pprof.Lookup(p.Name()).WriteTo(buf, 2)
	}

	for _, line := range strings.Split(buf.String(), "\n") {
		signalLog.Error(line)
	}
}

// FatalSignal returns true if the specified signal should cause the program
// to abort.
func FatalSignal(sig syscall.Signal) bool {
	s, exists := handledSignalsMap[sig]
	if !exists {
		return false
	}

	return s
}

// NonFatalSignal returns true if the specified signal should simply cause the
// program to Backtrace() but continue running.
func NonFatalSignal(sig syscall.Signal) bool {
	s, exists := handledSignalsMap[sig]
	if !exists {
		return false
	}

	return !s
}

// HandledSignals returns a list of signals the package can deal with.
func HandledSignals() []syscall.Signal {
	var signals []syscall.Signal

	for sig := range handledSignalsMap {
		signals = append(signals, sig)
	}

	return signals
}

// Die causes a backtrace to be produced. If CrashOnError is set a coredump
// will be produced, else the program will exit.
func Die(dieCb DieCb) {
	dieCb()

	Backtrace()

	if CrashOnError {
		signal.Reset(syscall.SIGABRT)
		syscall.Kill(0, syscall.SIGABRT)
	}

	os.Exit(1)
}
