// Copyright (c) 2025 Institute of Software, CAS.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

func getExpectedHostDetails(tmpdir string) (HostInfo, error) {
	expectedVendor := "0x0"
	expectedModel := "0x0"
	expectedVMContainerCapable := true
	return genericGetExpectedHostDetails(tmpdir, expectedVendor, expectedModel, expectedVMContainerCapable)
}
