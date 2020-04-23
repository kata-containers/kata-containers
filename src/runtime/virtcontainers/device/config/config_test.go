// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package config

import (
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestGetBackingFile(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "backing")
	assert.NoError(err)
	defer os.RemoveAll(dir)

	orgGetSysDevPath := getSysDevPath
	getSysDevPath = func(info DeviceInfo) string {
		return dir
	}
	defer func() { getSysDevPath = orgGetSysDevPath }()

	info := DeviceInfo{}
	path, err := getBackingFile(info)
	assert.Error(err)
	assert.Empty(path)

	loopDir := filepath.Join(dir, "loop")
	err = os.Mkdir(loopDir, os.FileMode(0755))
	assert.NoError(err)

	backingFile := "/fake-img"

	err = ioutil.WriteFile(filepath.Join(loopDir, "backing_file"), []byte(backingFile), os.FileMode(0755))
	assert.NoError(err)

	path, err = getBackingFile(info)
	assert.NoError(err)
	assert.Equal(backingFile, path)
}

func TestGetSysDevPathImpl(t *testing.T) {
	assert := assert.New(t)

	info := DeviceInfo{
		DevType: "",
		Major:   127,
		Minor:   0,
	}

	path := getSysDevPathImpl(info)
	assert.Empty(path)

	expectedFormat := fmt.Sprintf("%d:%d", info.Major, info.Minor)

	info.DevType = "c"
	path = getSysDevPathImpl(info)
	assert.Contains(path, expectedFormat)
	assert.Contains(path, "char")

	info.DevType = "b"
	path = getSysDevPathImpl(info)
	assert.Contains(path, expectedFormat)
	assert.Contains(path, "block")
}
