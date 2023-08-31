// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func getExpectedHostDetails(tmpdir string) (HostInfo, error) {
	expectedVendor := "moi"
	expectedModel := "awesome XI"
	expectedVMContainerCapable := false
	return genericGetExpectedHostDetails(tmpdir, expectedVendor, expectedModel, expectedVMContainerCapable)
}

func TestEnvGetEnvInfoSetsCPUType(t *testing.T) {
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

	archRequiredCPUFlags = map[string]string{}
	archRequiredCPUAttribs = map[string]string{}
	archRequiredKernelModules = map[string]kernelModule{}

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(err)

	expectedEnv, err := getExpectedSettings(config, tmpdir, configFile)
	assert.NoError(err)

	env, err := getEnvInfo(configFile, config)
	assert.NoError(err)

	// Free/Available are changing
	expectedEnv.Host.Memory = env.Host.Memory

	assert.Equal(expectedEnv, env)

	assert.NotEmpty(archRequiredCPUFlags)
	assert.NotEmpty(archRequiredCPUAttribs)
	assert.NotEmpty(archRequiredKernelModules)

	cpuType = getCPUtype()
	if cpuType == cpuTypeIntel {
		assert.Equal(archRequiredCPUFlags["vmx"], "Virtualization support")
	} else if cpuType == cpuTypeAMD {
		assert.Equal(archRequiredCPUFlags["svm"], "Virtualization support")
	}

	_, ok := archRequiredKernelModules["kvm"]
	assert.True(ok)
}
