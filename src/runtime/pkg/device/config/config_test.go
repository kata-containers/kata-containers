// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package config

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestGetBackingFile(t *testing.T) {
	assert := assert.New(t)

	dir := t.TempDir()

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

	err = os.WriteFile(filepath.Join(loopDir, "backing_file"), []byte(backingFile), os.FileMode(0755))
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

func TestIOMMUFDID(t *testing.T) {
	for _, tc := range []struct {
		devfsDev string
		expected string
	}{
		{"/dev/vfio/42", ""},
		{"/dev/vfio/devices/vfio99", "99"},
		{"/dev/vfio/invalid", ""},
		{"/dev/other/42", ""},
	} {
		t.Run(tc.devfsDev, func(t *testing.T) {
			assert := assert.New(t)

			info := VFIODev{
				DevfsDev: "/dev/vfio/devices/vfio5",
			}
			assert.Equal("5", info.IOMMUFDID())
		})
	}
}
