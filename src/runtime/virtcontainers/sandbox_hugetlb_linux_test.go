// Copyright (c) 2026 NAVER Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"errors"
	"math"
	"os"
	"testing"

	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	resCtrl "github.com/kata-containers/kata-containers/src/runtime/pkg/resourcecontrol"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

const (
	testHugetlbSandboxCgroup = "kubepods-podabc.slice:cri-containerd:deadbeef"
	testHugetlbPodCgroup     = "/kubepods.slice/kubepods-podabc.slice"
	testHugetlbDefaultMemMB  = uint32(4096)
	testHugetlbPageSize      = uint64(1) << 30
)

// The least Sandbox carrying a sandbox container whose OCI spec names a cgroup,
// which is what the sizing reads. QEMU over virtio-fs, as the shipped configs do.
func hugetlbSizingSandbox(hugePages, static, sandboxCgroupOnly bool) (*Sandbox, *SandboxConfig) {
	sbc := &SandboxConfig{
		ID:                 "hugetlb-sizing",
		HypervisorType:     QemuHypervisor,
		StaticResourceMgmt: static,
		SandboxCgroupOnly:  sandboxCgroupOnly,
		HypervisorConfig: HypervisorConfig{
			HugePages:          hugePages,
			HugePagesPodSizing: true,
			MemorySize:         testHugetlbDefaultMemMB,
			SharedFS:           config.VirtioFS,
		},
		Containers: []ContainerConfig{{
			ID: "pause",
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(PodSandbox),
			},
			CustomSpec: &specs.Spec{
				Linux: &specs.Linux{CgroupsPath: testHugetlbSandboxCgroup},
			},
		}},
	}
	return &Sandbox{id: sbc.ID, config: sbc}, sbc
}

// stubHugetlbHost points the sizing at a fake host for the test's lifetime
// and hands back where the cgroup path it was asked about is recorded.
func stubHugetlbHost(t *testing.T, pageSizeErr error, limit uint64, stated bool, limitErr error) *string {
	savedLimit, savedSize := readPodHugetlbLimit, readVMHugepageSize
	t.Cleanup(func() {
		readPodHugetlbLimit, readVMHugepageSize = savedLimit, savedSize
	})

	path := new(string)
	readVMHugepageSize = func() (uint64, error) { return testHugetlbPageSize, pageSizeErr }
	readPodHugetlbLimit = func(podCgroupPath string, size uint64) (uint64, bool, error) {
		*path = podCgroupPath
		return limit, stated, limitErr
	}
	return path
}

func TestQemuGuestMemoryPreallocated(t *testing.T) {
	for name, tc := range map[string]struct {
		hc   HypervisorConfig
		want bool
	}{
		"asked for":                        {HypervisorConfig{MemPrealloc: true}, true},
		"huge pages shared over virtio-fs": {HypervisorConfig{HugePages: true, SharedFS: config.VirtioFS}, true},
		"huge pages shared over nydus":     {HypervisorConfig{HugePages: true, SharedFS: config.VirtioFSNydus}, true},
		"huge pages without a shared fs":   {HypervisorConfig{HugePages: true, SharedFS: config.NoSharedFS}, false},
		"virtio-fs without huge pages":     {HypervisorConfig{SharedFS: config.VirtioFS}, false},
		"nothing asks for it":              {HypervisorConfig{}, false},
	} {
		t.Run(name, func(t *testing.T) {
			assert.Equal(t, tc.want, qemuGuestMemoryPreallocated(&tc.hc))
		})
	}
}

func TestHypervisorBacksGuestRAMWithHugePages(t *testing.T) {
	for name, tc := range map[string]struct {
		hypervisorType HypervisorType
		want           bool
	}{
		"qemu":             {QemuHypervisor, true},
		"cloud hypervisor": {ClhHypervisor, true},
		"firecracker":      {FirecrackerHypervisor, false},
		"stratovirt":       {StratovirtHypervisor, false},
		"remote":           {RemoteHypervisor, false},
		"mock":             {MockHypervisor, false},
	} {
		t.Run(name, func(t *testing.T) {
			assert.Equal(t, tc.want, hypervisorBacksGuestRAMWithHugePages(tc.hypervisorType))
		})
	}
}

func TestSizeHugepageBackedVMFromPodTakesTheReservation(t *testing.T) {
	assert := assert.New(t)
	gotPath := stubHugetlbHost(t, nil, 64<<30, true, nil)

	s, sbc := hugetlbSizingSandbox(true, true, true)
	assert.NoError(s.sizeHugepageBackedVMFromPod(sbc))
	assert.Equal(uint32(64*1024), sbc.HypervisorConfig.MemorySize)
	// The allowance is read from the pod's cgroup.
	assert.Equal(testHugetlbPodCgroup, *gotPath)
}

func TestSizeHugepageBackedVMFromPodNeedsTheOption(t *testing.T) {
	assert := assert.New(t)
	stubHugetlbHost(t, nil, 0, true, nil)

	// Without enable_hugepages_pod_sizing the VM keeps default_memory, and a
	// pod that reserved nothing starts.
	s, sbc := hugetlbSizingSandbox(true, true, true)
	sbc.HypervisorConfig.HugePagesPodSizing = false
	assert.NoError(s.sizeHugepageBackedVMFromPod(sbc))
	assert.Equal(uint32(testHugetlbDefaultMemMB), sbc.HypervisorConfig.MemorySize)
}

func TestSizeHugepageBackedVMFromPodRefusesAShortReservation(t *testing.T) {
	for name, tc := range map[string]struct {
		limit uint64
		want  []string
		tweak func(*SandboxConfig)
	}{
		"none": {limit: 0, want: []string{"reserved none", "pod overhead"}},
		// The message carries both numbers the operator has to reconcile.
		"below what a guest needs": {limit: 512 << 20, want: []string{"512 MiB", "1024 MiB a guest needs"}},
		// The floor is the guest's. Preallocation leaves it the same.
		"below what a guest needs, no preallocation": {limit: 512 << 20, want: []string{"512 MiB", "1024 MiB a guest needs"},
			tweak: func(c *SandboxConfig) { c.HypervisorConfig.SharedFS = config.NoSharedFS }},
	} {
		t.Run(name, func(t *testing.T) {
			assert := assert.New(t)
			stubHugetlbHost(t, nil, tc.limit, true, nil)

			s, sbc := hugetlbSizingSandbox(true, true, true)
			if tc.tweak != nil {
				tc.tweak(sbc)
			}
			err := s.sizeHugepageBackedVMFromPod(sbc)
			assert.Error(err)
			for _, want := range tc.want {
				assert.Contains(err.Error(), want)
			}
			// The message must point at the Kubernetes resource, spelled the
			// way Kubernetes spells it, and at the cgroup that was read.
			assert.Contains(err.Error(), "hugepages-1Gi")
			assert.Contains(err.Error(), testHugetlbPodCgroup)
			assert.Equal(testHugetlbDefaultMemMB, sbc.HypervisorConfig.MemorySize)
		})
	}
}

// A pod that reserves less than default_memory gets a smaller guest.
func TestSizeHugepageBackedVMFromPodSizesBelowDefaultMemory(t *testing.T) {
	for name, tweak := range map[string]func(*SandboxConfig){
		"":                   nil,
		", no preallocation": func(c *SandboxConfig) { c.HypervisorConfig.SharedFS = config.NoSharedFS },
	} {
		t.Run("a reservation under default_memory sizes the VM"+name, func(t *testing.T) {
			assert := assert.New(t)
			stubHugetlbHost(t, nil, 2<<30, true, nil)

			s, sbc := hugetlbSizingSandbox(true, true, true)
			if tweak != nil {
				tweak(sbc)
			}
			assert.NoError(s.sizeHugepageBackedVMFromPod(sbc))
			assert.Equal(uint32(2048), sbc.HypervisorConfig.MemorySize)
		})
	}
}

func TestSizeHugepageBackedVMFromPodRefusesMoreThanDefaultMaxMemory(t *testing.T) {
	assert := assert.New(t)
	stubHugetlbHost(t, nil, 16<<30, true, nil)

	s, sbc := hugetlbSizingSandbox(true, true, true)
	sbc.HypervisorConfig.DefaultMaxMemorySize = 8192
	err := s.sizeHugepageBackedVMFromPod(sbc)
	assert.Error(err)
	assert.Contains(err.Error(), "default_maxmemory")
	assert.Contains(err.Error(), "hugepages-1Gi")
	assert.Equal(testHugetlbDefaultMemMB, sbc.HypervisorConfig.MemorySize)

	// An unset default_maxmemory is derived from the host later on and does
	// not bound the reservation here.
	sbc.HypervisorConfig.DefaultMaxMemorySize = 0
	assert.NoError(s.sizeHugepageBackedVMFromPod(sbc))
	assert.Equal(uint32(16*1024), sbc.HypervisorConfig.MemorySize)
}

func TestSizeHugepageBackedVMFromPodLeavesOtherGuests(t *testing.T) {
	assert := assert.New(t)
	stubHugetlbHost(t, nil, 64<<30, true, nil)

	// A guest outside the huge page pool keeps default_memory, whatever else is set.
	s, sbc := hugetlbSizingSandbox(false, false, false)
	assert.NoError(s.sizeHugepageBackedVMFromPod(sbc))
	assert.Equal(testHugetlbDefaultMemMB, sbc.HypervisorConfig.MemorySize)
}

func TestSizeHugepageBackedVMFromPodRefusesWhatItCannotSize(t *testing.T) {
	for name, tc := range map[string]struct {
		static, sandboxCgroupOnly bool
		stated                    bool
		limit                     uint64
		limitErr                  error
		pageSizeErr               error
		tweak                     func(*SandboxConfig)
		want                      string
	}{
		"not statically sized":              {static: false, sandboxCgroupOnly: true, stated: true, want: "static_sandbox_resource_mgmt"},
		"hypervisor outside the pod cgroup": {static: true, sandboxCgroupOnly: false, stated: true, want: "sandbox_cgroup_only"},
		"hypervisor without the huge page pool": {static: true, sandboxCgroupOnly: true, stated: true, want: "firecracker",
			tweak: func(c *SandboxConfig) { c.HypervisorType = FirecrackerHypervisor }},
		"cgroup v1":     {static: true, sandboxCgroupOnly: true, limitErr: resCtrl.ErrHugetlbCgroupV1, want: "cgroup v2"},
		"no limit file": {static: true, sandboxCgroupOnly: true, limitErr: os.ErrNotExist, want: "cannot read the pod's hugepages-1Gi reservation"},
		"unlimited":     {static: true, sandboxCgroupOnly: true, stated: false, want: "no finite hugepages-1Gi limit the VM can be sized from (it reads max)"},
		"unlimited as a number": {static: true, sandboxCgroupOnly: true, stated: true, limit: math.MaxInt64 &^ 4095,
			want: "no finite hugepages-1Gi limit the VM can be sized from (it reads 9223372036854771712)"},
		"beyond a VM": {static: true, sandboxCgroupOnly: true, stated: true, limit: (uint64(math.MaxUint32) + 1) << 20,
			want: "no finite hugepages-1Gi limit"},
		"allowance unreadable":   {static: true, sandboxCgroupOnly: true, limitErr: errors.New("boom"), want: "boom"},
		"huge page size unknown": {static: true, sandboxCgroupOnly: true, stated: true, pageSizeErr: errors.New("boom"), want: "huge page size"},
		"no sandbox cgroup": {static: true, sandboxCgroupOnly: true, stated: true, want: "cgroup path",
			tweak: func(c *SandboxConfig) { c.Containers[0].CustomSpec.Linux.CgroupsPath = "" }},
		"no sandbox container": {static: true, sandboxCgroupOnly: true, stated: true, want: "cgroup path",
			tweak: func(c *SandboxConfig) { c.Containers = nil }},
	} {
		t.Run(name, func(t *testing.T) {
			assert := assert.New(t)
			limit := tc.limit
			if limit == 0 {
				limit = 64 << 30
			}
			stubHugetlbHost(t, tc.pageSizeErr, limit, tc.stated, tc.limitErr)

			s, sbc := hugetlbSizingSandbox(true, tc.static, tc.sandboxCgroupOnly)
			if tc.tweak != nil {
				tc.tweak(sbc)
			}
			assert.ErrorContains(s.sizeHugepageBackedVMFromPod(sbc), tc.want)
			assert.Equal(testHugetlbDefaultMemMB, sbc.HypervisorConfig.MemorySize)
		})
	}
}

func TestSizeHugepageBackedVMFromPodSizesOnlyANewSandbox(t *testing.T) {
	assert := assert.New(t)
	// A stated zero allowance would refuse a new sandbox; a fetched one keeps
	// the size its VM already runs with and is not looked at again.
	gotPath := stubHugetlbHost(t, nil, 0, true, nil)

	s, sbc := hugetlbSizingSandbox(true, true, true)
	s.state.State = types.StateRunning
	assert.NoError(s.sizeHugepageBackedVMFromPod(sbc))
	assert.Equal(testHugetlbDefaultMemMB, sbc.HypervisorConfig.MemorySize)
	assert.Empty(*gotPath)
}
