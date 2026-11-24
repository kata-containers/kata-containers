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

// ErrHugetlbCgroupV1 is returned on a cgroup v1 host. Only the unified hierarchy is read.
var ErrHugetlbCgroupV1 = errors.New("the hugetlb allowance is read from cgroup v2 only")

// HugetlbSizeName matches the kernel's hugetlb limit filenames.
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

// HugepagesResourceName uses Kubernetes resource names so error messages can
// be matched to the pod spec.
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

// PodCgroupPath finds the parent of OCI Linux.CgroupsPath. Under a CRI, the
// kubelet sets the pod's resource limits there.
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

// HugetlbLimitBytes reads the pod limit for hugepageSize.
// The bool reports a finite limit, including zero. A missing file is an error.
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
