// Copyright 2015 The rkt Authors
// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"os"
	"path/filepath"
	"syscall"
	"testing"

	ktu "github.com/kata-containers/runtime/pkg/katatestutils"
	"github.com/stretchr/testify/assert"
)

func TestBindMountInvalidSourceSymlink(t *testing.T) {
	source := filepath.Join(testDir, "fooFile")
	os.Remove(source)

	err := bindMount(context.Background(), source, "", false)
	assert.Error(t, err)
}

func TestBindMountFailingMount(t *testing.T) {
	source := filepath.Join(testDir, "fooLink")
	fakeSource := filepath.Join(testDir, "fooFile")
	os.Remove(source)
	os.Remove(fakeSource)
	assert := assert.New(t)

	_, err := os.OpenFile(fakeSource, os.O_CREATE, mountPerm)
	assert.NoError(err)

	err = os.Symlink(fakeSource, source)
	assert.NoError(err)

	err = bindMount(context.Background(), source, "", false)
	assert.Error(err)
}

func TestBindMountSuccessful(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	source := filepath.Join(testDir, "fooDirSrc")
	dest := filepath.Join(testDir, "fooDirDest")
	syscall.Unmount(dest, 0)
	os.Remove(source)
	os.Remove(dest)

	err := os.MkdirAll(source, mountPerm)
	assert.NoError(err)

	err = os.MkdirAll(dest, mountPerm)
	assert.NoError(err)

	err = bindMount(context.Background(), source, dest, false)
	assert.NoError(err)

	syscall.Unmount(dest, 0)
}

func TestBindMountReadonlySuccessful(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	source := filepath.Join(testDir, "fooDirSrc")
	dest := filepath.Join(testDir, "fooDirDest")
	syscall.Unmount(dest, 0)
	os.Remove(source)
	os.Remove(dest)

	err := os.MkdirAll(source, mountPerm)
	assert.NoError(err)

	err = os.MkdirAll(dest, mountPerm)
	assert.NoError(err)

	err = bindMount(context.Background(), source, dest, true)
	assert.NoError(err)

	defer syscall.Unmount(dest, 0)

	// should not be able to create file in read-only mount
	destFile := filepath.Join(dest, "foo")
	_, err = os.OpenFile(destFile, os.O_CREATE, mountPerm)
	assert.Error(err)
}

func TestEnsureDestinationExistsNonExistingSource(t *testing.T) {
	err := ensureDestinationExists("", "")
	assert.Error(t, err)
}

func TestEnsureDestinationExistsWrongParentDir(t *testing.T) {
	source := filepath.Join(testDir, "fooFile")
	dest := filepath.Join(source, "fooDest")
	os.Remove(source)
	os.Remove(dest)
	assert := assert.New(t)

	_, err := os.OpenFile(source, os.O_CREATE, mountPerm)
	assert.NoError(err)

	err = ensureDestinationExists(source, dest)
	assert.Error(err)
}

func TestEnsureDestinationExistsSuccessfulSrcDir(t *testing.T) {
	source := filepath.Join(testDir, "fooDirSrc")
	dest := filepath.Join(testDir, "fooDirDest")
	os.Remove(source)
	os.Remove(dest)
	assert := assert.New(t)

	err := os.MkdirAll(source, mountPerm)
	assert.NoError(err)

	err = ensureDestinationExists(source, dest)
	assert.NoError(err)
}

func TestEnsureDestinationExistsSuccessfulSrcFile(t *testing.T) {
	source := filepath.Join(testDir, "fooDirSrc")
	dest := filepath.Join(testDir, "fooDirDest")
	os.Remove(source)
	os.Remove(dest)
	assert := assert.New(t)

	_, err := os.OpenFile(source, os.O_CREATE, mountPerm)
	assert.NoError(err)

	err = ensureDestinationExists(source, dest)
	assert.NoError(err)
}
