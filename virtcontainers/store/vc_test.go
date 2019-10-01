// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package store

import (
	"context"
	"fmt"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestStoreVCRoots(t *testing.T) {
	rootURL := filesystemScheme + "://" + ConfigStoragePath()
	sandboxID := "sandbox"
	containerID := "container"
	sConfigRoot := rootURL + "/" + sandboxID
	cConfigRoot := rootURL + "/" + sandboxID + "/" + containerID

	assert.Equal(t, SandboxConfigurationRoot(sandboxID), sConfigRoot)
	assert.Equal(t, ContainerConfigurationRoot(sandboxID, containerID), cConfigRoot)
}

func testStoreVCSandboxDir(t *testing.T, item Item, expected string) error {
	var dir string
	if item == Configuration {
		dir = SandboxConfigurationRootPath(testSandboxID)
	} else {
		dir = SandboxRuntimeRootPath(testSandboxID)
	}

	if dir != expected {
		return fmt.Errorf("Unexpected sandbox directory %s vs %s", dir, expected)
	}

	return nil
}

func testStoreVCSandboxFile(t *testing.T, item Item, expected string) error {
	var file string
	var err error

	if item == Configuration {
		file, err = SandboxConfigurationItemPath(testSandboxID, item)
	} else {
		file, err = SandboxRuntimeItemPath(testSandboxID, item)
	}

	if err != nil {
		return err
	}

	if file != expected {
		return fmt.Errorf("Unexpected sandbox file %s vs %s", file, expected)
	}

	return nil
}

func TestStoreVCSandboxDirConfig(t *testing.T) {
	err := testStoreVCSandboxDir(t, Configuration, sandboxDirConfig)
	assert.Nil(t, err)
}

func TestStoreVCSandboxDirState(t *testing.T) {
	err := testStoreVCSandboxDir(t, State, sandboxDirState)
	assert.Nil(t, err)
}

func TestStoreVCSandboxDirLock(t *testing.T) {
	err := testStoreVCSandboxDir(t, Lock, sandboxDirLock)
	assert.Nil(t, err)
}

func TestStoreVCSandboxFileConfig(t *testing.T) {
	err := testStoreVCSandboxFile(t, Configuration, sandboxFileConfig)
	assert.Nil(t, err)
}

func TestStoreVCSandboxFileState(t *testing.T) {
	err := testStoreVCSandboxFile(t, State, sandboxFileState)
	assert.Nil(t, err)
}

func TestStoreVCSandboxFileLock(t *testing.T) {
	err := testStoreVCSandboxFile(t, Lock, sandboxFileLock)
	assert.Nil(t, err)
}

func TestStoreVCSandboxFileNegative(t *testing.T) {
	_, err := SandboxConfigurationItemPath("", State)
	assert.NotNil(t, err)

	_, err = SandboxRuntimeItemPath("", State)
	assert.NotNil(t, err)
}

func TestStoreVCNewVCSandboxStore(t *testing.T) {
	_, err := NewVCSandboxStore(context.Background(), testSandboxID)
	assert.Nil(t, err)

	_, err = NewVCSandboxStore(context.Background(), "")
	assert.NotNil(t, err)
}

func TestStoreVCNewVCContainerStore(t *testing.T) {
	_, err := NewVCContainerStore(context.Background(), testSandboxID, "foobar")
	assert.Nil(t, err)

	_, err = NewVCContainerStore(context.Background(), "", "foobar")
	assert.NotNil(t, err)

	_, err = NewVCContainerStore(context.Background(), "", "foobar")
	assert.NotNil(t, err)
}
