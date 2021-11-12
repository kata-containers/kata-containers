// Copyright Red Hat.
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestPciSlot(t *testing.T) {
	assert := assert.New(t)

	// Valid slots
	slot, err := PciSlotFromInt(0x00)
	assert.NoError(err)
	assert.Equal(slot, PciSlot{})
	assert.Equal(slot.String(), "00")

	slot, err = PciSlotFromString("00")
	assert.NoError(err)
	assert.Equal(slot, PciSlot{})

	slot, err = PciSlotFromInt(31)
	assert.NoError(err)
	slot2, err := PciSlotFromString("1f")
	assert.NoError(err)
	assert.Equal(slot, slot2)

	// Bad slots
	_, err = PciSlotFromInt(-1)
	assert.Error(err)

	_, err = PciSlotFromInt(32)
	assert.Error(err)

	_, err = PciSlotFromString("20")
	assert.Error(err)

	_, err = PciSlotFromString("xy")
	assert.Error(err)

	_, err = PciSlotFromString("00/")
	assert.Error(err)

	_, err = PciSlotFromString("")
	assert.Error(err)
}

func TestPciPath(t *testing.T) {
	assert := assert.New(t)

	slot3, err := PciSlotFromInt(0x03)
	assert.NoError(err)
	slot4, err := PciSlotFromInt(0x04)
	assert.NoError(err)
	slot5, err := PciSlotFromInt(0x05)
	assert.NoError(err)

	// Empty/nil paths
	pcipath := PciPath{}
	assert.True(pcipath.IsNil())

	pcipath, err = PciPathFromString("")
	assert.NoError(err)
	assert.True(pcipath.IsNil())
	assert.Equal(pcipath, PciPath{})

	// Valid paths
	pcipath, err = PciPathFromSlots(slot3)
	assert.NoError(err)
	assert.False(pcipath.IsNil())
	assert.Equal(pcipath.String(), "03")
	pcipath2, err := PciPathFromString("03")
	assert.NoError(err)
	assert.Equal(pcipath, pcipath2)

	pcipath, err = PciPathFromSlots(slot3, slot4)
	assert.NoError(err)
	assert.False(pcipath.IsNil())
	assert.Equal(pcipath.String(), "03/04")
	pcipath2, err = PciPathFromString("03/04")
	assert.NoError(err)
	assert.Equal(pcipath, pcipath2)

	pcipath, err = PciPathFromSlots(slot3, slot4, slot5)
	assert.NoError(err)
	assert.False(pcipath.IsNil())
	assert.Equal(pcipath.String(), "03/04/05")
	pcipath2, err = PciPathFromString("03/04/05")
	assert.NoError(err)
	assert.Equal(pcipath, pcipath2)

	// Bad paths
	_, err = PciPathFromSlots()
	assert.Error(err)

	_, err = PciPathFromString("20")
	assert.Error(err)

	_, err = PciPathFromString("//")
	assert.Error(err)

	_, err = PciPathFromString("xyz")
	assert.Error(err)

}
