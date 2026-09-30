// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package main

func getExpectedHostDetails(tmpdir string) (HostInfo, error) {
	expectedVendor := ""
	expectedModel := "Loongson-3C5000"
	expectedVMContainerCapable := true
	return genericGetExpectedHostDetails(tmpdir, expectedVendor, expectedModel, expectedVMContainerCapable)
}
