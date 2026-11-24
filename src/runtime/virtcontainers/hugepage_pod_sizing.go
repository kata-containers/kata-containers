// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"errors"
	"fmt"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

func hypervisorBacksGuestRAMWithHugePages(hypervisorType HypervisorType) bool {
	switch hypervisorType {
	case QemuHypervisor, ClhHypervisor:
		return true
	default:
		return false
	}
}

func hugePagePodSized(hc *HypervisorConfig) bool {
	return hc.HugePages && hc.HugePagesPodSizing
}

// CheckHugePagesPodSizing runs when the configuration loads and again for each
// sandbox, because annotations can change the settings it depends on.
func CheckHugePagesPodSizing(hypervisorType HypervisorType, staticResourceMgmt, sandboxCgroupOnly, vmFactory bool) error {
	switch {
	case !hypervisorBacksGuestRAMWithHugePages(hypervisorType):
		return fmt.Errorf("enable_hugepages_pod_sizing needs a hypervisor that maps guest RAM from the huge page pool (qemu or cloud-hypervisor), and this sandbox uses %s", hypervisorType)
	case !staticResourceMgmt:
		return errors.New("enable_hugepages_pod_sizing needs static_sandbox_resource_mgmt")
	case !sandboxCgroupOnly:
		return errors.New("enable_hugepages_pod_sizing needs sandbox_cgroup_only: the hypervisor's huge pages are charged to the pod's cgroup")
	case vmFactory:
		return errors.New("enable_hugepages_pod_sizing sizes each VM from its pod, so it cannot use VM templates or the VM cache")
	}
	return nil
}

type guestCapacity struct {
	memoryMB uint32
	vCPUs    uint32
}

func (g guestCapacity) memoryBytes() uint64 {
	return uint64(g.memoryMB) << utils.MibToBytesShift
}

// Memory hotplug makes a fixed capacity check invalid.
func staticGuestCapacity(sandbox *Sandbox) guestCapacity {
	if !sandbox.config.StaticResourceMgmt {
		return guestCapacity{}
	}
	return guestCapacity{
		memoryMB: sandbox.config.HypervisorConfig.MemorySize,
		vCPUs:    sandbox.config.HypervisorConfig.NumVCPUs(),
	}
}

// A container with neither a reservation nor a memory limit has no ceiling of
// its own, so the agent also bounds the pod's containers together.
func sandboxMemoryMaxBytes(sandbox *Sandbox) uint64 {
	if !hugePagePodSized(&sandbox.config.HypervisorConfig) {
		return 0
	}
	return holdableGuestMemoryBytes(staticGuestCapacity(sandbox))
}

const (
	// A guest reports less than the VM was given: a 64 byte struct page per
	// 4 KiB frame, plus a fixed slice for firmware, kernel and initrd.
	guestMemmapDivisor   = 64
	guestFixedOverheadMB = 128

	// What the guest keeps for its kernel, drivers, agent and free pages. It does
	// not grow in proportion to the VM, so it has a base, a slice and a vCPU term.
	guestReserveBaseMB     = 768
	guestReserveMemDivisor = 512
	guestReservePerVCPUMB  = 24
	guestReserveMaxDivisor = 4
)

// Zero for a VM too small to hold the guest's fixed share.
func guestMemTotalMB(vmMemMB uint32) uint32 {
	memmapMB := vmMemMB / guestMemmapDivisor
	if vmMemMB <= memmapMB+guestFixedOverheadMB {
		return 0
	}
	return vmMemMB - memmapMB - guestFixedOverheadMB
}

func guestReserveMB(memTotalMB, vCPUs uint32) uint32 {
	reserveMB := guestReserveBaseMB + memTotalMB/guestReserveMemDivisor + vCPUs*guestReservePerVCPUMB
	// The base alone would take most of a small guest.
	if capMB := memTotalMB / guestReserveMaxDivisor; reserveMB > capMB {
		return capMB
	}
	return reserveMB
}

// Zero disables admission checks when the guest capacity cannot be computed.
func holdableGuestMemoryBytes(guest guestCapacity) uint64 {
	if guest.memoryMB == 0 {
		return 0
	}

	memTotalMB := guestMemTotalMB(guest.memoryMB)
	if memTotalMB == 0 {
		return 0
	}

	reserveMB := guestReserveMB(memTotalMB, guest.vCPUs)
	if reserveMB >= memTotalMB {
		return 0
	}

	return uint64(memTotalMB-reserveMB) << utils.MibToBytesShift
}
