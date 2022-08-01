// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"context"
	"fmt"
	"path/filepath"
	"strings"

	"github.com/containerd/cgroups"
	systemdDbus "github.com/coreos/go-systemd/v22/dbus"
	"github.com/godbus/dbus/v5"
	"github.com/opencontainers/runc/libcontainer/cgroups/systemd"
)

// DefaultResourceControllerID runtime-determined location in the cgroups hierarchy.
const DefaultResourceControllerID = "/vc"

// ValidCgroupPathV1 returns a valid cgroup path for cgroup v1.
// see https://github.com/opencontainers/runtime-spec/blob/master/config-linux.md#cgroups-path
func ValidCgroupPathV1(path string, systemdCgroup bool) (string, error) {
	if IsSystemdCgroup(path) {
		return path, nil
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

// ValidCgroupPathV2 returns a valid cgroup path for cgroup v2.
// see https://github.com/opencontainers/runtime-spec/blob/master/config-linux.md#cgroups-path
func ValidCgroupPathV2(path string, systemdCgroup bool) (string, error) {
	// In cgroup v2，path must be a "clean" absolute path starts with "/".
	if IsSystemdCgroup(path) {
		return filepath.Join("/", path), nil
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

func cgroupHierarchy(path string) (cgroups.Hierarchy, cgroups.Path, error) {
	if !IsSystemdCgroup(path) {
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
		newProperty("IOAccounting", true),
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
