// Copyright 2015 The rkt Authors
// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

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

	file, err := os.OpenFile(source, os.O_CREATE, mountPerm)
	assert.NoError(err)
	defer file.Close()

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
	defer os.Remove(source)

	err = ensureDestinationExists(source, dest)
	assert.NoError(err)
}

func TestEnsureDestinationExistsSuccessfulSrcFile(t *testing.T) {
	source := filepath.Join(testDir, "fooFileSrc")
	dest := filepath.Join(testDir, "fooFileDest")
	os.Remove(source)
	os.Remove(dest)
	assert := assert.New(t)

	file, err := os.OpenFile(source, os.O_CREATE, mountPerm)
	assert.NoError(err)
	defer file.Close()

	err = ensureDestinationExists(source, dest)
	assert.NoError(err)
}
