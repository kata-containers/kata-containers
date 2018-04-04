// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import "fmt"

type bridgeType string

const (
	pciBridge  bridgeType = "pci"
	pcieBridge bridgeType = "pcie"
)

const pciBridgeMaxCapacity = 30

// Bridge is a bridge where devices can be hot plugged
type Bridge struct {
	// Address contains information about devices plugged and its address in the bridge
	Address map[uint32]string

	// Type is the type of the bridge (pci, pcie, etc)
	Type bridgeType

	//ID is used to identify the bridge in the hypervisor
	ID string

	// Addr is the PCI/e slot of the bridge
	Addr int
}

// addDevice on success adds the device ID to the bridge and return the address
// where the device was added, otherwise an error is returned
func (b *Bridge) addDevice(ID string) (uint32, error) {
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

// removeDevice on success removes the device ID from the bridge and return nil,
// otherwise an error is returned
func (b *Bridge) removeDevice(ID string) error {
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
