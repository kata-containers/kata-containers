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

const testSandboxID = "7f49d00d-1995-4156-8c79-5f5ab24ce138"

var sandboxDirConfig = ""
var sandboxFileConfig = ""
var sandboxDirState = ""
var sandboxDirLock = ""
var sandboxFileState = ""
var sandboxFileLock = ""
var storeRoot, storeRootDir = func() (string, string) {
	dir, _ := ioutil.TempDir("", "")
	return "file://" + dir, dir
}()
var testDir = ""
var ConfigStoragePathSaved = func() string { return "" }
var RunStoragePathSaved = func() string { return "" }

func TestNewStore(t *testing.T) {
	s, err := New(context.Background(), storeRoot)
	assert.Nil(t, err)
	assert.Equal(t, s.scheme, "file")
	assert.Equal(t, s.host, "")
	assert.Equal(t, s.path, storeRootDir)
}

func TestDeleteStore(t *testing.T) {
	s, err := New(context.Background(), storeRoot)
	assert.Nil(t, err)

	err = s.Delete()
	assert.Nil(t, err)

	// We should no longer find storeRoot
	newStore := stores.findStore(storeRoot)
	assert.Nil(t, newStore, "findStore should not have found %s", storeRoot)
}

func TestManagerAddStore(t *testing.T) {
	s, err := New(context.Background(), storeRoot)
	assert.Nil(t, err)
	defer stores.removeStore(storeRoot)

	// Positive find
	newStore := stores.findStore(storeRoot)
	assert.NotNil(t, newStore, "findStore failed")

	// Duplicate, should fail
	ns, err := stores.addStore(s)
	assert.Nil(t, err, "addStore should not failed")
	assert.Equal(t, s, ns)

	// Try with an empty URL
	sEmpty, err := New(context.Background(), storeRoot)
	assert.Nil(t, err)
	sEmpty.url = ""
	_, err = stores.addStore(sEmpty)
	assert.NotNil(t, err, "addStore should have failed on an empty store URL")

}

func TestManagerRemoveStore(t *testing.T) {
	_, err := New(context.Background(), storeRoot)
	assert.Nil(t, err)

	// Positive find
	newStore := stores.findStore(storeRoot)
	assert.NotNil(t, newStore, "findStore failed")

	// Negative removal
	stores.removeStore(storeRoot + "foobar")

	// We should still find storeRoot
	newStore = stores.findStore(storeRoot)
	assert.NotNil(t, newStore, "findStore failed")

	// Positive removal
	stores.removeStore(storeRoot)

	// We should no longer find storeRoot
	newStore = stores.findStore(storeRoot)
	assert.Nil(t, newStore, "findStore should not have found %s", storeRoot)
}

func TestManagerFindStore(t *testing.T) {
	_, err := New(context.Background(), storeRoot)
	assert.Nil(t, err)
	defer stores.removeStore(storeRoot)

	// Positive find
	newStore := stores.findStore(storeRoot)
	assert.NotNil(t, newStore, "findStore failed")

	// Negative find
	newStore = stores.findStore(storeRoot + "foobar")
	assert.Nil(t, newStore, "findStore should not have found a new store")
}

// TestMain is the common main function used by ALL the test functions
// for the store.
func TestMain(m *testing.M) {
	setup()
	rt := m.Run()
	shutdown()
	os.Exit(rt)
}

func shutdown() {
	os.RemoveAll(testDir)
	ConfigStoragePath = ConfigStoragePathSaved
	RunStoragePath = RunStoragePathSaved
}

func setup() {
	var err error
	testDir, err = ioutil.TempDir("", "store-tmp-")
	if err != nil {
		panic(err)
	}

	ConfigStoragePathSaved = ConfigStoragePath
	RunStoragePathSaved = RunStoragePath
	// allow the tests to run without affecting the host system.
	ConfigStoragePath = func() string { return filepath.Join(testDir, StoragePathSuffix, "config") }
	RunStoragePath = func() string { return filepath.Join(testDir, StoragePathSuffix, "run") }

	// set now that ConfigStoragePath has been overridden.
	sandboxDirConfig = filepath.Join(ConfigStoragePath(), testSandboxID)
	sandboxFileConfig = filepath.Join(ConfigStoragePath(), testSandboxID, ConfigurationFile)
	sandboxDirState = filepath.Join(RunStoragePath(), testSandboxID)
	sandboxDirLock = filepath.Join(RunStoragePath(), testSandboxID)
	sandboxFileState = filepath.Join(RunStoragePath(), testSandboxID, StateFile)
	sandboxFileLock = filepath.Join(RunStoragePath(), testSandboxID, LockFile)
}
