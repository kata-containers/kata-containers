// Copyright (c) 2018 ARM Limited
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"testing"
)

func getExpectedHostDetails(tmpdir string) (HostInfo, error) {
	expectedVendor := "0x41"
	expectedModel := "8"
	expectedVMContainerCapable := true
	return genericGetExpectedHostDetails(tmpdir, expectedVendor, expectedModel, expectedVMContainerCapable)
}

func TestEnvGetEnvInfoSetsCPUType(t *testing.T) {
	testEnvGetEnvInfoSetsCPUTypeGeneric(t)
}
