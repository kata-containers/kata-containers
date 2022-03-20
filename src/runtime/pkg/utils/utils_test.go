// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"fmt"
	"net/http"
	"os"
	"path"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestGzipAccepted(t *testing.T) {
	assert := assert.New(t)
	testCases := []struct {
		header string
		result bool
	}{
		{
			header: "",
			result: false,
		},
		{
			header: "abc",
			result: false,
		},
		{
			header: "gzip",
			result: true,
		},
		{
			header: "deflate, gzip;q=1.0, *;q=0.5",
			result: true,
		},
	}

	h := http.Header{}

	for i := range testCases {
		tc := testCases[i]
		h[acceptEncodingHeader] = []string{tc.header}
		b := GzipAccepted(h)
		assert.Equal(tc.result, b)
	}
}

func TestEnsureDir(t *testing.T) {
	const testMode = 0755
	assert := assert.New(t)
	tmpdir := t.TempDir()

	// nolint: govet
	testCases := []struct {
		before func()
		path   string
		err    bool
		msg    string
	}{
		{
			before: nil,
			path:   "a/b/c",
			err:    true,
			msg:    "Not an absolute path",
		},
		{
			before: nil,
			path:   fmt.Sprintf("%s/abc/def", tmpdir),
			err:    false,
			msg:    "",
		},
		{
			before: nil,
			path:   fmt.Sprintf("%s/abc", tmpdir),
			err:    false,
			msg:    "",
		},
		{
			before: func() {
				err := os.MkdirAll(fmt.Sprintf("%s/abc/def", tmpdir), testMode)
				assert.NoError(err)
			},
			path: fmt.Sprintf("%s/abc/def", tmpdir),
			err:  false,
			msg:  "",
		},
		{
			before: func() {
				// create a regular file
				err := os.MkdirAll(fmt.Sprintf("%s/abc", tmpdir), testMode)
				assert.NoError(err)
				_, err = os.Create(fmt.Sprintf("%s/abc/file.txt", tmpdir))
				assert.NoError(err)
			},
			path: fmt.Sprintf("%s/abc/file.txt", tmpdir),
			err:  true,
			msg:  "Not a directory",
		},
	}

	for _, tc := range testCases {
		if tc.before != nil {
			tc.before()
		}
		err := EnsureDir(tc.path, testMode)
		if tc.err {
			assert.Contains(err.Error(), tc.msg, "error msg should contains: %s, but got %s", tc.msg, err.Error())
		} else {
			assert.Equal(err, nil, "failed for path: %s, except no error, but got %+v", tc.path, err)
		}
	}
}

func TestFirstValidExecutable(t *testing.T) {
	assert := assert.New(t)
	tmpdir := t.TempDir()

	// nolint: govet
	testCases := []struct {
		before    func()
		paths     []string
		validPath string
		isValid   bool
		msg       string
	}{
		{
			before:    nil,
			paths:     []string{"a/b/c", "c/d"},
			validPath: "",
			isValid:   false,
			msg:       "all the executables are invalid",
		},
		{
			before: func() {
				err := os.MkdirAll(path.Join(tmpdir, "a", "b"), 0755)
				assert.NoError(err)
				// create a non-executable file
				err = os.WriteFile(path.Join(tmpdir, "a", "b", "c"), []byte("test\n"), 0644)
				assert.NoError(err)
			},
			paths:     []string{path.Join(tmpdir, "a", "b", "c"), "c/d"},
			validPath: "",
			isValid:   false,
			msg:       "all the executables are invalid",
		},
		{
			before: func() {
				err := os.MkdirAll(path.Join(tmpdir, "d", "e"), 0755)
				assert.NoError(err)
				// create an executable file
				err = os.WriteFile(path.Join(tmpdir, "d", "e", "f"), []byte("test\n"), 0755)
				assert.NoError(err)
			},
			paths:     []string{path.Join(tmpdir, "d", "e", "f"), "c/d"},
			validPath: fmt.Sprintf("%s/d/e/f", tmpdir),
			isValid:   true,
			msg:       "",
		},
		{
			before: func() {
				err := os.MkdirAll(path.Join(tmpdir, "g", "h"), 0755)
				assert.NoError(err)
				// create an executable file
				err = os.WriteFile(path.Join(tmpdir, "g", "h", "i"), []byte("test\n"), 0755)
				assert.NoError(err)
			},
			paths:     []string{"c/d", path.Join(tmpdir, "g", "h", "i")},
			validPath: path.Join(tmpdir, "g", "h", "i"),
			isValid:   true,
			msg:       "",
		},
	}

	for _, tc := range testCases {
		if tc.before != nil {
			tc.before()
		}
		path, err := FirstValidExecutable(tc.paths)
		assert.Equal(tc.isValid, err == nil)
		if tc.isValid {
			assert.Equal(tc.validPath, path)
		} else {
			assert.Equal(err.Error(), tc.msg)
			assert.Equal(tc.validPath, "")
		}
	}
}
