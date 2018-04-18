// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import "testing"

func TestBlockDeviceCapability(t *testing.T) {
	var caps capabilities

	if caps.isBlockDeviceSupported() {
		t.Fatal()
	}

	caps.setBlockDeviceSupport()

	if !caps.isBlockDeviceSupported() {
		t.Fatal()
	}
}

func TestBlockDeviceHotplugCapability(t *testing.T) {
	var caps capabilities

	if caps.isBlockDeviceHotplugSupported() {
		t.Fatal()
	}

	caps.setBlockDeviceHotplugSupport()

	if !caps.isBlockDeviceHotplugSupported() {
		t.Fatal()
	}
}
