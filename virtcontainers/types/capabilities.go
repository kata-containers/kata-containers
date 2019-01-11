// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

const (
	blockDeviceSupport = 1 << iota
	blockDeviceHotplugSupport
	multiQueueSupport
	fsSharingUnsupported
)

// Capabilities describe a virtcontainers hypervisor capabilities
// through a bit mask.
type Capabilities struct {
	flags uint
}

// IsBlockDeviceSupported tells if an hypervisor supports block devices.
func (caps *Capabilities) IsBlockDeviceSupported() bool {
	if caps.flags&blockDeviceSupport != 0 {
		return true
	}
	return false
}

// SetBlockDeviceSupport sets the block device support capability to true.
func (caps *Capabilities) SetBlockDeviceSupport() {
	caps.flags = caps.flags | blockDeviceSupport
}

// IsBlockDeviceHotplugSupported tells if an hypervisor supports hotplugging block devices.
func (caps *Capabilities) IsBlockDeviceHotplugSupported() bool {
	if caps.flags&blockDeviceHotplugSupport != 0 {
		return true
	}
	return false
}

// SetBlockDeviceHotplugSupport sets the block device hotplugging capability to true.
func (caps *Capabilities) SetBlockDeviceHotplugSupport() {
	caps.flags |= blockDeviceHotplugSupport
}

// IsMultiQueueSupported tells if an hypervisor supports device multi queue support.
func (caps *Capabilities) IsMultiQueueSupported() bool {
	if caps.flags&multiQueueSupport != 0 {
		return true
	}
	return false
}

// SetMultiQueueSupport sets the device multi queue capability to true.
func (caps *Capabilities) SetMultiQueueSupport() {
	caps.flags |= multiQueueSupport
}

// IsFsSharingSupported tells if an hypervisor supports host filesystem sharing.
func (caps *Capabilities) IsFsSharingSupported() bool {
	return caps.flags&fsSharingUnsupported == 0
}

// SetFsSharingUnsupported sets the host filesystem sharing capability to true.
func (caps *Capabilities) SetFsSharingUnsupported() {
	caps.flags |= fsSharingUnsupported
}
