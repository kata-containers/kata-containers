// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"context"
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func createTempContainerIDMapping(containerID, sandboxID string) (string, error) {
	tmpDir, err := ioutil.TempDir("", "containers-mapping")
	if err != nil {
		return "", err
	}
	ctrsMapTreePath = tmpDir

	path := filepath.Join(ctrsMapTreePath, containerID, sandboxID)
	if err := os.MkdirAll(path, 0750); err != nil {
		return "", err
	}

	return tmpDir, nil
}

func TestFetchContainerIDMappingContainerIDEmptyFailure(t *testing.T) {
	assert := assert.New(t)

	sandboxID, err := FetchContainerIDMapping("")
	assert.Error(err)
	assert.Empty(sandboxID)
}

func TestFetchContainerIDMappingEmptyMappingSuccess(t *testing.T) {
	assert := assert.New(t)

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	sandboxID, err := FetchContainerIDMapping(testContainerID)
	assert.NoError(err)
	assert.Empty(sandboxID)
}

func TestFetchContainerIDMappingTooManyFilesFailure(t *testing.T) {
	assert := assert.New(t)

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)
	err = os.MkdirAll(filepath.Join(ctrsMapTreePath, testContainerID, testSandboxID+"2"), ctrsMappingDirMode)
	assert.NoError(err)

	sandboxID, err := FetchContainerIDMapping(testContainerID)
	assert.Error(err)
	assert.Empty(sandboxID)
}

func TestFetchContainerIDMappingSuccess(t *testing.T) {
	assert := assert.New(t)

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	sandboxID, err := FetchContainerIDMapping(testContainerID)
	assert.NoError(err)
	assert.Equal(sandboxID, testSandboxID)
}

func TestAddContainerIDMappingContainerIDEmptyFailure(t *testing.T) {
	assert := assert.New(t)

	err := AddContainerIDMapping(context.Background(), "", testSandboxID)
	assert.Error(err)
}

func TestAddContainerIDMappingSandboxIDEmptyFailure(t *testing.T) {
	assert := assert.New(t)

	err := AddContainerIDMapping(context.Background(), testContainerID, "")
	assert.Error(err)
}

func TestAddContainerIDMappingSuccess(t *testing.T) {
	assert := assert.New(t)

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	_, err = os.Stat(filepath.Join(ctrsMapTreePath, testContainerID, testSandboxID))
	assert.True(os.IsNotExist(err))

	err = AddContainerIDMapping(context.Background(), testContainerID, testSandboxID)
	assert.NoError(err)

	_, err = os.Stat(filepath.Join(ctrsMapTreePath, testContainerID, testSandboxID))
	assert.NoError(err)
}

func TestDelContainerIDMappingContainerIDEmptyFailure(t *testing.T) {
	assert := assert.New(t)

	err := DelContainerIDMapping(context.Background(), "")
	assert.Error(err)
}

func TestDelContainerIDMappingSuccess(t *testing.T) {
	assert := assert.New(t)

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	_, err = os.Stat(filepath.Join(ctrsMapTreePath, testContainerID, testSandboxID))
	assert.NoError(err)

	err = DelContainerIDMapping(context.Background(), testContainerID)
	assert.NoError(err)

	_, err = os.Stat(filepath.Join(ctrsMapTreePath, testContainerID, testSandboxID))
	assert.True(os.IsNotExist(err))
}
