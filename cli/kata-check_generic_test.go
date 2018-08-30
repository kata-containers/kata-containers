// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// +build arm64 ppc64le

package main

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func testSetCPUTypeGeneric(t *testing.T) {
	assert := assert.New(t)

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

	setCPUtype()

	assert.Equal(archRequiredCPUFlags, savedArchRequiredCPUFlags)
	assert.Equal(archRequiredCPUAttribs, savedArchRequiredCPUAttribs)
	assert.Equal(archRequiredKernelModules, savedArchRequiredKernelModules)
}
