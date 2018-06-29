// Copyright 2018 Intel Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"fmt"
	"io"
	"io/ioutil"
	"os"
	"regexp"
	"strings"
	"testing"

	"github.com/sirupsen/logrus"
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
	initLogger("debug", "container-id", "exec-id", logrus.Fields{}, ioutil.Discard)
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

func TestInitLoggerFields(t *testing.T) {
	assert := assert.New(t)

	buf := &bytes.Buffer{}

	announceFields := logrus.Fields{
		"foo":      "bar",
		"A":        "B",
		"sausages": "yummy",
	}

	initLogger("debug", "container-id", "exec-id", announceFields, buf)

	line := buf.String()

	assert.True(strings.Contains(line, "level=info"))
	assert.True(strings.Contains(line, "msg=announce"))
	assert.True(strings.Contains(line, "container=container-id"))
	assert.True(strings.Contains(line, "exec-id=exec-id"))
	assert.True(strings.Contains(line, "name="+shimName))
	assert.True(strings.Contains(line, "source=shim"))

	pidPattern := regexp.MustCompile(`pid=\d+`)
	matches := pidPattern.FindAllString(line, -1)
	assert.NotNil(matches)

	for k, v := range announceFields {
		assert.True(strings.Contains(line, fmt.Sprintf("%s=%s", k, v)))

	}
}
