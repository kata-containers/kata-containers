// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"fmt"

	"github.com/opencontainers/runc/libcontainer/devices"
	"github.com/opencontainers/runtime-spec/specs-go"
	"golang.org/x/sys/unix"
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
