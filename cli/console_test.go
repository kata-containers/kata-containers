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
	"io/ioutil"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestConsoleFromFile(t *testing.T) {
	assert := assert.New(t)

	console := ConsoleFromFile(os.Stdout)

	assert.NotNil(console.File(), "console file is nil")
}

func TestNewConsole(t *testing.T) {
	assert := assert.New(t)

	console, err := newConsole()
	assert.NoError(err, "failed to create a new console: %s", err)
	defer console.Close()

	assert.NotEmpty(console.Path(), "console path is empty")

	assert.NotNil(console.File(), "console file is nil")
}

func TestIsTerminal(t *testing.T) {
	assert := assert.New(t)

	var fd uintptr = 4
	assert.False(isTerminal(fd), "Fd %d is not a terminal", fd)

	console, err := newConsole()
	assert.NoError(err, "failed to create a new console: %s", err)
	defer console.Close()

	fd = console.File().Fd()
	assert.True(isTerminal(fd), "Fd %d is a terminal", fd)
}

func TestReadWrite(t *testing.T) {
	assert := assert.New(t)

	// write operation
	f, err := ioutil.TempFile(os.TempDir(), ".tty")
	assert.NoError(err, "failed to create a temporal file")
	defer os.Remove(f.Name())

	console := ConsoleFromFile(f)
	assert.NotNil(console)
	defer console.Close()

	msgWrite := "hello"
	l, err := console.Write([]byte(msgWrite))
	assert.NoError(err, "failed to write message: %s", msgWrite)
	assert.Equal(len(msgWrite), l)

	console.master.Sync()
	console.master.Seek(0, 0)

	// Read operation
	msgRead := make([]byte, len(msgWrite))
	l, err = console.Read(msgRead)
	assert.NoError(err, "failed to read message: %s", msgWrite)
	assert.Equal(len(msgWrite), l)
	assert.Equal(msgWrite, string(msgRead))
}

func TestNewConsoleFail(t *testing.T) {
	assert := assert.New(t)

	orgPtmxPath := ptmxPath
	defer func() { ptmxPath = orgPtmxPath }()

	// OpenFile failure
	ptmxPath = "/this/file/does/not/exist"
	c, err := newConsole()
	assert.Error(err)
	assert.Nil(c)

	// saneTerminal failure
	f, err := ioutil.TempFile("", "")
	assert.NoError(err)
	assert.NoError(f.Close())
	defer os.Remove(f.Name())
	ptmxPath = f.Name()
	c, err = newConsole()
	assert.Error(err)
	assert.Nil(c)
}

func TestConsoleClose(t *testing.T) {
	assert := assert.New(t)

	// nil master
	c := &Console{}
	assert.NoError(c.Close())

	f, err := ioutil.TempFile("", "")
	assert.NoError(err)
	defer os.Remove(f.Name())

	c.master = f
	assert.NoError(c.Close())
}

func TestConsolePtsnameFail(t *testing.T) {
	assert := assert.New(t)

	pts, err := ptsname(nil)
	assert.Error(err)
	assert.Empty(pts)
}
