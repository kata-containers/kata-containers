// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"errors"
	"fmt"
	"strings"

	"github.com/opencontainers/runc/libcontainer/devices"
	"github.com/opencontainers/runtime-spec/specs-go"
	"golang.org/x/sys/unix"
)

var (
	ErrCgroupMode = errors.New("cgroup controller type error")
)

func DeviceToCgroupDeviceRule(device string) (*devices.Rule, error) {
	var st unix.Stat_t
	deviceRule := devices.Rule{
		Allow:       true,
		Permissions: "rwm",
	}

	if err := unix.Stat(device, &st); err != nil {
		return nil, err
	}

	devType := st.Mode & unix.S_IFMT

	switch devType {
	case unix.S_IFCHR:
		deviceRule.Type = 'c'
	case unix.S_IFBLK:
		deviceRule.Type = 'b'
	default:
		return nil, fmt.Errorf("unsupported device type: %v", devType)
	}

	major := int64(unix.Major(uint64(st.Rdev)))
	minor := int64(unix.Minor(uint64(st.Rdev)))
	deviceRule.Major = major
	deviceRule.Minor = minor

	return &deviceRule, nil
}

func DeviceToLinuxDevice(device string) (specs.LinuxDeviceCgroup, error) {
	dev, err := DeviceToCgroupDeviceRule(device)
	if err != nil {
		return specs.LinuxDeviceCgroup{}, err
	}

	return specs.LinuxDeviceCgroup{
		Allow:  dev.Allow,
		Type:   string(dev.Type),
		Major:  &dev.Major,
		Minor:  &dev.Minor,
		Access: string(dev.Permissions),
	}, nil
}

func IsSystemdCgroup(cgroupPath string) bool {

	// If we are utilizing systemd to manage cgroups, we expect to receive a path
	// in the format slice:scopeprefix:name. A typical example would be:
	//
	// system.slice:docker:6b4c4a4d0cc2a12c529dcb13a2b8e438dfb3b2a6af34d548d7d
	//
	// Based on this, let's split by the ':' delimiter and verify that the first
	// section has .slice as a suffix.
	parts := strings.Split(cgroupPath, ":")
	if len(parts) == 3 && strings.HasSuffix(parts[0], ".slice") {
		return true
	}

	return false
}
