// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"context"
	"fmt"
	"testing"

	"github.com/stretchr/testify/assert"
)

func testAddRemoveDevice(t *testing.T, b *Bridge) {
	assert := assert.New(t)

	// add device
	devID := "abc123"

	addr, err := b.AddDevice(context.Background(), devID)
	assert.NoError(err)
	if addr < 1 {
		assert.Fail("address cannot be less than 1")
	}

	// remove device
	err = b.RemoveDevice("")
	assert.Error(err)

	err = b.RemoveDevice(devID)
	assert.NoError(err)

	// add device when the bridge is full
	b.Devices = make(map[uint32]string)
	for i := uint32(1); i <= b.MaxCapacity; i++ {
		b.Devices[i] = fmt.Sprintf("%d", i)
	}
	addr, err = b.AddDevice(context.Background(), devID)
	assert.Error(err)
	if addr != 0 {
		assert.Fail("address should be 0")
	}
}

func TestAddressFormat(t *testing.T) {
	assert := assert.New(t)

	// successful cases for AddressFormat functions
	var ccwbridge = NewBridge(CCW, "", make(map[uint32]string), 0)
	format, err := ccwbridge.AddressFormatCCW("0")
	assert.NoError(err)
	assert.Equal(format, "fe.0.0", "Format string should be fe.0.0")
	format, err = ccwbridge.AddressFormatCCWForVirtServer("0")
	assert.NoError(err)
	assert.Equal(format, "0.0.0", "Format string should be 0.0.0")

	// failure cases for AddressFormat functions
	var pcibridge = NewBridge(PCI, "", make(map[uint32]string), 0)
	_, err = pcibridge.AddressFormatCCW("0")
	assert.Error(err)
	_, err = pcibridge.AddressFormatCCWForVirtServer("0")
	assert.Error(err)

}

func TestNewBridge(t *testing.T) {
	assert := assert.New(t)

	var pci Type = "pci"
	var pcie Type = "pcie"
	var ccw Type = "ccw"
	var maxDefaultCapacity uint32

	var pcibridge = NewBridge(PCI, "", make(map[uint32]string), 0)
	assert.Equal(pcibridge.Type, pci, "Type should be PCI")
	assert.Equal(pcibridge.Devices, make(map[uint32]string), "Devices should be equal to make(map[uint32]string)")
	assert.Equal(pcibridge.Addr, 0, "Address should be 0")

	var pciebridge = NewBridge(PCIE, "", make(map[uint32]string), 0)
	assert.Equal(pciebridge.Type, pcie, "Type should be PCIE")
	assert.Equal(pciebridge.Devices, make(map[uint32]string), "Devices should be equal to make(map[uint32]string)")
	assert.Equal(pciebridge.Addr, 0, "Address should be 0")

	var ccwbridge = NewBridge(CCW, "", make(map[uint32]string), 0)
	assert.Equal(ccwbridge.Type, ccw, "Type should be CCW")
	assert.Equal(ccwbridge.Devices, make(map[uint32]string), "Devices should be equal to make(map[uint32]string)")
	assert.Equal(ccwbridge.Addr, 0, "Address should be 0")

	var defaultbridge = NewBridge("", "", make(map[uint32]string), 0)
	assert.Empty(defaultbridge.Type)
	assert.Equal(defaultbridge.MaxCapacity, maxDefaultCapacity, "MaxCapacity should be 0")
}

func TestAddRemoveDevicePCI(t *testing.T) {

	// create a pci bridge
	bridges := []*Bridge{{make(map[uint32]string), "rgb123", 5, PCI, PCIBridgeMaxCapacity}}

	testAddRemoveDevice(t, bridges[0])
}

func TestAddRemoveDeviceCCW(t *testing.T) {

	// create a CCW bridge
	bridges := []*Bridge{{make(map[uint32]string), "rgb123", 5, CCW, CCWBridgeMaxCapacity}}

	testAddRemoveDevice(t, bridges[0])
}
