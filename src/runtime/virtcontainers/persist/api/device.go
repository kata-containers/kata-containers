// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persistapi

import vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"

// ============= sandbox level resources =============

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

	// DevNo
	DevNo string

	// Pmem enabled persistent memory. Use File as backing file
	// for a nvdimm device in the guest.
	Pmem bool
}

// VFIODev represents a VFIO drive used for hotplugging
type VFIODev struct {
	// ID is used to identify this drive in the hypervisor options.
	ID string

	// Type of VFIO device
	Type uint32

	// BDF (Bus:Device.Function) of the PCI address
	BDF string

	// Sysfsdev of VFIO mediated device
	SysfsDev string
}

// VhostUserDeviceAttrs represents data shared by most vhost-user devices
type VhostUserDeviceAttrs struct {
	DevID      string
	SocketPath string
	Type       string

	// MacAddress is only meaningful for vhost user net device
	MacAddress string

	// PCIPath is the PCI path used to identify the slot at which the drive is attached.
	// It is only meaningful for vhost user block devices
	PCIPath vcTypes.PciPath

	// Block index of the device if assigned
	Index int
}

// DeviceState is sandbox level resource which represents host devices
// plugged to hypervisor, one Device can be shared among containers in POD
// Refs: virtcontainers/device/drivers/generic.go:GenericDevice
type DeviceState struct {
	ID string

	// Type is used to specify driver type
	// Refs: virtcontainers/device/config/config.go:DeviceType
	Type string

	RefCount    uint
	AttachCount uint

	// Type of device: c, b, u or p
	// c , u - character(unbuffered)
	// p - FIFO
	// b - block(buffered) special file
	// More info in mknod(1).
	DevType string

	// Major, minor numbers for device.
	Major int64
	Minor int64

	// ColdPlug specifies whether the device must be cold plugged (true)
	// or hot plugged (false).
	ColdPlug bool

	// DriverOptions is specific options for each device driver
	// for example, for BlockDevice, we can set DriverOptions["blockDriver"]="virtio-blk"
	DriverOptions map[string]string

	// ============ device driver specific data ===========
	// BlockDrive is specific for block device driver
	BlockDrive *BlockDrive `json:",omitempty"`

	// VFIODev is specific VFIO device driver
	VFIODevs []*VFIODev `json:",omitempty"`

	// VhostUserDeviceAttrs is specific for vhost-user device driver
	VhostUserDev *VhostUserDeviceAttrs `json:",omitempty"`
	// ============ end device driver specific data ===========
}
