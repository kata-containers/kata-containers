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

package main

import (
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
)

var testFoo string

func testFunc() {
	testFoo = "bar"
}

func TestExit(t *testing.T) {
	assert := assert.New(t)

	var testExitStatus int
	exitFunc = func(status int) {
		testExitStatus = status
	}

	defer func() {
		exitFunc = os.Exit
	}()

	// test with no atexit functions added.
	exit(1)
	assert.Equal(testExitStatus, 1)

	// test with a function added to the atexit list.
	atexit(testFunc)
	exit(0)
	assert.Equal(testFoo, "bar")
	assert.Equal(testExitStatus, 0)
}
