// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"os"
	"testing"
)

func getExpectedHostDetails(tmpdir string) (HostInfo, error) {
	expectedVendor := ""
	expectedModel := "POWER9"
	expectedVMContainerCapable := true
	return genericGetExpectedHostDetails(tmpdir, expectedVendor, expectedModel, expectedVMContainerCapable)
}

func TestEnvGetEnvInfoSetsCPUType(t *testing.T) {
	if os.Getenv("GITHUB_RUNNER_CI_NON_VIRT") == "true" {
		t.Skip("Skipping the test as the GitHub self hosted runners for ppc64le do not support Virtualization")
	}
	testEnvGetEnvInfoSetsCPUTypeGeneric(t)
}
