// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"math"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// A refusal for the guest's capacity, which leaves the container as it was.
type guestCapacityRefusal struct{ error }

func (e guestCapacityRefusal) Unwrap() error { return e.error }

// Records the guest memory ceiling a container was given.
func (k *kataAgent) recordApplied(id string, resources *grpc.LinuxResources) {
	k.appliedLock.Lock()
	defer k.appliedLock.Unlock()
	if k.applied == nil {
		k.applied = make(map[string]uint64)
	}
	ceiling := k.applied[id]
	if resources != nil && resources.Memory != nil && resources.Memory.Limit != 0 {
		ceiling = uint64(memoryLimitBytes(resources.Memory))
	}
	k.applied[id] = ceiling
}

func (k *kataAgent) appliedCopy() map[string]uint64 {
	k.appliedLock.Lock()
	defer k.appliedLock.Unlock()
	out := make(map[string]uint64, len(k.applied))
	for id, c := range k.applied {
		out[id] = c
	}
	return out
}

// A container's memory ceiling inside the guest, or zero when it has none.
func guestCeilingBytes(r podReservation, copied uint64) uint64 {
	if own, _ := ownHugePages(r.hugePages, copied); own != 0 {
		return own
	}
	if r.memory > 0 && r.memory != math.MaxInt64 {
		return uint64(r.memory)
	}
	return 0
}

// Refuses a container when the pod's ceilings, as given, do not fit the guest.
func refusePodCeilingsPastGuest(reservations []podReservation, copied uint64, applied map[string]uint64, guest guestCapacity, self, name string) error {
	holdable := holdableGuestMemoryBytes(guest)
	if holdable <= 0 {
		return nil
	}
	var ceilings uint64
	for _, r := range reservations {
		c := guestCeilingBytes(r, copied)
		if a, ok := applied[r.id]; ok && r.id != self {
			c = a
		}
		if c > math.MaxUint64-ceilings {
			ceilings = math.MaxUint64
			break
		}
		ceilings += c
	}
	if ceilings <= uint64(holdable) {
		return nil
	}
	short := (ceilings - uint64(holdable)) >> utils.MibToBytesShift
	if short == 0 {
		short = 1
	}
	return fmt.Errorf("container %s brings the memory ceilings of the pod's containers to %d MiB, but the %d MiB guest holds %d MiB for them: the pod needs %d MiB more of the VM's huge pages, on the pod or as the runtime class's pod overhead",
		name,
		ceilings>>utils.MibToBytesShift,
		guest.memoryMB,
		uint64(holdable)>>utils.MibToBytesShift,
		short)
}

// Whether every container carries the same value past the VM, copy or not.
func ambiguousCopiedReservation(reservations []podReservation, vmMemoryBytes uint64) (uint64, bool) {
	if vmMemoryBytes == 0 {
		return 0, false
	}
	var value, sum uint64
	carriers := 0
	for _, r := range reservations {
		if r.hugePages == 0 {
			continue
		}
		if value != 0 && r.hugePages != value {
			return 0, false
		}
		value = r.hugePages
		if r.hugePages > math.MaxUint64-sum {
			sum = math.MaxUint64
		} else {
			sum += r.hugePages
		}
		carriers++
	}
	return value, carriers > 1 && sum > vmMemoryBytes && value <= vmMemoryBytes
}

// Zeroes a container's guest hugetlb limit for the VM's page size, since the
// guest keeps no huge pages for it.
func zeroHugePageLimits(limits []*grpc.LinuxHugepageLimit, pageSize string) {
	for _, l := range limits {
		if pageSize == "" || l.Pagesize == pageSize {
			l.Limit = 0
		}
	}
}

// Puts the reservation a container holds back into an update that names no
// huge pages of the VM's size, as one for the CPUs alone, so that the update
// still bounds the container by it.
func foldHeldHugePages(resources *grpc.LinuxResources, pageSize string, held uint64) {
	if held == 0 {
		return
	}
	for _, l := range resources.HugepageLimits {
		if l.Pagesize == pageSize {
			return
		}
	}
	resources.HugepageLimits = append(resources.HugepageLimits, &grpc.LinuxHugepageLimit{Pagesize: pageSize, Limit: held})
}

// The first page size other than the VM's that a container reserves. Such a
// pool is neither in the VM's size nor backed by host pages of its own size.
func otherPageSizeReserved(limits []*grpc.LinuxHugepageLimit, pageSize string) string {
	if pageSize == "" {
		return ""
	}
	for _, l := range limits {
		if l.Pagesize != pageSize && l.Limit != 0 {
			return l.Pagesize
		}
	}
	return ""
}

// Refuses a ceiling the guest cannot give, naming the shortfall, instead of
// holding the container below what it declared.
func refuseCeilingPastGuest(name string, ceiling int64, guest guestCapacity) error {
	holdable := holdableGuestMemoryBytes(guest)
	if holdable <= 0 || ceiling <= holdable {
		return nil
	}
	short := uint64(ceiling-holdable) >> utils.MibToBytesShift
	if short == 0 {
		short = 1
	}
	return fmt.Errorf("container %s declares a %d MiB memory ceiling, but the %d MiB guest holds %d MiB for it: the pod needs %d MiB more of the VM's huge pages, on the pod or as the runtime class's pod overhead",
		name,
		uint64(ceiling)>>utils.MibToBytesShift,
		guest.memoryMB,
		uint64(holdable)>>utils.MibToBytesShift,
		short)
}

// The bytes a memory limit carries, or zero for none.
func memoryLimitBytes(memory *grpc.LinuxMemory) int64 {
	if memory == nil || memory.Limit <= 0 || memory.Limit == math.MaxInt64 {
		return 0
	}
	return memory.Limit
}
