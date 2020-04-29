// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

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
