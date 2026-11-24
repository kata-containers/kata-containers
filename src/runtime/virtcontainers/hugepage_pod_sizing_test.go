// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestSandboxMemoryMaxBytes(t *testing.T) {
	assert := assert.New(t)

	for _, tt := range []struct {
		description string
		hugePages   bool
		notPodSized bool
		static      bool
		memMB       uint32
		vCPUs       float32
		expected    uint64
	}{
		{
			description: "huge page backed without enable_hugepages_pod_sizing",
			hugePages:   true,
			notPodSized: true,
			static:      true,
			memMB:       32 * 1024,
			vCPUs:       4,
		},
		{
			// A 32 GiB guest on 4 vCPUs reports 32128 MiB and keeps 926 of
			// them: 768 fixed, 62 for its size, 96 for its vCPUs.
			description: "huge page backed, static",
			hugePages:   true,
			static:      true,
			memMB:       32 * 1024,
			vCPUs:       4,
			expected:    uint64(31202) << 20,
		},
		{
			// A small guest keeps a quarter of what it reports, the cap
			// that keeps the fixed share from taking most of it.
			description: "small guest",
			hugePages:   true,
			static:      true,
			memMB:       2048,
			vCPUs:       1,
			expected:    uint64(1416) << 20,
		},
		{
			// Twice the vCPUs on the same size takes 96 MiB more.
			description: "the vCPU share grows with the vCPUs",
			hugePages:   true,
			static:      true,
			memMB:       32 * 1024,
			vCPUs:       8,
			expected:    uint64(31106) << 20,
		},
		{
			description: "not huge page backed",
			hugePages:   false,
			static:      true,
			memMB:       32 * 1024,
			vCPUs:       4,
		},
		{
			description: "grows on demand",
			hugePages:   true,
			static:      false,
			memMB:       32 * 1024,
			vCPUs:       4,
		},
		{
			description: "too small to hold anything",
			hugePages:   true,
			static:      true,
			memMB:       128,
			vCPUs:       1,
		},
	} {
		s := &Sandbox{config: &SandboxConfig{
			StaticResourceMgmt: tt.static,
			HypervisorConfig: HypervisorConfig{
				HugePages:          tt.hugePages,
				HugePagesPodSizing: !tt.notPodSized,
				MemorySize:         tt.memMB,
				NumVCPUsF:          tt.vCPUs,
			},
		}}
		assert.Equal(tt.expected, sandboxMemoryMaxBytes(s), tt.description)
	}
}

// Keeps the reserve above what a guest of that shape holds of its own, which a
// fraction of the VM cannot do at both ends of the size range.
func TestHoldableGuestMemoryReserve(t *testing.T) {
	assert := assert.New(t)

	for _, tt := range []struct {
		description string
		guest       guestCapacity
		// what such a guest holds of its own: its kernel, the drivers it
		// loaded for its devices and the free pages it keeps
		guestHoldsMB uint32
	}{
		{"a small guest", guestCapacity{memoryMB: 16 * 1024, vCPUs: 4}, 317},
		{"a mid sized guest", guestCapacity{memoryMB: 256 * 1024, vCPUs: 16}, 748},
		{"a large guest with devices", guestCapacity{memoryMB: 1024 * 1024, vCPUs: 64}, 2560},
	} {
		memTotalMB := guestMemTotalMB(tt.guest.memoryMB)
		reserveMB := guestReserveMB(memTotalMB, tt.guest.vCPUs)

		assert.Greater(reserveMB, tt.guestHoldsMB, tt.description)

		holdable := holdableGuestMemoryBytes(tt.guest)
		assert.Equal(uint64(memTotalMB-reserveMB)<<20, holdable, tt.description)
		assert.Less(holdable, uint64(memTotalMB)<<20, tt.description)
	}

	// The large guest keeps an eighth of what a thirty-second of the VM would
	// have taken from it.
	large := guestCapacity{memoryMB: 1024 * 1024, vCPUs: 64}
	assert.Less(guestReserveMB(guestMemTotalMB(large.memoryMB), large.vCPUs), large.memoryMB/32/4)

	// A guest too small to keep the whole fixed share keeps a quarter of what
	// it reports instead.
	small := guestCapacity{memoryMB: 2048, vCPUs: 1}
	memTotalMB := guestMemTotalMB(small.memoryMB)
	assert.Equal(memTotalMB/guestReserveMaxDivisor, guestReserveMB(memTotalMB, small.vCPUs))
}
