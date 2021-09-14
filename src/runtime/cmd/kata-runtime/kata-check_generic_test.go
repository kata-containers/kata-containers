// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// +build arm64 ppc64le

package main

import (
	"io/ioutil"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
)

func testSetCPUTypeGeneric(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	savedArchRequiredCPUFlags := archRequiredCPUFlags
	savedArchRequiredCPUAttribs := archRequiredCPUAttribs
	savedArchRequiredKernelModules := archRequiredKernelModules

	defer func() {
		archRequiredCPUFlags = savedArchRequiredCPUFlags
		archRequiredCPUAttribs = savedArchRequiredCPUAttribs
		archRequiredKernelModules = savedArchRequiredKernelModules
	}()

	assert.Empty(archRequiredCPUFlags)
	assert.Empty(archRequiredCPUAttribs)
	assert.NotEmpty(archRequiredKernelModules)

	_, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(err)

	setCPUtype(config.HypervisorType)

	assert.Equal(archRequiredCPUFlags, savedArchRequiredCPUFlags)
	assert.Equal(archRequiredCPUAttribs, savedArchRequiredCPUAttribs)
	assert.Equal(archRequiredKernelModules, savedArchRequiredKernelModules)
}
