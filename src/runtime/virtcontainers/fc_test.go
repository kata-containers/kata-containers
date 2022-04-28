//go:build linux
// +build linux

// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"strings"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

func TestFCGenerateSocket(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	i, err := fc.GenerateSocket("a")
	assert.NoError(err)
	assert.NotNil(i)

	hvsock, ok := i.(types.HybridVSock)
	assert.True(ok)
	assert.NotEmpty(hvsock.UdsPath)

	// Path must be absolute
	assert.True(strings.HasPrefix(hvsock.UdsPath, "/"))

	assert.NotZero(hvsock.Port)
}

func TestFCTruncateID(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}

	testLongID := "3ef98eb7c6416be11e0accfed2f4e6560e07f8e33fa8d31922fd4d61747d7ead"
	expectedID := "3ef98eb7c6416be11e0accfed2f4e656"
	id := fc.truncateID(testLongID)
	assert.Equal(expectedID, id)

	testShortID := "3ef98eb7c6416be11"
	expectedID = "3ef98eb7c6416be11"
	id = fc.truncateID(testShortID)
	assert.Equal(expectedID, id)
}

func TestFCParseVersion(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}

	for rawVersion, v := range map[string]string{"Firecracker v0.23.1": "0.23.1", "Firecracker v0.25.0\nSupported snapshot data format versions: 0.23.0": "0.25.0"} {
		parsedVersion, err := fc.parseVersion(rawVersion)
		assert.NoError(err)
		assert.Equal(parsedVersion, v)
	}
}

func TestFcSetConfig(t *testing.T) {
	assert := assert.New(t)

	config := HypervisorConfig{
		HypervisorPath: "/some/where/firecracker",
		KernelPath:     "/some/where/kernel",
		ImagePath:      "/some/where/image",
		JailerPath:     "/some/where/jailer",
		Debug:          true,
	}

	fc := firecracker{}

	assert.Equal(fc.config, HypervisorConfig{})

	err := fc.setConfig(&config)
	assert.NoError(err)

	assert.Equal(fc.config, config)
}
