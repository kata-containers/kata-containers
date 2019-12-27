// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package store

import (
	"context"
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

type TestNoopStructure struct {
	Field1 string
	Field2 string
}

var rootPath = func() string {
	dir, _ := ioutil.TempDir("", "")
	return dir
}()

var expectedFilesystemData = "{\"Field1\":\"value1\",\"Field2\":\"value2\"}"

func TestStoreFilesystemStore(t *testing.T) {
	f := filesystem{}

	err := f.new(context.Background(), rootPath, "")
	defer f.delete()
	assert.Nil(t, err)

	data := TestNoopStructure{
		Field1: "value1",
		Field2: "value2",
	}

	err = f.store(State, data)
	assert.Nil(t, err)

	filesystemData, err := ioutil.ReadFile(filepath.Join(rootPath, StateFile))
	assert.Nil(t, err)
	assert.Equal(t, string(filesystemData), expectedFilesystemData)
}

func TestStoreFilesystemLoad(t *testing.T) {
	f := filesystem{}

	err := f.new(context.Background(), rootPath, "")
	defer f.delete()
	assert.Nil(t, err)

	data := TestNoopStructure{
		Field1: "value1",
		Field2: "value2",
	}

	// Store test data
	err = f.store(State, data)
	assert.Nil(t, err)

	// Load and compare
	newData := TestNoopStructure{}
	err = f.load(State, &newData)
	assert.Nil(t, err)
	assert.Equal(t, newData, data)
}

func TestStoreFilesystemDelete(t *testing.T) {
	f := filesystem{}

	err := f.new(context.Background(), rootPath, "")
	assert.Nil(t, err)

	data := TestNoopStructure{
		Field1: "value1",
		Field2: "value2",
	}

	// Store test data
	err = f.store(State, data)
	assert.Nil(t, err)

	err = f.delete()
	assert.Nil(t, err)

	_, err = os.Stat(f.path)
	assert.NotNil(t, err)
}

func TestStoreFilesystemRaw(t *testing.T) {
	f := filesystem{}

	err := f.new(context.Background(), rootPath, "")
	defer f.delete()
	assert.Nil(t, err)

	path, err := f.raw("roah")
	assert.Nil(t, err)
	assert.Equal(t, path, filesystemScheme+"://"+filepath.Join(rootPath, "raw", "roah"))
}

func TestStoreFilesystemLockShared(t *testing.T) {
	f := filesystem{}

	err := f.new(context.Background(), rootPath, "")
	defer f.delete()
	assert.Nil(t, err)

	// Take 2 shared locks
	token1, err := f.lock(Lock, false)
	assert.Nil(t, err)

	token2, err := f.lock(Lock, false)
	assert.Nil(t, err)

	err = f.unlock(Lock, token1)
	assert.Nil(t, err)

	err = f.unlock(Lock, token2)
	assert.Nil(t, err)

	err = f.unlock(Lock, token2)
	assert.NotNil(t, err)
}

func TestStoreFilesystemLockExclusive(t *testing.T) {
	f := filesystem{}

	err := f.new(context.Background(), rootPath, "")
	defer f.delete()
	assert.Nil(t, err)

	// Take 1 exclusive lock
	token, err := f.lock(Lock, true)
	assert.Nil(t, err)

	err = f.unlock(Lock, token)
	assert.Nil(t, err)

	token, err = f.lock(Lock, true)
	assert.Nil(t, err)

	err = f.unlock(Lock, token)
	assert.Nil(t, err)

	err = f.unlock(Lock, token)
	assert.NotNil(t, err)
}
