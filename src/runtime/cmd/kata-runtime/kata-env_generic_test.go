// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//go:build arm64 || ppc64le

package main

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func testEnvGetEnvInfoSetsCPUTypeGeneric(t *testing.T) {
	assert := assert.New(t)

	tmpdir := t.TempDir()

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

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(err)

	expectedEnv, err := getExpectedSettings(config, tmpdir, configFile)
	assert.NoError(err)

	env, err := getEnvInfo(configFile, config)
	assert.NoError(err)

	// Free/Available are changing
	expectedEnv.Host.Memory = env.Host.Memory

	assert.Equal(expectedEnv, env)

	assert.Equal(archRequiredCPUFlags, savedArchRequiredCPUFlags)
	assert.Equal(archRequiredCPUAttribs, savedArchRequiredCPUAttribs)
	assert.Equal(archRequiredKernelModules, savedArchRequiredKernelModules)
}
