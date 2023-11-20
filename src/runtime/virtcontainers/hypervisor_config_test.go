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

func testHypervisorConfigValid(t *testing.T, hypervisorConfig *HypervisorConfig, success bool) {
	err := validateHypervisorConfig(hypervisorConfig)
	assert := assert.New(t)
	assert.False(success && err != nil)
	assert.False(!success && err == nil)
}

func TestHypervisorConfigNoKernelPath(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     "",
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	testHypervisorConfigValid(t, hypervisorConfig, false)
}

func TestRemoteHypervisorConfigNoKernelPath(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		RemoteHypervisorSocket: "dummy_socket",
		KernelPath:             "",
	}

	testHypervisorConfigValid(t, hypervisorConfig, true)
}
