// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import "testing"

func getExpectedHostDetails(tmpdir string) (HostInfo, error) {
	expectedVendor := ""
	expectedModel := "POWER9"
	expectedVMContainerCapable := true
	return genericGetExpectedHostDetails(tmpdir, expectedVendor, expectedModel, expectedVMContainerCapable)
}

func TestEnvGetEnvInfoSetsCPUType(t *testing.T) {
	testEnvGetEnvInfoSetsCPUTypeGeneric(t)
}
