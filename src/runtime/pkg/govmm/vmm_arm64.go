//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package govmm

// In qemu, maximum number of vCPUs depends on the GIC version, or on how
// many redistributors we can fit into the memory map.
// related codes are under github.com/qemu/qemu/hw/arm/virt.c(Line 135 and 1306 in stable-2.11)
// for now, qemu only supports v2 and v3, we treat v4 as v3 based on
// backward compatibility.
var gicList = map[uint32]uint32{
	uint32(2): uint32(8),
	uint32(3): uint32(123),
	uint32(4): uint32(123),
}

var defaultGICVersion = uint32(3)

// MaxVCPUs returns the maximum number of vCPUs supported
func MaxVCPUs() uint32 {
	return gicList[defaultGICVersion]
}
