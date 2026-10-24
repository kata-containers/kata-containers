// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/stretchr/testify/assert"

	pb "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
)

func TestZeroHugePageLimitsAndOtherPageSize(t *testing.T) {
	assert := assert.New(t)

	limits := []*pb.LinuxHugepageLimit{
		{Pagesize: "2MB", Limit: 0},
		{Pagesize: "1GB", Limit: 8 << 30},
	}
	assert.Equal("", otherPageSizeReserved(limits, "1GB"))
	zeroHugePageLimits(limits, "1GB")
	assert.Equal(uint64(0), limits[1].Limit)

	limits = append(limits, &pb.LinuxHugepageLimit{Pagesize: "2MB", Limit: 2 << 30})
	assert.Equal("2MB", otherPageSizeReserved(limits, "1GB"))
}

func TestBoundContainerInGuest(t *testing.T) {
	const gib = int64(1) << 30
	saved := readVMHugepageSize
	t.Cleanup(func() { readVMHugepageSize = saved })
	readVMHugepageSize = func() (uint64, error) { return 1 << 30, nil }

	guest := guestCapacity{memoryMB: 16 * 1024, vCPUs: 4}
	k := &kataAgent{}

	for _, tt := range []struct {
		description   string
		copied        uint64
		guest         guestCapacity
		resources     *pb.LinuxResources
		refused       string
		expectedLimit int64
		expectedHuge  uint64
	}{
		{
			description: "a container is bounded by its reservation and takes no huge pages",
			guest:       guest,
			resources: &pb.LinuxResources{
				Memory:         &pb.LinuxMemory{Limit: 1 << 20},
				HugepageLimits: []*pb.LinuxHugepageLimit{{Pagesize: "1GB", Limit: 8 << 30}},
			},
			expectedLimit: 8 * gib,
			expectedHuge:  0,
		},
		{
			description: "a copied reservation leaves the memory limit as the ceiling",
			copied:      16 << 30,
			guest:       guest,
			resources: &pb.LinuxResources{
				Memory:         &pb.LinuxMemory{Limit: 512 << 20},
				HugepageLimits: []*pb.LinuxHugepageLimit{{Pagesize: "1GB", Limit: 16 << 30}},
			},
			expectedLimit: 512 << 20,
			expectedHuge:  0,
		},
		{
			description: "a reservation of another page size is refused",
			guest:       guest,
			resources: &pb.LinuxResources{
				Memory:         &pb.LinuxMemory{Limit: gib},
				HugepageLimits: []*pb.LinuxHugepageLimit{{Pagesize: "2MB", Limit: 2 << 30}},
			},
			refused: "only 1GB huge pages",
		},
	} {
		assert := assert.New(t)
		err := k.boundContainerInGuest(tt.resources, tt.guest, tt.copied, "app")
		if tt.refused != "" {
			assert.Error(err, tt.description)
			assert.Contains(err.Error(), tt.refused, tt.description)
			continue
		}
		assert.NoError(err, tt.description)
		assert.Equal(tt.expectedLimit, tt.resources.Memory.Limit, tt.description)
		assert.Equal(tt.expectedHuge, hugePagesTotal(tt.resources.HugepageLimits, "1GB"), tt.description)
	}
}

func TestFoldHeldHugePages(t *testing.T) {
	const gib = int64(1) << 30
	assert := assert.New(t)
	saved := readVMHugepageSize
	t.Cleanup(func() { readVMHugepageSize = saved })
	readVMHugepageSize = func() (uint64, error) { return 1 << 30, nil }

	// An update for the memory alone keeps a container at its reservation
	// rather than at its nominal memory limit.
	resources := &pb.LinuxResources{Memory: &pb.LinuxMemory{Limit: 16 << 20}}
	foldHeldHugePages(resources, "1GB", 8<<30)
	k := &kataAgent{}
	assert.NoError(k.boundContainerInGuest(resources, guestCapacity{memoryMB: 16 * 1024, vCPUs: 4}, 0, "main"))
	assert.Equal(8*gib, resources.Memory.Limit)

	// An update that names the page size, zero or not, is left as it is.
	resources = &pb.LinuxResources{HugepageLimits: []*pb.LinuxHugepageLimit{{Pagesize: "1GB", Limit: 4 << 30}}}
	foldHeldHugePages(resources, "1GB", 8<<30)
	assert.Len(resources.HugepageLimits, 1)
	assert.Equal(uint64(4<<30), resources.HugepageLimits[0].Limit)

	resources = &pb.LinuxResources{}
	foldHeldHugePages(resources, "1GB", 0)
	assert.Empty(resources.HugepageLimits)
}

func TestGuestCeilingBytes(t *testing.T) {
	const gib = uint64(1) << 30
	assert := assert.New(t)

	assert.Equal(16*gib, guestCeilingBytes(podReservation{hugePages: 16 * gib, memory: 1 << 20}, 0), "a container is bounded by its pages")
	assert.Equal(2*gib, guestCeilingBytes(podReservation{memory: int64(2 * gib)}, 0), "a container without pages is bounded by its memory limit")
	assert.Equal(256*uint64(1<<20), guestCeilingBytes(podReservation{hugePages: 16 * gib, memory: 256 << 20}, 16*gib), "a copy is not a ceiling")
	assert.Equal(uint64(0), guestCeilingBytes(podReservation{}, 0), "no limit, no ceiling")
}

func TestRefusePodCeilingsPastGuest(t *testing.T) {
	const gib = uint64(1) << 30
	const mib = int64(1) << 20
	assert := assert.New(t)

	for _, tt := range []struct {
		description  string
		reservations []podReservation
		copied       uint64
		applied      map[string]uint64
		guest        guestCapacity
		refused      string
	}{
		{
			description:  "every container declares its pages and the pod leaves room for the guest",
			reservations: []podReservation{{hugePages: 16 * gib, memory: mib}, {hugePages: 2 * gib, memory: mib}, {hugePages: 2 * gib, memory: mib}},
			guest:        guestCapacity{memoryMB: 22 * 1024, vCPUs: 6},
		},
		{
			description:  "a sidecar's memory is not in the VM's size",
			reservations: []podReservation{{hugePages: 16 * gib, memory: 2048 * mib}, {memory: 2048 * mib}},
			guest:        guestCapacity{memoryMB: 18 * 1024, vCPUs: 5},
			refused:      "the pod needs",
		},
		{
			// a was created alone and given the pod level copy as its ceiling;
			// once b arrives the values reveal the copy, but a keeps what it got.
			description:  "a running container counts with the ceiling it was given",
			reservations: []podReservation{{id: "a", hugePages: 16 * gib, memory: 512 * mib}, {id: "self", hugePages: 10 * gib, memory: mib}},
			copied:       16 * gib,
			applied:      map[string]uint64{"a": 16 * gib},
			guest:        guestCapacity{memoryMB: 24 * 1024, vCPUs: 3},
			refused:      "the pod needs",
		},
		{
			description:  "the container being checked counts with its new ceiling",
			reservations: []podReservation{{id: "self", hugePages: 4 * gib, memory: mib}},
			applied:      map[string]uint64{"self": 64 * gib},
			guest:        guestCapacity{memoryMB: 6 * 1024, vCPUs: 2},
		},
		{
			description:  "a guest of unknown size settles nothing",
			reservations: []podReservation{{hugePages: 64 * gib}},
		},
		{
			description:  "a container without a ceiling adds nothing",
			reservations: []podReservation{{id: "pause"}, {hugePages: 4 * gib}, {id: "side"}},
			guest:        guestCapacity{memoryMB: 6 * 1024, vCPUs: 2},
		},
		{
			// A pod level 8Gi copied into b reads as b's own while the sum fits.
			description:  "a copy the values cannot reveal is refused as a declaration",
			reservations: []podReservation{{hugePages: 1 * gib, memory: 512 * mib}, {hugePages: 8 * gib, memory: 512 * mib}},
			guest:        guestCapacity{memoryMB: 10 * 1024, vCPUs: 3},
			refused:      "the pod needs 123 MiB more",
		},
	} {
		err := refusePodCeilingsPastGuest(tt.reservations, tt.copied, tt.applied, tt.guest, "self", "x")
		if tt.refused != "" {
			assert.Error(err, tt.description)
			assert.Contains(err.Error(), tt.refused, tt.description)
			continue
		}
		assert.NoError(err, tt.description)
	}
}

func TestRelinkContainerConfigs(t *testing.T) {
	assert := assert.New(t)
	s := &Sandbox{config: &SandboxConfig{HypervisorConfig: HypervisorConfig{HugePages: true, HugePagesPodSizing: true}}, containers: map[string]*Container{}}
	for _, id := range []string{"a", "b", "c"} {
		s.config.Containers = append(s.config.Containers, ContainerConfig{ID: id})
		s.relinkContainerConfigs()
		s.containers[id] = &Container{id: id, config: &s.config.Containers[len(s.config.Containers)-1]}
	}
	s.config.Containers = append(s.config.Containers[:0], s.config.Containers[1:]...)
	delete(s.containers, "a")
	s.relinkContainerConfigs()
	for _, id := range []string{"b", "c"} {
		assert.Equal(id, s.containers[id].config.ID, "a delete shifts the entries after it")
	}
}

func TestAmbiguousCopiedReservation(t *testing.T) {
	const gib = uint64(1) << 30
	assert := assert.New(t)

	value, ambiguous := ambiguousCopiedReservation([]podReservation{{hugePages: 16 * gib}, {hugePages: 16 * gib}}, 18*gib)
	assert.True(ambiguous, "the same value twice past the VM")
	assert.Equal(16*gib, value)

	_, ambiguous = ambiguousCopiedReservation([]podReservation{{hugePages: 10 * gib}, {hugePages: 16 * gib}, {hugePages: 16 * gib}}, 18*gib)
	assert.False(ambiguous, "a smaller value tells the copies apart")
	_, ambiguous = ambiguousCopiedReservation([]podReservation{{hugePages: 16 * gib}}, 18*gib)
	assert.False(ambiguous, "a lone container")
	_, ambiguous = ambiguousCopiedReservation([]podReservation{{hugePages: 8 * gib}, {hugePages: 8 * gib}, {}}, 18*gib)
	assert.False(ambiguous, "values that fit the VM are declarations")
	_, ambiguous = ambiguousCopiedReservation([]podReservation{{hugePages: 8 * gib}, {hugePages: 8 * gib}}, 0)
	assert.False(ambiguous, "a guest of unknown size settles nothing")
	_, ambiguous = ambiguousCopiedReservation([]podReservation{{hugePages: 4 * gib}, {hugePages: 4 * gib}}, 2*gib)
	assert.False(ambiguous, "a VM smaller than the value was not sized from the pod")
}

func TestRecordApplied(t *testing.T) {
	const gib = uint64(1) << 30
	assert := assert.New(t)
	k := &kataAgent{}
	k.recordApplied("a", &pb.LinuxResources{Memory: &pb.LinuxMemory{Limit: int64(2 * gib)}})
	assert.Equal(2*gib, k.appliedCopy()["a"])
	k.recordApplied("a", &pb.LinuxResources{Memory: &pb.LinuxMemory{Limit: int64(8 * gib)}})
	assert.Equal(8*gib, k.appliedCopy()["a"], "an update replaces the ceiling")
	k.recordApplied("a", &pb.LinuxResources{CPU: &pb.LinuxCPU{Quota: 100000}})
	assert.Equal(8*gib, k.appliedCopy()["a"], "an update without memory keeps the ceiling")
	k.recordApplied("u", nil)
	ceiling, ok := k.appliedCopy()["u"]
	assert.True(ok)
	assert.Equal(uint64(0), ceiling, "no limit, no ceiling")
}

func TestRelinkAfterRefusedDuplicate(t *testing.T) {
	assert := assert.New(t)
	s := &Sandbox{config: &SandboxConfig{HypervisorConfig: HypervisorConfig{HugePages: true, HugePagesPodSizing: true}}, containers: map[string]*Container{}}
	s.config.Containers = append(s.config.Containers, ContainerConfig{ID: "a", Annotations: map[string]string{"v": "kept"}})
	s.containers["a"] = &Container{id: "a"}
	s.relinkContainerConfigs()
	// A second create with the same ID appends, relinks, then rolls back.
	s.config.Containers = append(s.config.Containers, ContainerConfig{ID: "a", Annotations: map[string]string{"v": "refused"}})
	s.relinkContainerConfigs()
	s.config.Containers = s.config.Containers[:len(s.config.Containers)-1]
	s.relinkContainerConfigs()
	assert.Equal("kept", s.containers["a"].config.Annotations["v"])
}

func TestRelinkLeavesASandboxNotSizedFromThePod(t *testing.T) {
	assert := assert.New(t)
	s := &Sandbox{config: &SandboxConfig{}, containers: map[string]*Container{}}
	s.config.Containers = []ContainerConfig{{ID: "a"}}
	held := &ContainerConfig{ID: "a"}
	s.containers["a"] = &Container{id: "a", config: held}
	s.relinkContainerConfigs()
	assert.Same(held, s.containers["a"].config, "main behaviour is kept with the option off")
}
