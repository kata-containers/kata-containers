// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package manager

import (
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
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
		isVFIO := IsVFIODevice(d.path)
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
		isVhostUserBlk := IsVhostUserBlk(
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
