// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"
)

func testIsSandbox(t *testing.T, cType ContainerType, expected bool) {
	if result := cType.IsSandbox(); result != expected {
		t.Fatalf("Got %t, Expecting %t", result, expected)
	}
}

func TestIsPodSandboxTrue(t *testing.T) {
	testIsSandbox(t, PodSandbox, true)
}

func TestIsPodContainerFalse(t *testing.T) {
	testIsSandbox(t, PodContainer, false)
}

func TestIsSandboxUnknownContainerTypeFalse(t *testing.T) {
	testIsSandbox(t, UnknownContainerType, false)
}
