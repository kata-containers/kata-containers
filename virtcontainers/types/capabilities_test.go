// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import "testing"

func TestBlockDeviceCapability(t *testing.T) {
	var caps Capabilities

	if caps.IsBlockDeviceSupported() {
		t.Fatal()
	}

	caps.SetBlockDeviceSupport()

	if !caps.IsBlockDeviceSupported() {
		t.Fatal()
	}
}

func TestBlockDeviceHotplugCapability(t *testing.T) {
	var caps Capabilities

	if caps.IsBlockDeviceHotplugSupported() {
		t.Fatal()
	}

	caps.SetBlockDeviceHotplugSupport()

	if !caps.IsBlockDeviceHotplugSupported() {
		t.Fatal()
	}
}

func TestFsSharingCapability(t *testing.T) {
	var caps Capabilities

	if !caps.IsFsSharingSupported() {
		t.Fatal()
	}

	caps.SetFsSharingUnsupported()

	if caps.IsFsSharingSupported() {
		t.Fatal()
	}
}
