// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestHypervisorConfigNoImagePath(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      "",
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	testHypervisorConfigValid(t, hypervisorConfig, false)
}

func TestHypervisorConfigNoHypervisorPath(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: "",
	}

	testHypervisorConfigValid(t, hypervisorConfig, true)
}

func TestHypervisorConfigIsValid(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	testHypervisorConfigValid(t, hypervisorConfig, true)
}

func TestHypervisorConfigBothInitrdAndImage(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		InitrdPath:     fmt.Sprintf("%s/%s", testDir, testInitrd),
		HypervisorPath: "",
	}

	testHypervisorConfigValid(t, hypervisorConfig, false)
}

func TestHypervisorConfigSecureExecution(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:            fmt.Sprintf("%s/%s", testDir, testKernel),
		InitrdPath:            fmt.Sprintf("%s/%s", testDir, testInitrd),
		ConfidentialGuest:     true,
		HypervisorMachineType: QemuCCWVirtio,
	}

	// Secure Execution should only specify a kernel (encrypted image contains all components)
	testHypervisorConfigValid(t, hypervisorConfig, false)
}

func TestHypervisorConfigValidTemplateConfig(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:       fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:        fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath:   fmt.Sprintf("%s/%s", testDir, testHypervisor),
		BootToBeTemplate: true,
		BootFromTemplate: true,
	}
	testHypervisorConfigValid(t, hypervisorConfig, false)

	hypervisorConfig.BootToBeTemplate = false
	testHypervisorConfigValid(t, hypervisorConfig, false)
	hypervisorConfig.MemoryPath = "foobar"
	testHypervisorConfigValid(t, hypervisorConfig, false)
	hypervisorConfig.DevicesStatePath = "foobar"
	testHypervisorConfigValid(t, hypervisorConfig, true)

	hypervisorConfig.BootFromTemplate = false
	hypervisorConfig.BootToBeTemplate = true
	testHypervisorConfigValid(t, hypervisorConfig, true)
	hypervisorConfig.MemoryPath = ""
	testHypervisorConfigValid(t, hypervisorConfig, false)
}

func TestHypervisorConfigDefaults(t *testing.T) {
	assert := assert.New(t)
	hypervisorConfig := &HypervisorConfig{
		KernelPath:          fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:           fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath:      "",
		DisableGuestSeLinux: defaultDisableGuestSeLinux,
	}
	testHypervisorConfigValid(t, hypervisorConfig, true)

	hypervisorConfigDefaultsExpected := &HypervisorConfig{
		KernelPath:          fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:           fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath:      "",
		NumVCPUsF:           defaultVCPUs,
		MemorySize:          defaultMemSzMiB,
		DefaultBridges:      defaultBridges,
		BlockDeviceDriver:   defaultBlockDriver,
		DefaultMaxVCPUs:     defaultMaxVCPUs,
		Msize9p:             defaultMsize9p,
		DisableGuestSeLinux: defaultDisableGuestSeLinux,
	}

	assert.Exactly(hypervisorConfig, hypervisorConfigDefaultsExpected)
}
