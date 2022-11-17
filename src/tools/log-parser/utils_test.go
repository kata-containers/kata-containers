//
// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"os"
	"path"
	"path/filepath"
	"syscall"
	"testing"

	"github.com/stretchr/testify/assert"
)

const (
	testDirMode  = os.FileMode(0750)
	testFileMode = os.FileMode(0640)
)

func TestUtilsResolvePathEmptyPath(t *testing.T) {
	assert := assert.New(t)

	_, err := resolvePath("")
	assert.Error(err)
}

func TestUtilsResolvePathValidPath(t *testing.T) {
	assert := assert.New(t)

	dir, err := os.MkdirTemp("", "")
	if err != nil {
		t.Fatal(err)
	}

	defer func() {
		err = os.RemoveAll(dir)
		assert.NoError(err)
	}()

	target := path.Join(dir, "target")
	linkDir := path.Join(dir, "a/b/c")
	linkFile := path.Join(linkDir, "link")

	err = createEmptyFile(target)
	assert.NoError(err)

	absolute, err := filepath.Abs(target)
	assert.NoError(err)

	resolvedTarget, err := filepath.EvalSymlinks(absolute)
	assert.NoError(err)

	err = os.MkdirAll(linkDir, testDirMode)
	assert.NoError(err)

	err = syscall.Symlink(target, linkFile)
	assert.NoError(err)

	resolvedLink, err := resolvePath(linkFile)
	assert.NoError(err)

	assert.Equal(resolvedTarget, resolvedLink)
}

func TestUtilsResolvePathENOENT(t *testing.T) {
	assert := assert.New(t)

	dir, err := os.MkdirTemp("", "")
	if err != nil {
		t.Fatal(err)
	}

	target := path.Join(dir, "target")
	linkDir := path.Join(dir, "a/b/c")
	linkFile := path.Join(linkDir, "link")

	err = createEmptyFile(target)
	assert.NoError(err)

	err = os.MkdirAll(linkDir, testDirMode)
	assert.NoError(err)

	err = syscall.Symlink(target, linkFile)
	assert.NoError(err)

	cwd, err := os.Getwd()
	assert.NoError(err)

	defer func() {
		err = os.Chdir(cwd)
		assert.NoError(err)
	}()

	err = os.Chdir(dir)
	assert.NoError(err)

	err = os.RemoveAll(dir)
	assert.NoError(err)

	_, err = resolvePath(filepath.Base(linkFile))
	assert.Error(err)
}
