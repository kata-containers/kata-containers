// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
package errors

import (
	"fmt"
	"regexp"
	"strings"
	"testing"
)

func TestErrorContextDeferError(t *testing.T) {
	err := func() (err error) {
		defer ErrorContext(&err, "context")
		return fmt.Errorf("error")
	}()
	expected := "context\n\tCause: error"
	if err.Error() != expected {
		t.Errorf("ErrorContext error format failed:\nExpected=%q \nGot=%q", expected, err.Error())
	}
}

func TestErrorContextDeferErrorNil(t *testing.T) {
	err := func() (err error) {
		defer ErrorContext(&err, "context")
		return nil
	}()
	if err != nil {
		t.Errorf("Error should be nil, got %v", err)
	}
}

func TestErrorContextError(t *testing.T) {
	err := fmt.Errorf("error")
	ErrorContext(&err, "context")
	expected := "context\n\tCause: error"
	if err.Error() != expected {
		t.Errorf("ErrorContext error format failed:\nExpected=%q \nGot=%q", expected, err.Error())
	}
}

func TestErrorContextErrorNil(t *testing.T) {
	var err error
	ErrorContext(&err, "context")
	if err != nil {
		t.Errorf("pass a nil error does nothing:  err should be nl %s", err)
	}
}

func TestErrorContextFuncChainError(t *testing.T) {
	err := func() (err error) {
		defer ErrorContext(&err, "context1")
		return func() (err error) {
			defer ErrorContext(&err, "context2")
			return fmt.Errorf("error")
		}()
	}()
	expected := "context1\n\tCause: context2\n\tCause: error"
	if err.Error() != expected {
		t.Errorf("ErrorContext error format failed:\nExpected=%q \nGot=%q", expected, err.Error())
	}
}

func TestErrorContextFuncChainErrorNil(t *testing.T) {
	err := func() (err error) {
		defer ErrorContext(&err, "context1")
		return func() (err error) {
			defer ErrorContext(&err, "context2")
			return nil
		}()
	}()
	if err != nil {
		t.Errorf("Error should be nil, got %v", err)
	}
}

func TestErrorReportError(t *testing.T) {
	err := fmt.Errorf("cause")

	ErrorContext(&err, "context")
	report := ErrorReport(err)
	// Check if the error report is formatted correctly
	expected := `Cause: cause

Error trace:
	0. cause
	1. context

Stack:
0.`
	if !strings.HasPrefix(report.Error(), expected) {
		expected += "..."
		t.Errorf("Error report format failed:\nExpected=%q \n\n     Got=%q", expected, report)
	}

}
func TestErrorReportErrorNil(t *testing.T) {
	err := func() (err error) {
		defer ErrorContext(&err, "context")
		return nil
	}()
	if err != nil {
		t.Errorf("Error should be nil, got %v", err)
	}
	report := ErrorReport(err)
	if report != nil {
		t.Errorf("Error report should be nil, got %v", report)
	}

}

func TestReportFuncChainError(t *testing.T) {
	err := func() (err error) {
		defer ErrorContext(&err, "context1")
		return func() (err error) {
			defer ErrorContext(&err, "context2")
			return fmt.Errorf("error")
		}()
	}()
	report := ErrorReport(err)

	if report == nil {
		t.Errorf("Error report should not be nil, got %v", report)
	}

	expected := `Cause: error

Error trace:
	0. error
	1. context2
	2. context1

Stack:
0. .*
	.*/.*.go \+\d+
1. .*
	.*/.*.go \+\d+`

	match, err := regexp.MatchString(expected, report.Error())
	if err != nil {
		t.Errorf("Error matching error report: %v", err)
	}
	if !match {
		t.Errorf("Error report format failed matching with regex \nRegex=%q \n\n  Got=%q", expected, report)
	}

}

func TestReportFuncChainErrorNil(t *testing.T) {
	err := func() (err error) {
		defer ErrorContext(&err, "context1")
		return func() (err error) {
			defer ErrorContext(&err, "context2")
			return nil
		}()
	}()
	report := ErrorReport(err)
	if report != nil {
		t.Errorf("Error report should be nil, got %v", report)
	}
}
