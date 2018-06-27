// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"path/filepath"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/utils"
)

const maxDevIDSize = 31

// BlockDevice refers to a block storage device implementation.
type BlockDevice struct {
	ID         string
	DeviceInfo *config.DeviceInfo
	BlockDrive *config.BlockDrive
}

// NewBlockDevice creates a new block device based on DeviceInfo
func NewBlockDevice(devInfo *config.DeviceInfo) *BlockDevice {
	return &BlockDevice{
		ID:         devInfo.ID,
		DeviceInfo: devInfo,
	}
}

// Attach is standard interface of api.Device, it's used to add device to some
// DeviceReceiver
func (device *BlockDevice) Attach(devReceiver api.DeviceReceiver) (err error) {
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

	drive := &config.BlockDrive{
		File:   device.DeviceInfo.HostPath,
		Format: "raw",
		ID:     utils.MakeNameID("drive", device.DeviceInfo.ID, maxDevIDSize),
		Index:  index,
	}

	driveName, err := utils.GetVirtDriveName(index)
	if err != nil {
		return err
	}

	customOptions := device.DeviceInfo.DriverOptions
	if customOptions != nil && customOptions["block-driver"] == "virtio-blk" {
		drive.VirtPath = filepath.Join("/dev", driveName)
	} else {
		scsiAddr, err := utils.GetSCSIAddress(index)
		if err != nil {
			return err
		}

		drive.SCSIAddr = scsiAddr
	}

	deviceLogger().WithField("device", device.DeviceInfo.HostPath).Info("Attaching block device")
	device.BlockDrive = drive
	if err = devReceiver.HotplugAddDevice(device, config.DeviceBlock); err != nil {
		return err
	}

	device.DeviceInfo.Hotplugged = true

	return nil
}

// Detach is standard interface of api.Device, it's used to remove device from some
// DeviceReceiver
func (device *BlockDevice) Detach(devReceiver api.DeviceReceiver) error {
	deviceLogger().WithField("device", device.DeviceInfo.HostPath).Info("Unplugging block device")

	if err := devReceiver.HotplugRemoveDevice(device, config.DeviceBlock); err != nil {
		deviceLogger().WithError(err).Error("Failed to unplug block device")
		return err
	}
	device.DeviceInfo.Hotplugged = false
	return nil
}

// IsAttached checks if the device is attached
func (device *BlockDevice) IsAttached() bool {
	return device.DeviceInfo.Hotplugged
}

// DeviceType is standard interface of api.Device, it returns device type
func (device *BlockDevice) DeviceType() config.DeviceType {
	return config.DeviceBlock
}

// DeviceID returns device ID
func (device *BlockDevice) DeviceID() string {
	return device.ID
}

// GetDeviceInfo returns device information that the device is created based on
func (device *BlockDevice) GetDeviceInfo() *config.DeviceInfo {
	return device.DeviceInfo
}

// GetDeviceDrive returns device information used for creating
func (device *BlockDevice) GetDeviceDrive() interface{} {
	return device.BlockDrive
}
