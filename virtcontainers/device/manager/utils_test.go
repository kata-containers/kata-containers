// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package manager

import (
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/device/config"
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
