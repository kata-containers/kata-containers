// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package config

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"

	"github.com/go-ini/ini"
)

// DeviceType indicates device type
type DeviceType string

const (
	// DeviceVFIO is the VFIO device type
	DeviceVFIO DeviceType = "vfio"

	// DeviceBlock is the block device type
	DeviceBlock DeviceType = "block"

	// DeviceGeneric is a generic device type
	DeviceGeneric DeviceType = "generic"

	//VhostUserSCSI - SCSI based vhost-user type
	VhostUserSCSI = "vhost-user-scsi-pci"

	//VhostUserNet - net based vhost-user type
	VhostUserNet = "virtio-net-pci"

	//VhostUserBlk represents a block vhostuser device type
	VhostUserBlk = "vhost-user-blk-pci"
)

// Defining these as a variable instead of a const, to allow
// overriding this in the tests.

// SysDevPrefix is static string of /sys/dev
var SysDevPrefix = "/sys/dev"

// SysIOMMUPath is static string of /sys/kernel/iommu_groups
var SysIOMMUPath = "/sys/kernel/iommu_groups"

// DeviceInfo is an embedded type that contains device data common to all types of devices.
type DeviceInfo struct {
	// Device path on host
	HostPath string

	// Device path inside the container
	ContainerPath string

	// Type of device: c, b, u or p
	// c , u - character(unbuffered)
	// p - FIFO
	// b - block(buffered) special file
	// More info in mknod(1).
	DevType string

	// Major, minor numbers for device.
	Major int64
	Minor int64

	// FileMode permission bits for the device.
	FileMode os.FileMode

	// id of the device owner.
	UID uint32

	// id of the device group.
	GID uint32

	// Hotplugged is used to store device state indicating if the
	// device was hotplugged.
	Hotplugged bool

	// ID for the device that is passed to the hypervisor.
	ID string

	// DriverOptions is specific options for each device driver
	// for example, for BlockDevice, we can set DriverOptions["blockDriver"]="virtio-blk"
	DriverOptions map[string]string
}

// VhostUserDeviceAttrs represents data shared by most vhost-user devices
type VhostUserDeviceAttrs struct {
	DevType    DeviceType
	DeviceInfo DeviceInfo
	SocketPath string
	ID         string
}

// GetHostPathFunc is function pointer used to mock GetHostPath in tests.
var GetHostPathFunc = GetHostPath

// GetHostPath is used to fetch the host path for the device.
// The path passed in the spec refers to the path that should appear inside the container.
// We need to find the actual device path on the host based on the major-minor numbers of the device.
func GetHostPath(devInfo DeviceInfo) (string, error) {
	if devInfo.ContainerPath == "" {
		return "", fmt.Errorf("Empty path provided for device")
	}

	var pathComp string

	switch devInfo.DevType {
	case "c", "u":
		pathComp = "char"
	case "b":
		pathComp = "block"
	default:
		// Unsupported device types. Return nil error to ignore devices
		// that cannot be handled currently.
		return "", nil
	}

	format := strconv.FormatInt(devInfo.Major, 10) + ":" + strconv.FormatInt(devInfo.Minor, 10)
	sysDevPath := filepath.Join(SysDevPrefix, pathComp, format, "uevent")

	if _, err := os.Stat(sysDevPath); err != nil {
		// Some devices(eg. /dev/fuse, /dev/cuse) do not always implement sysfs interface under /sys/dev
		// These devices are passed by default by docker.
		//
		// Simply return the path passed in the device configuration, this does mean that no device renames are
		// supported for these devices.

		if os.IsNotExist(err) {
			return devInfo.ContainerPath, nil
		}

		return "", err
	}

	content, err := ini.Load(sysDevPath)
	if err != nil {
		return "", err
	}

	devName, err := content.Section("").GetKey("DEVNAME")
	if err != nil {
		return "", err
	}

	return filepath.Join("/dev", devName.String()), nil
}
