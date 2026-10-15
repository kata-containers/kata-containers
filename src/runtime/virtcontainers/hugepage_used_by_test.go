// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"path/filepath"
	"testing"

	ctrAnnotations "github.com/containerd/containerd/pkg/cri/annotations"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"

	pb "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
)

func TestParseHugePagesUsedBy(t *testing.T) {
	assert := assert.New(t)

	usedBy, err := parseHugePagesUsedBy(" nvmf=guest , trainer=host ")
	assert.NoError(err)
	assert.Equal(map[string]string{"nvmf": HugePagesUsedByGuest, "trainer": HugePagesUsedByHost}, usedBy)

	usedBy, err = parseHugePagesUsedBy("")
	assert.NoError(err)
	assert.Empty(usedBy)

	for _, bad := range []string{"nvmf", "=guest", "nvmf=pool", "nvmf=guest,nvmf=host"} {
		_, err := parseHugePagesUsedBy(bad)
		assert.Error(err, bad)
	}

	assert.True(validHugePagesUsedBy(""))
	assert.True(validHugePagesUsedBy(HugePagesUsedByGuest))
	assert.False(validHugePagesUsedBy("pool"))
}

// A sandbox whose pause container carries the pod's annotations.
func usedBySandbox(classUsedBy, annotation string) *Sandbox {
	annotations := map[string]string{}
	if annotation != "" {
		annotations[vcAnnotations.ContainerHugePagesUsedBy] = annotation
	}
	return &Sandbox{config: &SandboxConfig{
		HypervisorConfig: HypervisorConfig{
			HugePages:          true,
			HugePagesPodSizing: true,
			HugePagesUsedBy:    classUsedBy,
		},
		Containers: []ContainerConfig{{
			ID:          "pause",
			Annotations: map[string]string{vcAnnotations.ContainerTypeKey: string(PodSandbox)},
			CustomSpec:  &specs.Spec{Annotations: annotations},
		}},
	}}
}

func usedByContainer(name, annotation string) *ContainerConfig {
	annotations := map[string]string{}
	if annotation != "" {
		annotations[vcAnnotations.ContainerHugePagesUsedBy] = annotation
	}
	return &ContainerConfig{
		ID:          name + "-id",
		Annotations: map[string]string{ctrAnnotations.ContainerName: name},
		CustomSpec:  &specs.Spec{Annotations: annotations},
	}
}

func TestContainerHugePagesUsedBy(t *testing.T) {
	const own = uint64(2) << 30
	assert := assert.New(t)

	for _, tt := range []struct {
		description string
		class       string
		annotation  string
		container   string
		own         uint64
		expected    string
	}{
		{"the default class is host", "", "", "app", own, HugePagesUsedByHost},
		{"a guest class gives every container that reserves pages to the guest", HugePagesUsedByGuest, "", "dpdk", own, HugePagesUsedByGuest},
		{"a guest class leaves a container that reserves none to the host", HugePagesUsedByGuest, "", "log", 0, HugePagesUsedByHost},
		{"a host class with a guest entry", HugePagesUsedByHost, "nvmf=guest", "nvmf", own, HugePagesUsedByGuest},
		{"a host class leaves unnamed containers alone", HugePagesUsedByHost, "nvmf=guest", "trainer", own, HugePagesUsedByHost},
		{"a guest class with a host entry", HugePagesUsedByGuest, "accel=host", "accel", own, HugePagesUsedByHost},
		{"an entry holds even for a container that reserves none", HugePagesUsedByHost, "nvmf=guest", "nvmf", 0, HugePagesUsedByGuest},
	} {
		sandbox := usedBySandbox(tt.class, tt.annotation)
		usedBy, err := containerHugePagesUsedBy(sandbox, usedByContainer(tt.container, tt.annotation), tt.own)
		assert.NoError(err, tt.description)
		assert.Equal(tt.expected, usedBy, tt.description)
	}

	pause := usedByContainer("pause", "")
	pause.Annotations[vcAnnotations.ContainerTypeKey] = string(PodSandbox)
	usedBy, err := containerHugePagesUsedBy(usedBySandbox(HugePagesUsedByGuest, ""), pause, own)
	assert.NoError(err)
	assert.Equal(HugePagesUsedByHost, usedBy, "a guest class does not give the sandbox container's pages to the guest")

	// The kubelet rebuilds a restarted container from the pod's current
	// annotations; the sandbox keeps the ones it started with.
	sandbox := usedBySandbox(HugePagesUsedByHost, "nvmf=guest")
	_, err = containerHugePagesUsedBy(sandbox, usedByContainer("nvmf", "nvmf=host"), own)
	assert.Error(err)
	assert.Contains(err.Error(), "cannot change while the pod runs")

	_, err = containerHugePagesUsedBy(usedBySandbox("", "nvmf=pool"), usedByContainer("nvmf", "nvmf=pool"), own)
	assert.Error(err)

	// In a guest sized from the pod the entry, not a mount, decides the pool.
	assert.True(containerHasGuestHugePagePool(sandbox, usedByContainer("nvmf", "nvmf=guest"), own))
	assert.False(containerHasGuestHugePagePool(sandbox, usedByContainer("trainer", "nvmf=guest"), own))
}

func TestGuestPoolTotalNeverShrinks(t *testing.T) {
	assert := assert.New(t)
	k := &kataAgent{}

	assert.Equal(uint64(4<<30), k.guestPoolTotal("1GB", 4<<30))
	assert.Equal(uint64(12<<30), k.guestPoolTotal("1GB", 12<<30))
	// A pool container that stopped no longer counts, but its pages may still be
	// held; the pool is not asked to shrink.
	assert.Equal(uint64(12<<30), k.guestPoolTotal("1GB", 8<<30))
	assert.Equal(uint64(2<<20), k.guestPoolTotal("2MB", 2<<20))
}

func TestGuestPoolStorage(t *testing.T) {
	assert := assert.New(t)

	s := guestPoolStorage("abc", 1<<30, 12<<30)
	assert.Equal("hugetlbfs", s.Fstype)
	assert.Equal(KataEphemeralDevType, s.Driver)
	assert.Equal(filepath.Join(ephemeralPath(), "kata-hugepage-pool-abc"), s.MountPoint)
	assert.Equal([]string{"pagesize=1073741824,size=12884901888"}, s.Options)
}

func TestStripPoolPagesAndOtherPageSize(t *testing.T) {
	assert := assert.New(t)

	limits := []*pb.LinuxHugepageLimit{
		{Pagesize: "2MB", Limit: 0},
		{Pagesize: "1GB", Limit: 8 << 30},
	}
	assert.Equal("", otherPageSizeReserved(limits, "1GB"))
	stripPoolPages(limits, "1GB")
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
		usedBy        string
		copied        uint64
		guest         guestCapacity
		resources     *pb.LinuxResources
		refused       string
		expectedLimit int64
		expectedHuge  uint64
	}{
		{
			description: "a host container is bounded by its reservation and takes no pool pages",
			usedBy:      HugePagesUsedByHost,
			guest:       guest,
			resources: &pb.LinuxResources{
				Memory:         &pb.LinuxMemory{Limit: 1 << 20},
				HugepageLimits: []*pb.LinuxHugepageLimit{{Pagesize: "1GB", Limit: 8 << 30}},
			},
			expectedLimit: 8 * gib,
			expectedHuge:  0,
		},
		{
			description: "a guest container keeps its reservation as its guest hugetlb limit",
			usedBy:      HugePagesUsedByGuest,
			guest:       guest,
			resources: &pb.LinuxResources{
				Memory:         &pb.LinuxMemory{Limit: 2 * gib},
				HugepageLimits: []*pb.LinuxHugepageLimit{{Pagesize: "1GB", Limit: 4 << 30}},
			},
			expectedLimit: 2 * gib,
			expectedHuge:  4 << 30,
		},
		{
			description: "a guest container's memory ceiling must fit after the pools",
			usedBy:      HugePagesUsedByGuest,
			guest:       guestCapacity{memoryMB: 16 * 1024, vCPUs: 4, pools: 12 << 30},
			resources: &pb.LinuxResources{
				Memory:         &pb.LinuxMemory{Limit: 4 * gib},
				HugepageLimits: []*pb.LinuxHugepageLimit{{Pagesize: "1GB", Limit: 12 << 30}},
			},
			refused: "the pod needs",
		},
		{
			description: "a copied reservation leaves the memory limit as the ceiling",
			usedBy:      HugePagesUsedByHost,
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
			usedBy:      HugePagesUsedByGuest,
			guest:       guest,
			resources: &pb.LinuxResources{
				Memory:         &pb.LinuxMemory{Limit: gib},
				HugepageLimits: []*pb.LinuxHugepageLimit{{Pagesize: "2MB", Limit: 2 << 30}},
			},
			refused: "only 1GB huge pages",
		},
	} {
		assert := assert.New(t)
		err := k.boundContainerInGuest(tt.resources, tt.guest, tt.copied, tt.usedBy, "app")
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

	// An update for the memory alone keeps a host container at its reservation
	// rather than at its nominal memory limit.
	resources := &pb.LinuxResources{Memory: &pb.LinuxMemory{Limit: 16 << 20}}
	foldHeldHugePages(resources, "1GB", 8<<30)
	k := &kataAgent{}
	assert.NoError(k.boundContainerInGuest(resources, guestCapacity{memoryMB: 16 * 1024, vCPUs: 4}, 0, HugePagesUsedByHost, "main"))
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
