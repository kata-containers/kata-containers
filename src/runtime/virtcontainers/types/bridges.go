// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"context"
	"fmt"
)

// Type represents a type of bus and bridge.
type Type string

const PCIBridgeMaxCapacity = 30

const (
	// PCI represents a PCI bus and bridge
	PCI Type = "pci"

	// PCIE represents a PCIe bus and bridge
	PCIE Type = "pcie"
)

const CCWBridgeMaxCapacity = 0xffff

const (
	CCW Type = "ccw"
)

type Bridge struct {
	// Devices contains information about devices plugged and its address in the bridge
	Devices map[uint32]string

	// ID is used to identify the bridge in the hypervisor
	ID string

	// Type is the type of the bridge (pci, pcie, etc)
	Type Type

	// Addr is the slot of the bridge on its parent's bus. For a bridge plugged
	// directly on pcie.0/pci.0 this is the slot on the root bus. For a bridge
	// nested under another device (e.g. a pcie-pci-bridge sitting on top of a
	// pcie-root-port), this is the slot on that parent device's secondary bus
	// (typically 0, since pcie-root-ports expose a single slot).
	Addr int

	// MaxCapacity is the max capacity of the bridge
	MaxCapacity uint32

	// ParentID, when non-empty, is the QEMU ID of the device on whose bus this
	// bridge is plugged (e.g. a pcie-root-port). When set, the bridge is
	// emitted with "bus=<ParentID>" instead of pcie.0/pci.0. An empty
	// ParentID means the bridge sits directly on the root bus.
	ParentID string

	// ParentAddr is the slot of the parent device (referenced by ParentID) on
	// the root bus (pcie.0/pci.0). It is only meaningful when ParentID is set
	// and is used to construct multi-level guest PCI paths for devices that
	// end up nested under this bridge.
	ParentAddr int
}

func NewBridge(bt Type, id string, devices map[uint32]string, addr int) Bridge {
	var maxCapacity uint32
	switch bt {
	case PCI:
		fallthrough
	case PCIE:
		maxCapacity = PCIBridgeMaxCapacity
	case CCW:
		maxCapacity = CCWBridgeMaxCapacity
	default:
		maxCapacity = 0
	}
	return Bridge{
		Devices:     devices,
		ID:          id,
		Addr:        addr,
		Type:        bt,
		MaxCapacity: maxCapacity,
	}
}

// NewNestedBridge returns a Bridge that sits behind another device on the root
// bus (e.g. a pcie-pci-bridge cold-plugged onto a pcie-root-port). The bridge
// will be emitted on the QEMU command line with "bus=<parentID>", and the
// resulting guest PCI path for any device hot-plugged underneath will include
// the parent's slot on the root bus.
func NewNestedBridge(bt Type, id string, devices map[uint32]string, addr int, parentID string, parentAddr int) Bridge {
	b := NewBridge(bt, id, devices, addr)
	b.ParentID = parentID
	b.ParentAddr = parentAddr
	return b
}

// HasParent returns true when the bridge sits behind another bus device on the
// root complex (i.e. its bus= is not pcie.0/pci.0).
func (b *Bridge) HasParent() bool {
	return b.ParentID != ""
}

func (b *Bridge) AddDevice(ctx context.Context, ID string) (uint32, error) {
	var addr uint32

	// looking for the first available address
	for i := uint32(1); i <= b.MaxCapacity; i++ {
		if _, ok := b.Devices[i]; !ok {
			addr = i
			break
		}
	}

	if addr == 0 {
		return 0, fmt.Errorf("Unable to hot plug device on bridge: there are no empty slots")
	}

	// save address and device
	b.Devices[addr] = ID
	return addr, nil
}

func (b *Bridge) RemoveDevice(ID string) error {
	// check if the device was hot plugged in the bridge
	for addr, devID := range b.Devices {
		if devID == ID {
			// free address to re-use the same slot with other devices
			delete(b.Devices, addr)
			return nil
		}
	}

	return fmt.Errorf("Unable to hot unplug device %s: not present on bridge", ID)
}

// AddressFormatCCW returns the address format for the device number. The channel subsystem-ID 0xfe is reserved to the virtual channel and the address format is in the form fe.n.dddd, where n is subchannel set ID and ddd the device number. More details at https://www.ibm.com/support/knowledgecenter/en/linuxonibm/com.ibm.linux.z.ldva/ldva_t_configuringSCSIdevices.html
func (b *Bridge) AddressFormatCCW(addr string) (string, error) {
	if b.Type != CCW {
		return "", fmt.Errorf("Expected bridge type %T, got %T (%+v)", CCW, b.Type, b)
	}

	return fmt.Sprintf("fe.%x.%s", b.Addr, addr), nil
}

// AddressFormatCCWForVirtServer returns the address format for the virtual server. The address format is in the form of 0.n.dddd
func (b *Bridge) AddressFormatCCWForVirtServer(addr string) (string, error) {
	if b.Type != CCW {
		return "", fmt.Errorf("Wrong bridge type")
	}
	return fmt.Sprintf("0.%x.%s", b.Addr, addr), nil
}
