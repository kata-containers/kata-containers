// Copyright (c) 2026 NAVER Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestHugetlbSizeName(t *testing.T) {
	assert := assert.New(t)

	for _, tc := range []struct {
		size uint64
		want string
	}{
		{64 << 10, "64KB"},
		{2 << 20, "2MB"},
		{32 << 20, "32MB"},
		{512 << 20, "512MB"},
		{1 << 30, "1GB"},
		{16 << 30, "16GB"},
	} {
		got, err := HugetlbSizeName(tc.size)
		assert.NoError(err, "size %d", tc.size)
		assert.Equal(tc.want, got, "size %d", tc.size)
	}

	for _, size := range []uint64{0, 1000, 4095} {
		_, err := HugetlbSizeName(size)
		assert.Error(err, "size %d", size)
	}
}

func TestHugepagesResourceName(t *testing.T) {
	assert := assert.New(t)
	assert.Equal("hugepages-2Mi", HugepagesResourceName(2<<20))
	assert.Equal("hugepages-1Gi", HugepagesResourceName(1<<30))
	assert.Equal("hugepages-64Ki", HugepagesResourceName(64<<10))
	assert.Equal("hugepages-1000", HugepagesResourceName(1000))
}

func TestPodCgroupPath(t *testing.T) {
	assert := assert.New(t)

	for _, tc := range []struct {
		path string
		want string
	}{
		// A Guaranteed pod's sandbox under the systemd driver.
		{"kubepods-pode1e09ca9_7402_4842_8778_b1b3d7b681b9.slice:cri-containerd:2e95aa80", "/kubepods.slice/kubepods-pode1e09ca9_7402_4842_8778_b1b3d7b681b9.slice"},
		// A Burstable pod sits one slice deeper.
		{"kubepods-burstable-pod3231f378.slice:cri-containerd:66596d08", "/kubepods.slice/kubepods-burstable.slice/kubepods-burstable-pod3231f378.slice"},
		// The cgroupfs driver states the whole path.
		{"/kubepods/pode1e09ca9/2e95aa80", "/kubepods/pode1e09ca9"},
		{"kubepods/pode1e09ca9/2e95aa80", "/kubepods/pode1e09ca9"},
	} {
		got, err := PodCgroupPath(tc.path)
		assert.NoError(err, tc.path)
		assert.Equal(tc.want, got, tc.path)
	}

	for _, path := range []string{"/2e95aa80", "2e95aa80", "/"} {
		_, err := PodCgroupPath(path)
		assert.Error(err, path)
	}
}

func TestHugetlbLimitBytesAt(t *testing.T) {
	assert := assert.New(t)

	root := t.TempDir()
	podCgroup := "/kubepods.slice/kubepods-podabc.slice"
	podDir := filepath.Join(root, podCgroup)
	assert.NoError(os.MkdirAll(podDir, 0o755))
	oneGB := uint64(1 << 30)

	write := func(name, value string) {
		assert.NoError(os.WriteFile(filepath.Join(podDir, name), []byte(value+"\n"), 0o644))
	}

	// No hugetlb controller on this cgroup: nothing is stated.
	limit, stated, err := hugetlbLimitBytesAt(root, podCgroup, oneGB)
	assert.NoError(err)
	assert.False(stated)
	assert.Zero(limit)

	// No limit set: nothing is stated either.
	write("hugetlb.1GB.max", HugetlbUnlimited)
	_, stated, err = hugetlbLimitBytesAt(root, podCgroup, oneGB)
	assert.NoError(err)
	assert.False(stated)

	// The pod reserved 192 pages of 1GB.
	write("hugetlb.1GB.max", "206158430208")
	limit, stated, err = hugetlbLimitBytesAt(root, podCgroup, oneGB)
	assert.NoError(err)
	assert.True(stated)
	assert.Equal(uint64(192<<30), limit)

	// The pod reserved no 1GB pages: stated, and zero.
	write("hugetlb.1GB.max", "0")
	limit, stated, err = hugetlbLimitBytesAt(root, podCgroup, oneGB)
	assert.NoError(err)
	assert.True(stated)
	assert.Zero(limit)

	// The 2MB file is read for 2MB pages, independently of the 1GB one.
	write("hugetlb.2MB.max", "4194304")
	limit, stated, err = hugetlbLimitBytesAt(root, podCgroup, 2<<20)
	assert.NoError(err)
	assert.True(stated)
	assert.Equal(uint64(4<<20), limit)

	// Garbage is an error, not a silent fallback.
	write("hugetlb.1GB.max", "lots")
	_, _, err = hugetlbLimitBytesAt(root, podCgroup, oneGB)
	assert.Error(err)

	// A page size the kernel could not name is an error too.
	_, _, err = hugetlbLimitBytesAt(root, podCgroup, 1000)
	assert.Error(err)
}
