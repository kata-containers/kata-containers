// Copyright (c) 2023 Loongson Technology Corporation Limited
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"testing"
)

func getExpectedHostDetails(tmpdir string) (HostInfo, error) {
	expectedVendor := ""
	expectedModel := "Loongson-3C5000"
	expectedVMContainerCapable := true
	return genericGetExpectedHostDetails(tmpdir, expectedVendor, expectedModel, expectedVMContainerCapable)
}

func TestEnvGetEnvInfoSetsCPUType(t *testing.T) {
	testEnvGetEnvInfoSetsCPUTypeGeneric(t)
}
