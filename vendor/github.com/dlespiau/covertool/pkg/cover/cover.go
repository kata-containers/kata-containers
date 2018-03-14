// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package cover

import (
	"flag"
	"io"
	"os"
	"strings"
	"testing"
)

// ParseAndStriptestFlags runs flag.Parse to parse the standard flags of a test
// binary and remove them from os.Args.
func ParseAndStripTestFlags() {
	// Parse the command line using the stdlib flag package so the flags defined
	// in the testing package get populated.
	flag.Parse()

	// Strip command line arguments that were for the testing package and are now
	// handled.
	var runtimeArgs []string
	for _, arg := range os.Args {
		if strings.HasPrefix(arg, "-test.") ||
			strings.HasPrefix(arg, "-httptest.") {
			continue
		}
		runtimeArgs = append(runtimeArgs, arg)
	}
	os.Args = runtimeArgs
}

type dummyTestDeps func(pat, str string) (bool, error)

func (d dummyTestDeps) MatchString(pat, str string) (bool, error)   { return false, nil }
func (d dummyTestDeps) StartCPUProfile(io.Writer) error             { return nil }
func (d dummyTestDeps) StopCPUProfile()                             {}
func (d dummyTestDeps) WriteHeapProfile(io.Writer) error            { return nil }
func (d dummyTestDeps) WriteProfileTo(string, io.Writer, int) error { return nil }
func (f dummyTestDeps) ImportPath() string                          { return "" }

// FlushProfiles flushes test profiles to disk. It works by build and executing
// a dummy list of 1 test. This is to ensure we execute the M.after() function
// (a function internal to the testing package) where all reporting (cpu, mem,
// cover, ... profiles) is flushed to disk.
func FlushProfiles() {
	// Redirect Stdout/err temporarily so the testing code doesn't output the
	// regular:
	//   PASS
	//   coverage: 21.4% of statements
	// Thanks to this, we can test the output of the instrumented binary the same
	// way the normal binary is.
	oldstdout := os.Stdout
	oldstderr := os.Stderr
	os.Stdout, _ = os.Open(os.DevNull)
	os.Stderr, _ = os.Open(os.DevNull)

	tests := []testing.InternalTest{}
	benchmarks := []testing.InternalBenchmark{}
	examples := []testing.InternalExample{}
	var f dummyTestDeps
	dummyM := testing.MainStart(f, tests, benchmarks, examples)
	dummyM.Run()

	// restore stdout/err
	os.Stdout = oldstdout
	os.Stderr = oldstderr
}
