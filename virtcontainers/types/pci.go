// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import "fmt"

// PCIType represents a type of PCI bus and bridge.
type PCIType string

const (
	// PCI represents a PCI bus and bridge
	PCI PCIType = "pci"

	// PCIE represents a PCIe bus and bridge
	PCIE PCIType = "pcie"
)

const pciBridgeMaxCapacity = 30

// PCIBridge is a PCI or PCIe bridge where devices can be hot plugged
type PCIBridge struct {
	// Address contains information about devices plugged and its address in the bridge
	Address map[uint32]string

	// Type is the PCI type of the bridge (pci, pcie, etc)
	Type PCIType

	// ID is used to identify the bridge in the hypervisor
	ID string

	// Addr is the PCI/e slot of the bridge
	Addr int
}

// AddDevice on success adds the device ID to the PCI bridge and returns
// the address where the device was added.
func (b *PCIBridge) AddDevice(ID string) (uint32, error) {
	var addr uint32

	// looking for the first available address
	for i := uint32(1); i <= pciBridgeMaxCapacity; i++ {
		if _, ok := b.Address[i]; !ok {
			addr = i
			break
		}
	}

	if addr == 0 {
		return 0, fmt.Errorf("Unable to hot plug device on bridge: there are not empty slots")
	}

	// save address and device
	b.Address[addr] = ID
	return addr, nil
}

// RemoveDevice removes the device ID from the PCI bridge.
func (b *PCIBridge) RemoveDevice(ID string) error {
	// check if the device was hot plugged in the bridge
	for addr, devID := range b.Address {
		if devID == ID {
			// free address to re-use the same slot with other devices
			delete(b.Address, addr)
			return nil
		}
	}

	return fmt.Errorf("Unable to hot unplug device %s: not present on bridge", ID)
}
