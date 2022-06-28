/*
 * Copyright (c) 2021, NVIDIA CORPORATION.  All rights reserved.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package nvpci

import (
	"fmt"
	"sort"

	"gitlab.com/nvidia/cloud-native/go-nvlib/pkg/nvpci/mmio"
)

const (
	pmcEndianRegister = 0x4
	pmcLittleEndian   = 0x0
	pmcBigEndian      = 0x01000001
)

// MemoryResource represents a mmio region
type MemoryResource struct {
	Start uintptr
	End   uintptr
	Flags uint64
	Path  string
}

// OpenRW read write mmio region
func (mr *MemoryResource) OpenRW() (mmio.Mmio, error) {
	rw, err := mmio.OpenRW(mr.Path, 0, int(mr.End-mr.Start+1))
	if err != nil {
		return nil, fmt.Errorf("failed to open file for mmio: %v", err)
	}
	switch rw.Read32(pmcEndianRegister) {
	case pmcBigEndian:
		return rw.BigEndian(), nil
	case pmcLittleEndian:
		return rw.LittleEndian(), nil
	}
	return nil, fmt.Errorf("unknown endianness for mmio: %v", err)
}

// OpenRO read only mmio region
func (mr *MemoryResource) OpenRO() (mmio.Mmio, error) {
	ro, err := mmio.OpenRO(mr.Path, 0, int(mr.End-mr.Start+1))
	if err != nil {
		return nil, fmt.Errorf("failed to open file for mmio: %v", err)
	}
	switch ro.Read32(pmcEndianRegister) {
	case pmcBigEndian:
		return ro.BigEndian(), nil
	case pmcLittleEndian:
		return ro.LittleEndian(), nil
	}
	return nil, fmt.Errorf("unknown endianness for mmio: %v", err)
}

// From Bit Twiddling Hacks, great resource for all low level bit manipulations
func calcNextPowerOf2(n uint64) uint64 {
	n--
	n |= n >> 1
	n |= n >> 2
	n |= n >> 4
	n |= n >> 8
	n |= n >> 16
	n |= n >> 32
	n++

	return n
}

// GetTotalAddressableMemory will accumulate the 32bit and 64bit memory windows
// of each BAR and round the value if needed to the next power of 2; first
// return value is the accumulated 32bit addresable memory size the second one
// is the accumulated 64bit addressable memory size in bytes. These values are
// needed to configure virtualized environments.
func (mrs MemoryResources) GetTotalAddressableMemory(roundUp bool) (uint64, uint64) {
	const pciIOVNumBAR = 6
	const pciBaseAddressMemTypeMask = 0x06
	const pciBaseAddressMemType32 = 0x00 /* 32 bit address */
	const pciBaseAddressMemType64 = 0x04 /* 64 bit address */

	// We need to sort the resources so the first 6 entries are the BARs
	// How a map is represented in memory is not guaranteed, it is not an
	// array. Keys do not have an order.
	keys := make([]int, 0, len(mrs))
	for k := range mrs {
		keys = append(keys, k)
	}
	sort.Ints(keys)

	numBAR := 0
	memSize32bit := uint64(0)
	memSize64bit := uint64(0)

	for _, key := range keys {
		// The PCIe spec only defines 5 BARs per device, we're
		// discarding everything after the 5th entry of the resources
		// file, see lspci.c
		if key >= pciIOVNumBAR || numBAR == pciIOVNumBAR {
			break
		}
		numBAR = numBAR + 1

		region := mrs[key]

		flags := region.Flags & pciBaseAddressMemTypeMask
		memType32bit := flags == pciBaseAddressMemType32
		memType64bit := flags == pciBaseAddressMemType64

		memSize := (region.End - region.Start) + 1

		if memType32bit {
			memSize32bit = memSize32bit + uint64(memSize)
		}
		if memType64bit {
			memSize64bit = memSize64bit + uint64(memSize)
		}

	}

	if roundUp {
		memSize32bit = calcNextPowerOf2(memSize32bit)
		memSize64bit = calcNextPowerOf2(memSize64bit)
	}

	return memSize32bit, memSize64bit
}
