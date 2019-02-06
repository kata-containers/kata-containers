// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package store

import (
	"context"
	"io/ioutil"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

type TestNoopStructure struct {
	Field1 string
	Field2 string
}

var rootPath = "/tmp/root1/"
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
