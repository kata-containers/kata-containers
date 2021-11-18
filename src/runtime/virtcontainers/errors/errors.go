// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
package errors

import (
	"fmt"
	"strings"

	"github.com/pkg/errors"
)

// Interface implmented by pkg/errors
type stackTracer interface {
	StackTrace() errors.StackTrace
}

// Interface implmented by pkg/errors
type causer interface {
	Cause() error
}

// ErrorContext: Helper function for adding a context to an error.
// err: An error pointer to add the context to.
// ctx: The context to add to the error.
// if err is nil, nothing happens.
func ErrorContext(err *error, ctx string) {
	if *err == nil {
		return
	}
	if _, ok := (*err).(causer); !ok {
		*err = errors.New((*err).Error())
	}
	*err = errors.Wrap(*err, ctx+"\n\tCause")
}

// ErrorReport: Helper function to format a high level error.
// err: An error pointer to format.
// if err is nil, nothing happens.
// return a formated error which includes the stack trace and the cause.
// Format:
// Cause: <most-inner-error>
// Error trace:
// 	0. <most-inner-error>
// 	1. <...>
// 	N. <least-inner-error>
//
// Stack:
// 0. <path-to-most-inner-function>
//  <file-path>: +<line-number>
// ...
// Example:
//
// Cause: error
// Error trace:
// 	0. error
// 	1. context2
// 	2. context1
//
// Stack:
// 0. github.com/kata-containers/kata-containers/src/runtime/virtcontainers/errors.AddContext
// 	/mnt/go/src/github.com/kata-containers/kata-containers/src/runtime/virtcontainers/errors/errors.go +`
func ErrorReport(err error) error {
	if err == nil {
		return nil
	}

	// Find the most inner error cause
	cause := errors.Cause(err)

	// Show  cause in mesg
	report := fmt.Errorf("Cause: %s", cause)

	if _, ok := err.(causer); ok {
		errStr := fmt.Sprintf("%s", err)
		errSlice := strings.Split(errStr, "\tCause: ")
		errLen := len(errSlice)
		errStack := make([]string, errLen)
		for i, c := range errSlice {
			idx := errLen - 1 - i
			if c[len(c)-1] == '\n' {
				c = c[:len(c)-1]
			}
			errStack[idx] = fmt.Sprintf("%d. %s", idx, c)
		}
		errStrStack := strings.Join(errStack, "\n\t")
		report = fmt.Errorf("%s\n\nError trace:\n\t%s", report, errStrStack)
	}

	report = fmt.Errorf("%s\n\nStack:", report)
	if s, ok := errors.Cause(err).(stackTracer); ok {
		for level, f := range s.StackTrace() {
			report = fmt.Errorf("%s\n%d. %+s +%d", report, level, f, f)
		}
	} else {
		report = fmt.Errorf("%s\n\tStacktrace not found", report)
	}
	// split report into lines
	reportLines := strings.Split(fmt.Sprintf("%s", report), "\n")
	// print each line
	for _, line := range reportLines {
		LogError(line)
	}

	return report
}

var New = errors.New

var Errorf = errors.Errorf
var Wrapf = errors.Wrapf
var Wrap = errors.Wrap

// LogError is a helper function to log an error.
// args is variable list of arguments of interface{}
// usage:
// errors.LogError = logger.Error()
var LogError = func(args ...interface{}) {}
