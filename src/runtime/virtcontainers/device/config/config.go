// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package config

import (
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/go-ini/ini"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"
	"golang.org/x/sys/unix"
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

	// VirtioBlockCCW means use virtio-blk for hotplugging drives
	VirtioBlockCCW = "virtio-blk-ccw"

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

const (
	// The OCI spec requires the major-minor number to be provided for a
	// device. We have chosen the below major numbers to represent
	// vhost-user devices.
	VhostUserBlkMajor  = 241
	VhostUserSCSIMajor = 242
)

// Defining these as a variable instead of a const, to allow
// overriding this in the tests.

// SysDevPrefix is static string of /sys/dev
var SysDevPrefix = "/sys/dev"

// SysIOMMUPath is static string of /sys/kernel/iommu_groups
var SysIOMMUPath = "/sys/kernel/iommu_groups"

// SysBusPciDevicesPath is static string of /sys/bus/pci/devices
var SysBusPciDevicesPath = "/sys/bus/pci/devices"

var getSysDevPath = getSysDevPathImpl

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

	// Pmem enabled persistent memory. Use HostPath as backing file
	// for a nvdimm device in the guest.
	Pmem bool

	// If applicable, should this device be considered RO
	ReadOnly bool

	// ColdPlug specifies whether the device must be cold plugged (true)
	// or hot plugged (false).
	ColdPlug bool

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

	// PCIPath is the PCI path used to identify the slot at which the drive is attached.
	PCIPath vcTypes.PciPath

	// SCSI Address of the block device, in case the device is attached using SCSI driver
	// SCSI address is in the format SCSI-Id:LUN
	SCSIAddr string

	// NvdimmID is the nvdimm id inside the VM
	NvdimmID string

	// VirtPath at which the device appears inside the VM, outside of the container mount namespace
	VirtPath string

	// DevNo identifies the css bus id for virtio-blk-ccw
	DevNo string

	// ShareRW enables multiple qemu instances to share the File
	ShareRW bool

	// ReadOnly sets the device file readonly
	ReadOnly bool

	// Pmem enables persistent memory. Use File as backing file
	// for a nvdimm device in the guest
	Pmem bool
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
	// IsPCIe specifies device is PCIe or PCI
	IsPCIe bool

	// Type of VFIO device
	Type VFIODeviceType

	// ID is used to identify this drive in the hypervisor options.
	ID string

	// BDF (Bus:Device.Function) of the PCI address
	BDF string

	// sysfsdev of VFIO mediated device
	SysfsDev string

	// VendorID specifies vendor id
	VendorID string

	// DeviceID specifies device id
	DeviceID string

	// PCI Class Code
	Class string

	// Bus of VFIO PCIe device
	Bus string
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
	Cache     string

	// PCIPath is the PCI path used to identify the slot at which
	// the drive is attached.  It is only meaningful for vhost
	// user block devices
	PCIPath vcTypes.PciPath

	// Block index of the device if assigned
	Index int
}

// GetHostPathFunc is function pointer used to mock GetHostPath in tests.
var GetHostPathFunc = GetHostPath

// GetVhostUserNodeStatFunc is function pointer used to mock GetVhostUserNodeStat
// in tests. Through this functon, user can get device type information.
var GetVhostUserNodeStatFunc = GetVhostUserNodeStat

// GetHostPath is used to fetch the host path for the device.
// The path passed in the spec refers to the path that should appear inside the container.
// We need to find the actual device path on the host based on the major-minor numbers of the device.
func GetHostPath(devInfo DeviceInfo, vhostUserStoreEnabled bool, vhostUserStorePath string) (string, error) {
	if devInfo.ContainerPath == "" {
		return "", fmt.Errorf("Empty path provided for device")
	}

	// Filter out vhost-user storage devices by device Major numbers.
	if vhostUserStoreEnabled && devInfo.DevType == "b" &&
		(devInfo.Major == VhostUserSCSIMajor || devInfo.Major == VhostUserBlkMajor) {
		return getVhostUserHostPath(devInfo, vhostUserStorePath)
	}

	ueventPath := filepath.Join(getSysDevPath(devInfo), "uevent")
	if _, err := os.Stat(ueventPath); err != nil {
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

	content, err := ini.Load(ueventPath)
	if err != nil {
		return "", err
	}

	devName, err := content.Section("").GetKey("DEVNAME")
	if err != nil {
		return "", err
	}

	return filepath.Join("/dev", devName.String()), nil
}

// getBackingFile is used to fetch the backing file for the device.
func getBackingFile(devInfo DeviceInfo) (string, error) {
	backingFilePath := filepath.Join(getSysDevPath(devInfo), "loop", "backing_file")
	data, err := ioutil.ReadFile(backingFilePath)
	if err != nil {
		return "", err
	}

	return strings.TrimSpace(string(data)), nil
}

func getSysDevPathImpl(devInfo DeviceInfo) string {
	var pathComp string

	switch devInfo.DevType {
	case "c", "u":
		pathComp = "char"
	case "b":
		pathComp = "block"
	default:
		// Unsupported device types. Return nil error to ignore devices
		// that cannot be handled currently.
		return ""
	}

	format := strconv.FormatInt(devInfo.Major, 10) + ":" + strconv.FormatInt(devInfo.Minor, 10)
	return filepath.Join(SysDevPrefix, pathComp, format)
}

// getVhostUserHostPath is used to fetch host path for the vhost-user device.
// For vhost-user block device like vhost-user-blk or vhost-user-scsi, its
// socket should be under directory "<vhostUserStorePath>/block/sockets/";
// its corresponding device node should be under directory
// "<vhostUserStorePath>/block/devices/"
func getVhostUserHostPath(devInfo DeviceInfo, vhostUserStorePath string) (string, error) {
	vhostUserDevNodePath := filepath.Join(vhostUserStorePath, "/block/devices/")
	vhostUserSockPath := filepath.Join(vhostUserStorePath, "/block/sockets/")

	sockFileName, err := getVhostUserDevName(vhostUserDevNodePath,
		uint32(devInfo.Major), uint32(devInfo.Minor))
	if err != nil {
		return "", err
	}

	// Locate socket path of vhost-user device
	sockFilePath := filepath.Join(vhostUserSockPath, sockFileName)
	if _, err = os.Stat(sockFilePath); os.IsNotExist(err) {
		return "", err
	}

	return sockFilePath, nil
}

func GetVhostUserNodeStat(devNodePath string, devNodeStat *unix.Stat_t) (err error) {
	return unix.Stat(devNodePath, devNodeStat)
}

// Filter out name of the device node whose device type is Major:Minor from directory
func getVhostUserDevName(dirname string, majorNum, minorNum uint32) (string, error) {
	files, err := ioutil.ReadDir(dirname)
	if err != nil {
		return "", err
	}

	for _, file := range files {
		var devStat unix.Stat_t

		devFilePath := filepath.Join(dirname, file.Name())
		err = GetVhostUserNodeStatFunc(devFilePath, &devStat)
		if err != nil {
			return "", err
		}

		devMajor := unix.Major(devStat.Rdev)
		devMinor := unix.Minor(devStat.Rdev)
		if devMajor == majorNum && devMinor == minorNum {
			return file.Name(), nil
		}
	}

	return "", fmt.Errorf("Required device node (%d:%d) doesn't exist under directory %s",
		majorNum, minorNum, dirname)
}
