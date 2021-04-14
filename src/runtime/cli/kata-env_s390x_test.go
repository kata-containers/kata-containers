// Copyright (c) 2021 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package main

func getExpectedHostDetails(tmpdir string) (HostInfo, error) {
	expectedVendor := "moi"
	expectedModel := "awesome XI"
	expectedVMContainerCapable := true
	return genericGetExpectedHostDetails(tmpdir, expectedVendor, expectedModel, expectedVMContainerCapable)
}
