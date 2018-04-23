// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"reflect"
	goruntime "runtime"
	"sort"
	"strings"
	"syscall"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestSignalFatalSignal(t *testing.T) {
	assert := assert.New(t)

	for sig, fatal := range handledSignalsMap {
		result := nonFatalSignal(sig)
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
		result := fatalSignal(sig)
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

	got := handledSignals()

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
		result := nonFatalSignal(sig)
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

	result := fatalSignal(sig)
	assert.False(result)
}

func TestSignalNonFatalSignalInvalidSignal(t *testing.T) {
	assert := assert.New(t)

	sig := syscall.SIGXCPU

	result := nonFatalSignal(sig)
	assert.False(result)
}

func TestSignalBacktrace(t *testing.T) {
	assert := assert.New(t)

	// create buffer to save logger output
	buf := &bytes.Buffer{}

	savedOut := kataLog.Logger.Out
	defer func() {
		kataLog.Logger.Out = savedOut
	}()

	// capture output to buffer
	kataLog.Logger.Out = buf

	// determine name of *this* function
	pc := make([]uintptr, 1)
	goruntime.Callers(1, pc)
	fn := goruntime.FuncForPC(pc[0])
	name := fn.Name()

	backtrace()

	b := buf.String()

	// very basic tests to check if a backtrace was produced
	assert.True(strings.Contains(b, "contention:"))
	assert.True(strings.Contains(b, `"level":"error"`))
	assert.True(strings.Contains(b, name))
}
