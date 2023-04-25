// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package signals

import (
	"bytes"
	"errors"
	"os"
	"os/exec"
	"reflect"
	goruntime "runtime"
	"sort"
	"strings"
	"syscall"
	"testing"

	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
)

func TestSignalFatalSignal(t *testing.T) {
	assert := assert.New(t)

	for sig, fatal := range handledSignalsMap {
		result := NonFatalSignal(sig)
		if fatal {
			assert.False(result)
		} else {
			assert.True(result)
		}
	}
}

func TestSignalHandledSignalsMap(t *testing.T) {
	assert := assert.New(t)

	for sig, fatal := range handledSignalsMap {
		result := FatalSignal(sig)
		if fatal {
			assert.True(result)
		} else {
			assert.False(result)
		}
	}
}

func TestSignalHandledSignals(t *testing.T) {
	assert := assert.New(t)

	var expected []syscall.Signal

	for sig := range handledSignalsMap {
		expected = append(expected, sig)
	}

	got := HandledSignals()

	sort.Slice(expected, func(i, j int) bool {
		return int(expected[i]) < int(expected[j])
	})

	sort.Slice(got, func(i, j int) bool {
		return int(got[i]) < int(got[j])
	})

	assert.True(reflect.DeepEqual(expected, got))
}

func TestSignalNonFatalSignal(t *testing.T) {
	assert := assert.New(t)

	for sig, fatal := range handledSignalsMap {
		result := NonFatalSignal(sig)
		if fatal {
			assert.False(result)
		} else {
			assert.True(result)
		}
	}
}

func TestSignalFatalSignalInvalidSignal(t *testing.T) {
	assert := assert.New(t)

	sig := syscall.SIGXCPU

	result := FatalSignal(sig)
	assert.False(result)
}

func TestSignalNonFatalSignalInvalidSignal(t *testing.T) {
	assert := assert.New(t)

	sig := syscall.SIGXCPU

	result := NonFatalSignal(sig)
	assert.False(result)
}

func TestSignalBacktrace(t *testing.T) {
	assert := assert.New(t)

	savedLog := signalLog
	defer func() {
		signalLog = savedLog
	}()

	signalLog = logrus.WithFields(logrus.Fields{
		"name":        "name",
		"pid":         os.Getpid(),
		"source":      "throttler",
		"test-logger": true})

	// create buffer to save logger output
	buf := &bytes.Buffer{}

	savedOut := signalLog.Logger.Out
	defer func() {
		signalLog.Logger.Out = savedOut
	}()

	// capture output to buffer
	signalLog.Logger.Out = buf

	// determine name of *this* function
	pc := make([]uintptr, 1)
	goruntime.Callers(1, pc)

	Backtrace()

	b := buf.String()

	// very basic tests to check if a backtrace was produced
	assert.True(strings.Contains(b, "contention:"))
	assert.True(strings.Contains(b, `level=error`))
}

func TestSignalHandlePanic(t *testing.T) {
	assert := assert.New(t)

	savedLog := signalLog
	defer func() {
		signalLog = savedLog
	}()

	signalLog = logrus.WithFields(logrus.Fields{
		"name":        "name",
		"pid":         os.Getpid(),
		"source":      "throttler",
		"test-logger": true})

	// Create buffer to save logger output.
	buf := &bytes.Buffer{}

	savedOut := signalLog.Logger.Out
	defer func() {
		signalLog.Logger.Out = savedOut
	}()

	// Capture output to buffer.
	signalLog.Logger.Out = buf

	HandlePanic(nil)

	b := buf.String()
	assert.True(len(b) == 0)
}

func TestSignalHandlePanicWithError(t *testing.T) {
	assert := assert.New(t)

	if os.Getenv("CALL_EXIT") != "1" {
		cmd := exec.Command(os.Args[0], "-test.run=TestSignalHandlePanicWithError")
		cmd.Env = append(os.Environ(), "CALL_EXIT=1")

		err := cmd.Run()
		assert.True(err != nil)

		exitError, ok := err.(*exec.ExitError)
		assert.True(ok)
		assert.True(exitError.ExitCode() == 1)

		return
	}

	signalLog = logrus.WithFields(logrus.Fields{
		"name":        "name",
		"pid":         os.Getpid(),
		"source":      "throttler",
		"test-logger": true})

	// Create buffer to save logger output.
	buf := &bytes.Buffer{}

	// Capture output to buffer.
	signalLog.Logger.Out = buf

	dieCallBack := func() {}
	defer HandlePanic(dieCallBack)
	e := errors.New("test-panic")
	panic(e)
}
