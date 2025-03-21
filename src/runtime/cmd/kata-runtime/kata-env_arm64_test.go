// Copyright (c) 2018 ARM Limited
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"os"
	"testing"
)

func getExpectedHostDetails(tmpdir string) (HostInfo, error) {
	expectedVendor := "0x41"
	expectedModel := "8"
	expectedVMContainerCapable := true
	return genericGetExpectedHostDetails(tmpdir, expectedVendor, expectedModel, expectedVMContainerCapable)
}

func TestEnvGetEnvInfoSetsCPUType(t *testing.T) {
	if os.Getenv("GITHUB_RUNNER_CI_NON_VIRT") == "true" {
		t.Skip("Skipping the test as the GitHub self hosted runners for ARM64 do not support Virtualization")
	}
	testEnvGetEnvInfoSetsCPUTypeGeneric(t)
}
