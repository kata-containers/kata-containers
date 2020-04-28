// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func testIsSandbox(t *testing.T, cType ContainerType, expected bool) {
	assert.Equal(t, cType.IsSandbox(), expected)
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
