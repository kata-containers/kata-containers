// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018-2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"context"
	"path/filepath"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

const maxDevIDSize = 31

// BlockDevice refers to a block storage device implementation.
type BlockDevice struct {
	*GenericDevice
	BlockDrive *config.BlockDrive
}

// NewBlockDevice creates a new block device based on DeviceInfo
func NewBlockDevice(devInfo *config.DeviceInfo) *BlockDevice {
	return &BlockDevice{
		GenericDevice: &GenericDevice{
			ID:         devInfo.ID,
			DeviceInfo: devInfo,
		},
	}
}

// Attach is standard interface of api.Device, it's used to add device to some
// DeviceReceiver
func (device *BlockDevice) Attach(ctx context.Context, devReceiver api.DeviceReceiver) (err error) {
	skip, err := device.bumpAttachCount(true)
	if err != nil {
		return err
	}
	if skip {
		return nil
	}

	// Increment the block index for the sandbox. This is used to determine the name
	// for the block device in the case where the block device is used as container
	// rootfs and the predicted block device name needs to be provided to the agent.
	index, err := devReceiver.GetAndSetSandboxBlockIndex()

	defer func() {
		if err != nil {
			devReceiver.UnsetSandboxBlockIndex(index)
			device.bumpAttachCount(false)
		}
	}()

	if err != nil {
		return err
	}

	drive := &config.BlockDrive{
		File:     device.DeviceInfo.HostPath,
		Format:   "raw",
		ID:       utils.MakeNameID("drive", device.DeviceInfo.ID, maxDevIDSize),
		Index:    index,
		Pmem:     device.DeviceInfo.Pmem,
		ReadOnly: device.DeviceInfo.ReadOnly,
	}

	if fs, ok := device.DeviceInfo.DriverOptions[config.FsTypeOpt]; ok {
		drive.Format = fs
	}

	customOptions := device.DeviceInfo.DriverOptions
	if customOptions == nil ||
		customOptions[config.BlockDriverOpt] == config.VirtioSCSI {
		// User has not chosen a specific block device type
		// Default to SCSI
		scsiAddr, err := utils.GetSCSIAddress(index)
		if err != nil {
			return err
		}

		drive.SCSIAddr = scsiAddr
	} else if customOptions[config.BlockDriverOpt] != config.Nvdimm {
		var globalIdx int

		switch customOptions[config.BlockDriverOpt] {
		case config.VirtioBlock:
			globalIdx = index
		case config.VirtioBlockCCW:
			globalIdx = index
		case config.VirtioMmio:
			//With firecracker the rootfs for the VM itself
			//sits at /dev/vda and consumes the first index.
			//Longer term block based VM rootfs should be added
			//as a regular block device which eliminates the
			//offset.
			//https://github.com/kata-containers/runtime/issues/1061
			globalIdx = index + 1
		}

		driveName, err := utils.GetVirtDriveName(globalIdx)
		if err != nil {
			return err
		}

		drive.VirtPath = filepath.Join("/dev", driveName)
	}

	deviceLogger().WithField("device", device.DeviceInfo.HostPath).WithField("VirtPath", drive.VirtPath).Infof("Attaching %s device", customOptions[config.BlockDriverOpt])
	device.BlockDrive = drive
	if err = devReceiver.HotplugAddDevice(ctx, device, config.DeviceBlock); err != nil {
		return err
	}

	return nil
}

// Detach is standard interface of api.Device, it's used to remove device from some
// DeviceReceiver
func (device *BlockDevice) Detach(ctx context.Context, devReceiver api.DeviceReceiver) error {
	skip, err := device.bumpAttachCount(false)
	if err != nil {
		return err
	}
	if skip {
		return nil
	}

	defer func() {
		if err != nil {
			device.bumpAttachCount(true)
		} else {
			devReceiver.UnsetSandboxBlockIndex(device.BlockDrive.Index)
		}
	}()

	deviceLogger().WithField("device", device.DeviceInfo.HostPath).Info("Unplugging block device")

	if err = devReceiver.HotplugRemoveDevice(ctx, device, config.DeviceBlock); err != nil {
		deviceLogger().WithError(err).Error("Failed to unplug block device")
		return err
	}
	return nil
}

// DeviceType is standard interface of api.Device, it returns device type
func (device *BlockDevice) DeviceType() config.DeviceType {
	return config.DeviceBlock
}

// GetDeviceInfo returns device information used for creating
func (device *BlockDevice) GetDeviceInfo() interface{} {
	return device.BlockDrive
}

// Save converts Device to DeviceState
func (device *BlockDevice) Save() config.DeviceState {
	ds := device.GenericDevice.Save()
	ds.Type = string(device.DeviceType())

	ds.BlockDrive = device.BlockDrive

	return ds
}

// Load loads DeviceState and converts it to specific device
func (device *BlockDevice) Load(ds config.DeviceState) {
	device.GenericDevice = &GenericDevice{}
	device.GenericDevice.Load(ds)

	device.BlockDrive = ds.BlockDrive
}

// It should implement GetAttachCount() and DeviceID() as api.Device implementation
// here it shares function from *GenericDevice so we don't need duplicate codes
