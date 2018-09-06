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

	//VhostUserFS represents a virtio-fs vhostuser device type
	VhostUserFS = "vhost-user-fs-pci"
)

const (
	// VirtioMmio means use virtio-mmio for mmio based drives
	VirtioMmio = "virtio-mmio"

	// VirtioBlock means use virtio-blk for hotplugging drives
	VirtioBlock = "virtio-blk"

	// VirtioSCSI means use virtio-scsi for hotplugging drives
	VirtioSCSI = "virtio-scsi"

	// Nvdimm means use nvdimm for hotplugging drives
	Nvdimm = "nvdimm"
)

const (
	// Virtio9P means use virtio-9p for the shared file system
	Virtio9P = "virtio-9p"

	// VirtioFS means use virtio-fs for the shared file system
	VirtioFS = "virtio-fs"
)

// Defining these as a variable instead of a const, to allow
// overriding this in the tests.

// SysDevPrefix is static string of /sys/dev
var SysDevPrefix = "/sys/dev"

// SysIOMMUPath is static string of /sys/kernel/iommu_groups
var SysIOMMUPath = "/sys/kernel/iommu_groups"

// DeviceInfo is an embedded type that contains device data common to all types of devices.
type DeviceInfo struct {
	// Hostpath is device path on host
	HostPath string

	// ContainerPath is device path inside container
	ContainerPath string `json:"-"`

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

	// ID for the device that is passed to the hypervisor.
	ID string

	// DriverOptions is specific options for each device driver
	// for example, for BlockDevice, we can set DriverOptions["blockDriver"]="virtio-blk"
	DriverOptions map[string]string
}

// BlockDrive represents a block storage drive which may be used in case the storage
// driver has an underlying block storage device.
type BlockDrive struct {
	// File is the path to the disk-image/device which will be used with this drive
	File string

	// Format of the drive
	Format string

	// ID is used to identify this drive in the hypervisor options.
	ID string

	// Index assigned to the drive. In case of virtio-scsi, this is used as SCSI LUN index
	Index int

	// MmioAddr is used to identify the slot at which the drive is attached (order?).
	MmioAddr string

	// PCIAddr is the PCI address used to identify the slot at which the drive is attached.
	PCIAddr string

	// SCSI Address of the block device, in case the device is attached using SCSI driver
	// SCSI address is in the format SCSI-Id:LUN
	SCSIAddr string

	// NvdimmID is the nvdimm id inside the VM
	NvdimmID string

	// VirtPath at which the device appears inside the VM, outside of the container mount namespace
	VirtPath string
}

// VFIODeviceType indicates VFIO device type
type VFIODeviceType uint32

const (
	// VFIODeviceErrorType is the error type of VFIO device
	VFIODeviceErrorType VFIODeviceType = iota

	// VFIODeviceNormalType is a normal VFIO device type
	VFIODeviceNormalType

	// VFIODeviceMediatedType is a VFIO mediated device type
	VFIODeviceMediatedType
)

// VFIODev represents a VFIO drive used for hotplugging
type VFIODev struct {
	// ID is used to identify this drive in the hypervisor options.
	ID string

	// Type of VFIO device
	Type VFIODeviceType

	// BDF (Bus:Device.Function) of the PCI address
	BDF string

	// sysfsdev of VFIO mediated device
	SysfsDev string
}

// RNGDev represents a random number generator device
type RNGDev struct {
	// ID is used to identify the device in the hypervisor options.
	ID string
	// Filename is the file to use as entropy source.
	Filename string
}

// VhostUserDeviceAttrs represents data shared by most vhost-user devices
type VhostUserDeviceAttrs struct {
	DevID      string
	SocketPath string
	Type       DeviceType

	// MacAddress is only meaningful for vhost user net device
	MacAddress string

	// These are only meaningful for vhost user fs devices
	Tag       string
	CacheSize uint32
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
