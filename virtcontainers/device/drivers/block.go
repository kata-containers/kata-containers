// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"encoding/hex"
	"path/filepath"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/utils"
)

const maxDevIDSize = 31

// Drive represents a block storage drive which may be used in case the storage
// driver has an underlying block storage device.
type Drive struct {

	// Path to the disk-image/device which will be used with this drive
	File string

	// Format of the drive
	Format string

	// ID is used to identify this drive in the hypervisor options.
	ID string

	// Index assigned to the drive. In case of virtio-scsi, this is used as SCSI LUN index
	Index int

	// PCIAddr is the PCI address used to identify the slot at which the drive is attached.
	PCIAddr string
}

// BlockDevice refers to a block storage device implementation.
type BlockDevice struct {
	DevType    config.DeviceType
	DeviceInfo config.DeviceInfo

	// SCSI Address of the block device, in case the device is attached using SCSI driver
	// SCSI address is in the format SCSI-Id:LUN
	SCSIAddr string

	// Path at which the device appears inside the VM, outside of the container mount namespace
	VirtPath string

	// PCI Slot of the block device
	PCIAddr string

	BlockDrive *Drive
}

// NewBlockDevice creates a new block device based on DeviceInfo
func NewBlockDevice(devInfo config.DeviceInfo) *BlockDevice {
	return &BlockDevice{
		DevType:    config.DeviceBlock,
		DeviceInfo: devInfo,
	}
}

// Attach is standard interface of api.Device, it's used to add device to some
// DeviceReceiver
func (device *BlockDevice) Attach(devReceiver api.DeviceReceiver) (err error) {
	randBytes, err := utils.GenerateRandomBytes(8)
	if err != nil {
		return err
	}

	device.DeviceInfo.ID = hex.EncodeToString(randBytes)

	// Increment the block index for the sandbox. This is used to determine the name
	// for the block device in the case where the block device is used as container
	// rootfs and the predicted block device name needs to be provided to the agent.
	index, err := devReceiver.GetAndSetSandboxBlockIndex()

	defer func() {
		if err != nil {
			devReceiver.DecrementSandboxBlockIndex()
		}
	}()

	if err != nil {
		return err
	}

	drive := Drive{
		File:   device.DeviceInfo.HostPath,
		Format: "raw",
		ID:     utils.MakeNameID("drive", device.DeviceInfo.ID, maxDevIDSize),
		Index:  index,
	}

	deviceLogger().WithField("device", device.DeviceInfo.HostPath).Info("Attaching block device")
	device.BlockDrive = &drive
	if err = devReceiver.HotplugAddDevice(device, config.DeviceBlock); err != nil {
		return err
	}

	device.DeviceInfo.Hotplugged = true

	driveName, err := utils.GetVirtDriveName(index)
	if err != nil {
		return err
	}

	customOptions := device.DeviceInfo.DriverOptions
	if customOptions != nil && customOptions["block-driver"] == "virtio-blk" {
		device.VirtPath = filepath.Join("/dev", driveName)
		device.PCIAddr = drive.PCIAddr
	} else {
		scsiAddr, err := utils.GetSCSIAddress(index)
		if err != nil {
			return err
		}

		device.SCSIAddr = scsiAddr
	}

	return nil
}

// Detach is standard interface of api.Device, it's used to remove device from some
// DeviceReceiver
func (device *BlockDevice) Detach(devReceiver api.DeviceReceiver) error {
	if device.DeviceInfo.Hotplugged {
		deviceLogger().WithField("device", device.DeviceInfo.HostPath).Info("Unplugging block device")

		if err := devReceiver.HotplugRemoveDevice(device, config.DeviceBlock); err != nil {
			deviceLogger().WithError(err).Error("Failed to unplug block device")
			return err
		}

	}
	return nil
}

// DeviceType is standard interface of api.Device, it returns device type
func (device *BlockDevice) DeviceType() config.DeviceType {
	return device.DevType
}
