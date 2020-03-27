// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

const (
	blockDeviceSupport = 1 << iota
	blockDeviceHotplugSupport
	multiQueueSupport
	fsSharingSupported
)

// Capabilities describe a virtcontainers hypervisor capabilities
// through a bit mask.
type Capabilities struct {
	flags uint
}

// IsBlockDeviceSupported tells if an hypervisor supports block devices.
func (caps *Capabilities) IsBlockDeviceSupported() bool {
	return caps.flags&blockDeviceSupport != 0
}

// SetBlockDeviceSupport sets the block device support capability to true.
func (caps *Capabilities) SetBlockDeviceSupport() {
	caps.flags = caps.flags | blockDeviceSupport
}

// IsBlockDeviceHotplugSupported tells if an hypervisor supports hotplugging block devices.
func (caps *Capabilities) IsBlockDeviceHotplugSupported() bool {
	return caps.flags&blockDeviceHotplugSupport != 0
}

// SetBlockDeviceHotplugSupport sets the block device hotplugging capability to true.
func (caps *Capabilities) SetBlockDeviceHotplugSupport() {
	caps.flags |= blockDeviceHotplugSupport
}

// IsMultiQueueSupported tells if an hypervisor supports device multi queue support.
func (caps *Capabilities) IsMultiQueueSupported() bool {
	return caps.flags&multiQueueSupport != 0
}

// SetMultiQueueSupport sets the device multi queue capability to true.
func (caps *Capabilities) SetMultiQueueSupport() {
	caps.flags |= multiQueueSupport
}

// IsFsSharingSupported tells if an hypervisor supports host filesystem sharing.
func (caps *Capabilities) IsFsSharingSupported() bool {
	return caps.flags&fsSharingSupported != 0
}

// SetFsSharingUnsupported sets the host filesystem sharing capability to true.
func (caps *Capabilities) SetFsSharingSupport() {
	caps.flags |= fsSharingSupported
}
