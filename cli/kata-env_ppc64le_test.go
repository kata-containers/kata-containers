// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import "testing"

func getExpectedHostDetails(tmpdir string) (HostInfo, error) {
	return genericGetExpectedHostDetails(tmpdir)
}

func TestEnvGetEnvInfoSetsCPUType(t *testing.T) {
	testEnvGetEnvInfoSetsCPUTypeGeneric(t)
}
