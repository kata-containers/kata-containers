// Copyright (c) 2026 NAVER Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/opencontainers/cgroups/systemd"
)

// HugetlbUnlimited is the value of a cgroup v2 hugetlb limit file with no limit set.
const HugetlbUnlimited = "max"

// ErrHugetlbCgroupV1 is returned on a cgroup v1 host, whose hugetlb controller is not read.
var ErrHugetlbCgroupV1 = errors.New("the hugetlb allowance is not read from a cgroup v1 hierarchy")

// HugetlbSizeName names a huge page size the way the kernel names its
// hugetlb.<name>.max files (mem_fmt in mm/hugetlb_cgroup.c): the unit is picked
// by threshold and the quotient truncated, so 1 GiB is "1GB" and 2 MiB "2MB".
func HugetlbSizeName(sizeBytes uint64) (string, error) {
	const (
		kb = uint64(1) << 10
		mb = uint64(1) << 20
		gb = uint64(1) << 30
	)

	if sizeBytes == 0 || sizeBytes%kb != 0 {
		return "", fmt.Errorf("huge page size %d is not a whole number of KB", sizeBytes)
	}

	switch {
	case sizeBytes >= gb:
		return fmt.Sprintf("%dGB", sizeBytes/gb), nil
	case sizeBytes >= mb:
		return fmt.Sprintf("%dMB", sizeBytes/mb), nil
	default:
		return fmt.Sprintf("%dKB", sizeBytes/kb), nil
	}
}

// HugepagesResourceName names a huge page size the way Kubernetes does:
// hugepages-2Mi, hugepages-1Gi. A size that is not a whole number of any unit
// falls back to the byte count.
func HugepagesResourceName(sizeBytes uint64) string {
	const kib = uint64(1) << 10
	for _, unit := range []struct {
		suffix string
		size   uint64
	}{
		{"Gi", kib * kib * kib},
		{"Mi", kib * kib},
		{"Ki", kib},
	} {
		if sizeBytes >= unit.size && sizeBytes%unit.size == 0 {
			return fmt.Sprintf("hugepages-%d%s", sizeBytes/unit.size, unit.suffix)
		}
	}
	return fmt.Sprintf("hugepages-%d", sizeBytes)
}

// PodCgroupPath returns the parent of the cgroup an OCI Linux.CgroupsPath names,
// relative to the cgroup mount point. Under a CRI that parent is the pod's cgroup,
// which the kubelet configures from the pod's resources.
func PodCgroupPath(cgroupPath string) (string, error) {
	if IsSystemdCgroup(cgroupPath) {
		slice, _, err := getSliceAndUnit(cgroupPath)
		if err != nil {
			return "", err
		}
		// The slice is the unit's parent; expanding its name gives its full path.
		expanded, err := systemd.ExpandSlice(slice)
		if err != nil {
			return "", fmt.Errorf("expand slice %q: %w", slice, err)
		}
		return expanded, nil
	}

	parent := filepath.Dir(filepath.Clean("/" + cgroupPath))
	if parent == "/" {
		return "", fmt.Errorf("cgroup path %q has no parent cgroup", cgroupPath)
	}
	return parent, nil
}

// HugetlbLimitBytes reads the hugetlb limit for pages of hugepageSize on the
// cgroup at podCgroupPath, relative to the cgroup v2 mount point. The bool is
// false when no limit is stated: the file is absent or holds "max". A limit of
// zero is a stated limit.
func HugetlbLimitBytes(podCgroupPath string, hugepageSize uint64) (uint64, bool, error) {
	isV1, err := IsCgroupV1()
	if err != nil {
		return 0, false, err
	}
	if isV1 {
		return 0, false, ErrHugetlbCgroupV1
	}
	return hugetlbLimitBytesAt(unifiedMountpoint, podCgroupPath, hugepageSize)
}

// hugetlbLimitBytesAt is HugetlbLimitBytes against a hierarchy mounted at root, for tests.
func hugetlbLimitBytesAt(root, podCgroupPath string, hugepageSize uint64) (uint64, bool, error) {
	sizeName, err := HugetlbSizeName(hugepageSize)
	if err != nil {
		return 0, false, err
	}

	limitPath := filepath.Join(root, podCgroupPath, fmt.Sprintf("hugetlb.%s.max", sizeName))
	raw, err := os.ReadFile(limitPath)
	if os.IsNotExist(err) {
		return 0, false, nil
	}
	if err != nil {
		return 0, false, err
	}

	value := strings.TrimSpace(string(raw))
	if value == HugetlbUnlimited {
		return 0, false, nil
	}

	limit, err := strconv.ParseUint(value, 10, 64)
	if err != nil {
		return 0, false, fmt.Errorf("parse %q: %w", limitPath, err)
	}
	return limit, true, nil
}
