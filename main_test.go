// Copyright 2018 Intel Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"io"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestInitLogger(t *testing.T) {
	origStdout := os.Stdout
	origStderr := os.Stderr

	r, w, err := os.Pipe()
	assert.Nil(t, err, "Could not create the pipe: %v", err)
	os.Stdout = w
	os.Stderr = w
	defer func() {
		r.Close()
		w.Close()
		os.Stdout = origStdout
		os.Stderr = origStderr
	}()

	testOutString := "Foo Bar"
	initLogger("debug", "container-id", "exec-id")
	logger().Info(testOutString)

	outC := make(chan string)
	go func() {
		var buf bytes.Buffer
		io.Copy(&buf, r)
		outC <- buf.String()
	}()

	w.Close()
	out := <-outC
	assert.Equal(t, out, "", "Expecting %q to be empty", out)
}
