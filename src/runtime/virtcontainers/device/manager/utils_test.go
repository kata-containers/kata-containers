// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package manager

import (
	"io/ioutil"
	"os"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/stretchr/testify/assert"
)

func TestIsVFIO(t *testing.T) {
	type testData struct {
		path     string
		expected bool
	}

	data := []testData{
		{"/dev/vfio/16", true},
		{"/dev/vfio/1", true},
		{"/dev/vfio/", false},
		{"/dev/vfio", false},
		{"/dev/vf", false},
		{"/dev", false},
		{"/dev/vfio/vfio", false},
		{"/dev/vfio/vfio/12", false},
	}

	for _, d := range data {
		isVFIO := isVFIO(d.path)
		assert.Equal(t, d.expected, isVFIO)
	}
}

func TestIsBlock(t *testing.T) {
	type testData struct {
		devType  string
		expected bool
	}

	data := []testData{
		{"b", true},
		{"c", false},
		{"u", false},
	}

	for _, d := range data {
		isBlock := isBlock(config.DeviceInfo{DevType: d.devType})
		assert.Equal(t, d.expected, isBlock)
	}
}

func TestIsVhostUserBlk(t *testing.T) {
	type testData struct {
		devType  string
		major    int64
		expected bool
	}

	data := []testData{
		{"b", config.VhostUserBlkMajor, true},
		{"c", config.VhostUserBlkMajor, false},
		{"b", config.VhostUserSCSIMajor, false},
		{"c", config.VhostUserSCSIMajor, false},
		{"b", 240, false},
	}

	for _, d := range data {
		isVhostUserBlk := isVhostUserBlk(
			config.DeviceInfo{
				DevType: d.devType,
				Major:   d.major,
			})
		assert.Equal(t, d.expected, isVhostUserBlk)
	}
}

func TestIsVhostUserSCSI(t *testing.T) {
	type testData struct {
		devType  string
		major    int64
		expected bool
	}

	data := []testData{
		{"b", config.VhostUserBlkMajor, false},
		{"c", config.VhostUserBlkMajor, false},
		{"b", config.VhostUserSCSIMajor, true},
		{"c", config.VhostUserSCSIMajor, false},
		{"b", 240, false},
	}

	for _, d := range data {
		isVhostUserSCSI := isVhostUserSCSI(
			config.DeviceInfo{
				DevType: d.devType,
				Major:   d.major,
			})
		assert.Equal(t, d.expected, isVhostUserSCSI)
	}
}

func TestIsLargeBarSpace(t *testing.T) {
	assert := assert.New(t)

	// File not exist
	bs, err := isLargeBarSpace("/abc/xyz/123/rgb")
	assert.Error(err)
	assert.False(bs)

	f, err := ioutil.TempFile("", "pci")
	assert.NoError(err)
	defer f.Close()
	defer os.RemoveAll(f.Name())

	type testData struct {
		resourceInfo string
		error        bool
		result       bool
	}

	for _, d := range []testData{
		{"", false, false},
		{"\t\n\t  ", false, false},
		{"abc zyx", false, false},
		{"abc zyx rgb", false, false},
		{"abc\t       zyx     \trgb", false, false},
		{"0x00015\n0x0013", false, false},
		{"0x00000000c6000000 0x00000000c6ffffff 0x0000000000040200", false, false},
		{"0x0000383bffffffff 0x0000383800000000", false, false}, // start greater than end
		{"0x0000383800000000 0x0000383bffffffff", false, true},
		{"0x0000383800000000 0x0000383bffffffff 0x000000000014220c", false, true},
	} {
		f.WriteAt([]byte(d.resourceInfo), 0)
		bs, err = isLargeBarSpace(f.Name())
		assert.NoError(f.Truncate(0))
		if d.error {
			assert.Error(err, d.resourceInfo)
		} else {
			assert.NoError(err, d.resourceInfo)
		}
		assert.Equal(d.result, bs, d.resourceInfo)
	}
}
