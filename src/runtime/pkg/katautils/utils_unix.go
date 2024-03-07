// Copyright (c) 2022 NetEase Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"fmt"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"golang.org/x/sys/unix"
	"os"
)

func GetDeviceInfoByPath(devPath string) (config.DeviceInfo, error) {
	var devInfo *config.DeviceInfo
	stat, err := os.Stat(devPath)
	if err != nil {
		return *devInfo, fmt.Errorf("error stating device path: %w", err)
	}

	if stat.IsDir() {
		return *devInfo, fmt.Errorf("the device path is a directory: %s", devPath)
	}

	devInfo, err = DeviceFromPath(devPath)
	if err != nil {
		return *devInfo, err
	}
	return *devInfo, nil
}

const (
	wildcardDevice = "a" //nolint // currently unused, but should be included when upstreaming to OCI runtime-spec.
	blockDevice    = "b"
	charDevice     = "c" // or "u"
	fifoDevice     = "p"
)

// DeviceFromPath takes the path to a device to look up the information about a
// linux device and returns that information as a config.DeviceInfo struct.
func DeviceFromPath(path string) (*config.DeviceInfo, error) {
	var stat unix.Stat_t
	if err := unix.Lstat(path, &stat); err != nil {
		return nil, err
	}

	var (
		devNumber = uint64(stat.Rdev) //nolint: unconvert // the type is 32bit on mips.
		major     = unix.Major(devNumber)
		minor     = unix.Minor(devNumber)
	)

	var (
		devType string
		mode    = stat.Mode
	)

	switch mode & unix.S_IFMT {
	case unix.S_IFBLK:
		devType = blockDevice
	case unix.S_IFCHR:
		devType = charDevice
	case unix.S_IFIFO:
		devType = fifoDevice
	default:
		return nil, fmt.Errorf("not a device node")
	}

	// only get the permission bits for the device. ignore the type bits.
	fileMode := os.FileMode(mode &^ unix.S_IFMT)

	deviceInfo := &config.DeviceInfo{
		ContainerPath: path,
		DevType:       devType,
		Major:         int64(major),
		Minor:         int64(minor),
		UID:           stat.Uid,
		GID:           stat.Gid,
		FileMode:      fileMode,
	}

	return deviceInfo, nil
}
