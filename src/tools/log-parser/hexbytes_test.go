//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"io"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNewHexByteReader(t *testing.T) {
	assert := assert.New(t)

	file := "/tmp/foo.txt"
	r := NewHexByteReader(file)
	assert.Equal(r.file, file)
	assert.Nil(r.f)
}

func TestNewHexByteReaderStdin(t *testing.T) {
	assert := assert.New(t)

	file := "-"
	r := NewHexByteReader(file)
	assert.Equal(r.file, file)
	assert.Equal(r.f, os.Stdin)
}

func TestHexByteReaderRead(t *testing.T) {
	assert := assert.New(t)

	dir, err := os.MkdirTemp("", "")
	if err != nil {
		t.Fatal(err)
	}

	defer func() {
		err = os.RemoveAll(dir)
		assert.NoError(err)
	}()

	type testData struct {
		contents    string
		result      string
		expectError bool
	}

	data := []testData{
		{"", "", true},

		// Valid
		{" ", " ", false},
		{"hello world", "hello world", false},
		{`\x00`, `\\x00`, false},
		{`\x00\x01`, `\\x00\\x01`, false},
	}

	for i, d := range data {
		file := filepath.Join(dir, "file.log")
		err := createFile(file, d.contents)
		assert.NoError(err)

		reader := NewHexByteReader(file)
		bytes, err := io.ReadAll(reader)

		if d.expectError {
			assert.Errorf(err, "test[%d]: %+v", i, d)
		} else {
			assert.NoErrorf(err, "test[%d]: %+v", i, d)
			assert.Equal([]byte(d.result), bytes)
		}

		err = os.Remove(file)
		assert.NoError(err)
	}
}
