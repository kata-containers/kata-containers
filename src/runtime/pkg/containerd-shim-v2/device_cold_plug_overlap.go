// Copyright (c) 2026 NAVER Cloud Corp.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/container-orchestrated-devices/container-device-interface/pkg/cdi"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
)

// sysRoot and devRoot are variables so tests can point device-node
// resolution at a temp-dir fixture.
var (
	sysRoot = "/sys"
	devRoot = "/dev"
)

// checkCrossSourcePhysicalOverlap errors when a device-plugin CDI device and
// a DRA CDI device resolve to the same underlying physical device. The same
// device can be reachable via a legacy iommu-group cdev on one side and a
// per-device vfio cdev on the other, so string-comparing CDI names misses a
// double plug; compare physical coordinates (PCI BDF or mdev UUID) instead.
func checkCrossSourcePhysicalOverlap(devicePluginDevs, draDevs []string) error {
	if len(devicePluginDevs) == 0 || len(draDevs) == 0 {
		return nil
	}

	dpCoords, err := resolvePhysicalCoords(devicePluginDevs)
	if err != nil {
		return err
	}
	draCoords, err := resolvePhysicalCoords(draDevs)
	if err != nil {
		return err
	}

	for coord, dpName := range dpCoords {
		if draName, ok := draCoords[coord]; ok {
			return fmt.Errorf(
				"cold plug: physical device %q is reachable via both pod_resource_device_sources: "+
					"%q (device-plugin CDI device %q) and %q (dra CDI device %q); "+
					"this would double cold-plug the same underlying device",
				coord, oci.PodResourceDeviceSourceDevicePlugin, dpName, oci.PodResourceDeviceSourceDRA, draName)
		}
	}

	return nil
}

// resolvePhysicalCoords maps CDI devices to physical coordinates (BDF or
// mdev UUID). Unresolvable names are skipped (InjectCDIDevices already
// rejects them); a path that cannot be normalized is an error: the guard
// cannot prove it does not overlap.
func resolvePhysicalCoords(cdiDevs []string) (map[string]string, error) {
	coords := make(map[string]string)

	registry := cdi.GetRegistry()
	for _, name := range cdiDevs {
		dev := registry.DeviceDB().GetDevice(name)
		if dev == nil || dev.Device == nil {
			// A nil embedded spec would nil-panic on ContainerEdits below.
			continue
		}

		for _, node := range dev.ContainerEdits.DeviceNodes {
			if node == nil || node.Path == "" {
				continue
			}
			coord, err := normalizeDeviceNodePath(node.Path)
			if err != nil {
				return nil, fmt.Errorf("cold plug: failed to normalize device node path for overlap check (device %q): %w", name, err)
			}
			for _, c := range coord {
				if _, exists := coords[c]; !exists {
					coords[c] = name
				}
			}
		}
	}

	return coords, nil
}

// normalizeDeviceNodePath maps a device node path to physical coordinate
// keys. A legacy /dev/vfio/N group cdev expands to every BDF in the group,
// since any member could alias a per-device cdev from the other source;
// non-vfio paths are their own key.
func normalizeDeviceNodePath(path string) ([]string, error) {
	vfioDevicesDir := filepath.Join(devRoot, "vfio", "devices")
	vfioGroupDir := filepath.Join(devRoot, "vfio")

	if filepath.Dir(path) == vfioDevicesDir {
		return normalizeVFIODeviceCdev(path)
	}

	if filepath.Dir(path) == vfioGroupDir {
		base := filepath.Base(path)
		if isIOMMUGroup(base) {
			return normalizeIOMMUGroupCdev(base)
		}
	}

	return []string{path}, nil
}

// normalizeVFIODeviceCdev resolves /dev/vfio/devices/vfioN via its sysfs
// device symlink; the target basename is the device's own identity (PCI BDF,
// or mdev instance UUID -- never the parent), so distinct mdev slices of one
// parent never collide. Mdevs are assumed cdev-only: group-node vs cdev
// aliasing of an mdev is out of scope.
func normalizeVFIODeviceCdev(path string) ([]string, error) {
	vfioName := filepath.Base(path)

	link := filepath.Join(sysRoot, "class", "vfio-dev", vfioName, "device")
	target, err := os.Readlink(link)
	if err != nil {
		return nil, fmt.Errorf("failed to readlink %s: %w", link, err)
	}

	return []string{filepath.Base(target)}, nil
}

// normalizeIOMMUGroupCdev returns the BDFs of every device in the IOMMU group.
func normalizeIOMMUGroupCdev(group string) ([]string, error) {
	devicesDir := filepath.Join(sysRoot, "kernel", "iommu_groups", group, "devices")
	entries, err := os.ReadDir(devicesDir)
	if err != nil {
		return nil, fmt.Errorf("failed to list %s: %w", devicesDir, err)
	}

	var bdfs []string
	for _, e := range entries {
		bdfs = append(bdfs, e.Name())
	}

	return bdfs, nil
}

func isIOMMUGroup(base string) bool {
	return base != "" && strings.TrimLeft(base, "0123456789") == ""
}
