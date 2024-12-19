// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"context"
	"fmt"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"

	"github.com/containerd/cgroups"
	systemdDbus "github.com/coreos/go-systemd/v22/dbus"
	"github.com/godbus/dbus/v5"
	"github.com/opencontainers/runc/libcontainer/cgroups/systemd"
	"golang.org/x/sys/unix"
)

// DefaultResourceControllerID runtime-determined location in the cgroups hierarchy.
const DefaultResourceControllerID = "/vc"

// ValidCgroupPath returns a valid cgroup path.
// see https://github.com/opencontainers/runtime-spec/blob/master/config-linux.md#cgroups-path
func ValidCgroupPath(path string, isCgroupV2 bool, systemdCgroup bool) (string, error) {
	if IsSystemdCgroup(path) {
		if isCgroupV2 {
			return filepath.Join("/", path), nil
		} else {
			return path, nil
		}
	}

	if systemdCgroup {
		return "", fmt.Errorf("malformed systemd path '%v': expected to be of form 'slice:prefix:name'", path)
	}

	// In the case of an absolute path (starting with /), the runtime MUST
	// take the path to be relative to the cgroups mount point.
	if filepath.IsAbs(path) {
		return filepath.Clean(path), nil
	}

	// In the case of a relative path (not starting with /), the runtime MAY
	// interpret the path relative to a runtime-determined location in the cgroups hierarchy.
	// clean up path and return a new path relative to DefaultResourceControllerID
	return filepath.Join(DefaultResourceControllerID, filepath.Clean("/"+path)), nil
}

func newProperty(name string, units interface{}) systemdDbus.Property {
	return systemdDbus.Property{
		Name:  name,
		Value: dbus.MakeVariant(units),
	}
}

func cgroupHierarchy(path string, sandboxCgroupOnly bool) (cgroups.Hierarchy, cgroups.Path, error) {
	if !IsSystemdCgroup(path) || !sandboxCgroupOnly {
		return cgroups.V1, cgroups.StaticPath(path), nil
	} else {
		slice, unit, err := getSliceAndUnit(path)
		if err != nil {
			return nil, nil, err
		}

		cgroupSlicePath, _ := systemd.ExpandSlice(slice)
		if err != nil {
			return nil, nil, err
		}

		return cgroups.Systemd, cgroups.Slice(cgroupSlicePath, unit), nil
	}
}

// checkSystemd should be used temporarily to decide
// available resource control properties.
// This is not ideal but is temporary fixed for deciding between subsystem IOAccounting and BlockIOAccounting
func ParseSystemdVersion(systemdVersion string) (int, error) {
	// version get returned as quoted string: "219" instead of 219
	ver := strings.Trim(systemdVersion, `"`)
	re := regexp.MustCompile(`^(\d+)`)
	subStringMatch := re.FindStringSubmatch(ver)
	if len(subStringMatch) < 2 {
		return 0, fmt.Errorf("error parsing systemd version with regex, substring: %v", subStringMatch)
	}
	version, err := strconv.Atoi(subStringMatch[1])
	if err != nil {
		return 0, fmt.Errorf("error parsing systemd version: %v", err)
	}

	return version, nil
}

func createCgroupsSystemd(slice string, unit string, pid int) error {
	ctx := context.TODO()
	conn, err := systemdDbus.NewWithContext(ctx)
	if err != nil {
		return err
	}
	defer conn.Close()

	properties := []systemdDbus.Property{
		systemdDbus.PropDescription("cgroup " + unit),
		newProperty("DefaultDependencies", false),
		newProperty("MemoryAccounting", true),
		newProperty("CPUAccounting", true),
	}

	systemdVersion, err := conn.GetManagerProperty("Version")
	if err != nil {
		return fmt.Errorf("error getting systemd version: %v", err)
	}

	var ver int
	if ver, err = ParseSystemdVersion(systemdVersion); err != nil {
		return err
	}
	// BlockIOAccounting has been replaced with IOAccounting in newer version
	// if IOAccounting is used it breaks the cgroup creation in older versions of systemd.
	// Below is changelog for systemd, when IOAccounting was introduced:
	// https://github.com/systemd/systemd/commit/13c31542cc57e1454dccd6383bfdac98cbee5bb1#diff-8bd72c8fe1849563e9978d5f71[…]ee9371c9e94a0e35159a5735244c216
	if ver < 252 {
		properties = append(properties, newProperty("BlockIOAccounting", true))
	} else {
		properties = append(properties, newProperty("IOAccounting", true))
	}

	if strings.HasSuffix(unit, ".slice") {
		// If we create a slice, the parent is defined via a Wants=.
		properties = append(properties, systemdDbus.PropWants(slice))
	} else {
		// Otherwise it's a scope, which we put into a Slice=.
		properties = append(properties, systemdDbus.PropSlice(slice))
	}

	// Assume scopes always support delegation (supported since systemd v218).
	properties = append(properties, newProperty("Delegate", true))

	if pid != -1 {
		properties = append(properties, systemdDbus.PropPids(uint32(pid)))
	}

	ch := make(chan string)
	// https://www.freedesktop.org/wiki/Software/systemd/ControlGroupInterface/
	_, err = conn.StartTransientUnitContext(ctx, unit, "replace", properties, ch)
	if err != nil {
		return err
	}
	<-ch
	return nil
}

func getSliceAndUnit(cgroupPath string) (string, string, error) {
	parts := strings.Split(cgroupPath, ":")
	if len(parts) == 3 && strings.HasSuffix(parts[0], ".slice") {
		return parts[0], fmt.Sprintf("%s-%s.scope", parts[1], parts[2]), nil
	}

	return "", "", fmt.Errorf("Path: %s is not valid systemd's cgroups path", cgroupPath)
}

func IsCgroupV1() (bool, error) {
	if cgroups.Mode() == cgroups.Legacy || cgroups.Mode() == cgroups.Hybrid {
		return true, nil
	} else if cgroups.Mode() == cgroups.Unified {
		return false, nil
	} else {
		return false, ErrCgroupMode
	}
}

func SetThreadAffinity(threadID int, cpuSetSlice []int) error {
	unixCPUSet := unix.CPUSet{}

	for _, cpuId := range cpuSetSlice {
		unixCPUSet.Set(cpuId)
	}

	if err := unix.SchedSetaffinity(threadID, &unixCPUSet); err != nil {
		return fmt.Errorf("failed to set vcpu thread %d affinity to cpu %d: %v", threadID, cpuSetSlice, err)
	}

	return nil
}
