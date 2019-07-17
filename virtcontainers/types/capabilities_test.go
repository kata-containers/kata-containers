// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestBlockDeviceCapability(t *testing.T) {
	var caps Capabilities

	assert.False(t, caps.IsBlockDeviceSupported())
	caps.SetBlockDeviceSupport()
	assert.True(t, caps.IsBlockDeviceSupported())
}

func TestBlockDeviceHotplugCapability(t *testing.T) {
	var caps Capabilities

	assert.False(t, caps.IsBlockDeviceHotplugSupported())
	caps.SetBlockDeviceHotplugSupport()
	assert.True(t, caps.IsBlockDeviceHotplugSupported())
}

func TestFsSharingCapability(t *testing.T) {
	var caps Capabilities

	assert.True(t, caps.IsFsSharingSupported())
	caps.SetFsSharingUnsupported()
	assert.False(t, caps.IsFsSharingSupported())
}
