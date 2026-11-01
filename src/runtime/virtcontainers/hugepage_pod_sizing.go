// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"errors"
	"fmt"
	"math"

	ctrAnnotations "github.com/containerd/containerd/pkg/cri/annotations"
	resCtrl "github.com/kata-containers/kata-containers/src/runtime/pkg/resourcecontrol"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

// Whether the hypervisor takes guest RAM from the hugetlb pool when
// enable_hugepages is set. Only then is the pod's reservation the VM's size.
func hypervisorBacksGuestRAMWithHugePages(hypervisorType HypervisorType) bool {
	switch hypervisorType {
	case QemuHypervisor, ClhHypervisor:
		return true
	default:
		return false
	}
}

// Whether the pod's huge page reservation sizes this huge page backed guest
// and bounds its containers: enable_hugepages_pod_sizing.
func hugePagePodSized(hc *HypervisorConfig) bool {
	return hc.HugePages && hc.HugePagesPodSizing
}

// CheckHugePagesPodSizing checks the settings enable_hugepages_pod_sizing depends
// on. It runs when the configuration loads and again for each sandbox, since
// annotations can change them.
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

// The page size the VM's memory is on, spelled as a reservation spells it, or
// empty for a VM not sized from its pod.
func vmHugePageSize(hc *HypervisorConfig) string {
	name, err := resCtrl.HugetlbSizeName(hc.PodSizedHugePageSize)
	if err != nil {
		return ""
	}
	return name
}

// Adds up the pages of pageSize a container reserved, saturating at the
// largest value.
func hugePagesTotal(limits []specs.LinuxHugepageLimit, pageSize string) uint64 {
	var total uint64
	for _, l := range limits {
		if l.Pagesize != pageSize {
			continue
		}
		if l.Limit > math.MaxUint64-total {
			return math.MaxUint64
		}
		total += l.Limit
	}
	return total
}

// The first page size other than the VM's that a container reserves. Such a
// pool is outside the VM's size and has no host pages of its own size behind it.
func otherPageSizeReserved(limits []specs.LinuxHugepageLimit, pageSize string) string {
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

// The bytes a memory limit carries, or zero for none.
func memoryLimitBytes(limit int64) uint64 {
	if limit <= 0 || limit == math.MaxInt64 {
		return 0
	}
	return uint64(limit)
}

// capToInt64 is a byte count as a memory limit, saturating at math.MaxInt64.
func capToInt64(bytes uint64) int64 {
	if bytes > uint64(math.MaxInt64) {
		return math.MaxInt64
	}
	return int64(bytes)
}

// What one container carries into the guest: the pages of the VM's size it is
// down for, the memory limit its ceiling adds to them, and the ceiling it was
// given once it runs.
type podReservation struct {
	id        string
	hugePages uint64
	memory    uint64
	applied   *uint64
}

func reservationOf(cfg *ContainerConfig, pageSize string) podReservation {
	r := podReservation{
		id:        cfg.ID,
		hugePages: hugePagesTotal(cfg.Resources.HugepageLimits, pageSize),
		applied:   cfg.GuestMemoryCeiling,
	}
	if cfg.Resources.Memory != nil && cfg.Resources.Memory.Limit != nil {
		r.memory = memoryLimitBytes(*cfg.Resources.Memory.Limit)
	}
	return r
}

// What the sandbox's other containers carry. A stopped one is left out: it
// holds nothing now, and counting it would refuse the containers after it.
func podReservations(sandbox *Sandbox, pageSize, except string) []podReservation {
	var out []podReservation
	for _, cfg := range sandbox.containerConfigs() {
		if cfg.ID == except {
			continue
		}
		if c, ok := sandbox.containers[cfg.ID]; ok && c.state.State == types.StateStopped {
			continue
		}
		out = append(out, reservationOf(cfg, pageSize))
	}
	return out
}

// The name a container goes by in its pod, or its ID when none was passed.
func containerDisplayName(cfg *ContainerConfig) string {
	if name := cfg.Annotations[ctrAnnotations.ContainerName]; name != "" {
		return name
	}
	return cfg.ID
}

// The kubelet copies the pod's allowance into each container that declares none
// and nothing marks the copy. Declared values fit the VM, so values that add up
// past it carry copies: the largest one is the copy, and zero means none.
func copiedReservation(reservations []podReservation, vmMemoryBytes uint64) uint64 {
	if vmMemoryBytes == 0 {
		return 0
	}

	var largest, sum uint64
	for _, r := range reservations {
		if r.hugePages > largest {
			largest = r.hugePages
		}
		if r.hugePages > math.MaxUint64-sum {
			sum = math.MaxUint64
		} else {
			sum += r.hugePages
		}
	}
	below := false
	for _, r := range reservations {
		if r.hugePages != 0 && r.hugePages < largest {
			below = true
		}
	}
	if sum <= vmMemoryBytes || !below || largest > vmMemoryBytes {
		return 0
	}
	return largest
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

// What a container reserved for itself, and why it lost the reservation it
// carries when it did. Both the create and the update path hold to this.
func ownHugePages(hugePages, copied uint64) (uint64, string) {
	if copied != 0 && hugePages == copied {
		return 0, "the container states no huge pages of its own and was handed the pod's allowance: its memory limit alone bounds it inside the guest"
	}
	return hugePages, ""
}

// A container's memory ceiling inside the guest, or zero when it has none.
func guestCeilingBytes(r podReservation, copied uint64) uint64 {
	if own, _ := ownHugePages(r.hugePages, copied); own != 0 {
		return own
	}
	return r.memory
}

// What a guest holds: the VM's memory and its vCPUs. Its own needs grow with
// both, so both decide how much the pod's containers may use.
type guestCapacity struct {
	memoryMB uint32
	vCPUs    uint32
}

func (g guestCapacity) memoryBytes() uint64 {
	return uint64(g.memoryMB) << utils.MibToBytesShift
}

// Capacity of a sandbox that has all its memory at boot; empty for one that
// grows on demand, whose memory is hotplugged after a container asks.
func staticGuestCapacity(sandbox *Sandbox) guestCapacity {
	if !sandbox.config.StaticResourceMgmt {
		return guestCapacity{}
	}
	return guestCapacity{
		memoryMB: sandbox.config.HypervisorConfig.MemorySize,
		vCPUs:    sandbox.config.HypervisorConfig.NumVCPUs(),
	}
}

// Bounds the pod's containers together inside the guest, since their own
// ceilings can add up past it. Zero for any other guest.
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

// guestMemTotalMB returns the memory a guest of vmMemMB reports, or zero for a
// VM too small to boot one.
func guestMemTotalMB(vmMemMB uint32) uint32 {
	memmapMB := vmMemMB / guestMemmapDivisor
	if vmMemMB <= memmapMB+guestFixedOverheadMB {
		return 0
	}
	return vmMemMB - memmapMB - guestFixedOverheadMB
}

// guestReserveMB returns what a guest of memTotalMB on vCPUs keeps out of the
// memory it reports.
func guestReserveMB(memTotalMB, vCPUs uint32) uint32 {
	reserveMB := guestReserveBaseMB + memTotalMB/guestReserveMemDivisor + vCPUs*guestReservePerVCPUMB
	if capMB := memTotalMB / guestReserveMaxDivisor; reserveMB > capMB {
		return capMB
	}
	return reserveMB
}

// What the pod's containers may use between them; zero when the guest's size
// is not known upfront.
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

// Refuses a container when the pod's ceilings, as given, do not fit the guest.
// The others count with the ceiling they were given, this one with its own.
func refusePodCeilingsPastGuest(others []podReservation, this podReservation, copied uint64, guest guestCapacity, name string) error {
	holdable := holdableGuestMemoryBytes(guest)
	if holdable == 0 {
		return nil
	}
	ceilings := guestCeilingBytes(this, copied)
	for _, r := range others {
		c := guestCeilingBytes(r, copied)
		if r.applied != nil {
			c = *r.applied
		}
		if c > math.MaxUint64-ceilings {
			ceilings = math.MaxUint64
			break
		}
		ceilings += c
	}
	if ceilings <= holdable {
		return nil
	}
	short := (ceilings - holdable) >> utils.MibToBytesShift
	if short == 0 {
		short = 1
	}
	return fmt.Errorf("container %s brings the memory ceilings of the pod's containers to %d MiB, but the %d MiB guest holds %d MiB for them: the pod needs %d MiB more of the VM's huge pages, on the pod or as the runtime class's pod overhead",
		name,
		ceilings>>utils.MibToBytesShift,
		guest.memoryMB,
		holdable>>utils.MibToBytesShift,
		short)
}

// Refuses what a guest sized from the pod cannot give a container, before
// anything is changed for it, and returns the pod allowance copied into the
// containers that declared none.
func refusePodSizedContainer(sandbox *Sandbox, this podReservation, limits []specs.LinuxHugepageLimit, name string) ([]podReservation, uint64, error) {
	pageSize := vmHugePageSize(&sandbox.config.HypervisorConfig)
	if other := otherPageSizeReserved(limits, pageSize); other != "" {
		return nil, 0, fmt.Errorf("container %s reserves huge pages of %s, but the VM's memory is on %s pages: only %s huge pages can be used in this runtime class", name, other, pageSize, pageSize)
	}
	guest := staticGuestCapacity(sandbox)
	others := podReservations(sandbox, pageSize, this.id)
	copied := copiedReservation(append(others, this), guest.memoryBytes())
	if err := refusePodCeilingsPastGuest(others, this, copied, guest, name); err != nil {
		return nil, 0, err
	}
	return others, copied, nil
}

// Checks a new container and returns the huge pages it holds as its own.
func refusePodSizedCreate(sandbox *Sandbox, cfg *ContainerConfig, name string) (uint64, error) {
	this := reservationOf(cfg, vmHugePageSize(&sandbox.config.HypervisorConfig))
	others, copied, err := refusePodSizedContainer(sandbox, this, cfg.Resources.HugepageLimits, name)
	if err != nil {
		return 0, err
	}
	vmBytes := staticGuestCapacity(sandbox).memoryBytes()
	if value, ambiguous := ambiguousCopiedReservation(append(others, this), vmBytes); ambiguous {
		resource := resCtrl.HugepagesResourceName(sandbox.config.HypervisorConfig.PodSizedHugePageSize)
		return 0, fmt.Errorf("the pod level %s is copied into every container that does not declare it, and each container of this pod carries %d MiB of it, so a copy cannot be told from a declaration: declare %s on every container of the pod", resource, value>>utils.MibToBytesShift, resource)
	}
	own, why := ownHugePages(this.hugePages, copied)
	if why != "" {
		sandbox.Logger().WithFields(logrus.Fields{"container": name, "huge-pages": this.hugePages}).Info(why)
	}
	return own, nil
}

// Checks an update to a running container against the pod before it is recorded.
func refusePodSizedUpdate(c *Container, memory *specs.LinuxMemory, hugepageLimits []specs.LinuxHugepageLimit) error {
	cfg := *c.config
	cfg.Resources.HugepageLimits = hugepageLimits
	if memory != nil && memory.Limit != nil {
		cfg.Resources.Memory = memory
	}
	cfg.GuestMemoryCeiling = nil
	this := reservationOf(&cfg, vmHugePageSize(&c.sandbox.config.HypervisorConfig))
	_, _, err := refusePodSizedContainer(c.sandbox, this, hugepageLimits, containerDisplayName(c.config))
	return err
}

// The huge pages a running container holds as its own.
func ownHugePagesInPod(sandbox *Sandbox, cfg *ContainerConfig) uint64 {
	pageSize := vmHugePageSize(&sandbox.config.HypervisorConfig)
	this := reservationOf(cfg, pageSize)
	copied := copiedReservation(append(podReservations(sandbox, pageSize, cfg.ID), this), staticGuestCapacity(sandbox).memoryBytes())
	own, _ := ownHugePages(this.hugePages, copied)
	return own
}

// Sets what a container may use inside a guest sized from the pod: its own
// reservation, or its memory limit when it has none, and no huge pages. Both
// the create and the update path hold to this.
func (k *kataAgent) boundContainerInGuest(resources *grpc.LinuxResources, pageSize string, own uint64, name string) {
	if resources == nil {
		return
	}
	for _, l := range resources.HugepageLimits {
		if l.Pagesize == pageSize {
			l.Limit = 0
		}
	}
	if own == 0 {
		return
	}
	if resources.Memory == nil {
		resources.Memory = &grpc.LinuxMemory{}
	}
	memory := resources.Memory

	k.Logger().WithFields(logrus.Fields{
		"container":  name,
		"host-limit": memory.Limit,
		"huge-pages": own,
	}).Debug("applying the container's huge page reservation as the memory limit inside the guest")

	memory.Limit = capToInt64(own)
	if memory.Reservation > memory.Limit {
		memory.Reservation = memory.Limit
	}
	// Swap carries memory plus swap, so it cannot sit under the limit.
	if memory.Swap != 0 && memory.Swap != memory.Limit {
		memory.Swap = memory.Limit
	}
}

// Records the guest memory ceiling a container was given.
func recordGuestCeiling(cfg *ContainerConfig, resources *grpc.LinuxResources) {
	var ceiling uint64
	if cfg.GuestMemoryCeiling != nil {
		ceiling = *cfg.GuestMemoryCeiling
	}
	if resources != nil && resources.Memory != nil && resources.Memory.Limit != 0 {
		ceiling = memoryLimitBytes(resources.Memory.Limit)
	}
	cfg.GuestMemoryCeiling = &ceiling
}
