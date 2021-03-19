// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018-2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"context"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/sirupsen/logrus"
)

// VhostUserBlkDevice is a block vhost-user based device
type VhostUserBlkDevice struct {
	*GenericDevice
	VhostUserDeviceAttrs *config.VhostUserDeviceAttrs
}

// NewVhostUserBlkDevice creates a new vhost-user block device based on DeviceInfo
func NewVhostUserBlkDevice(devInfo *config.DeviceInfo) *VhostUserBlkDevice {
	return &VhostUserBlkDevice{
		GenericDevice: &GenericDevice{
			ID:         devInfo.ID,
			DeviceInfo: devInfo,
		},
	}
}

//
// VhostUserBlkDevice's implementation of the device interface:
//

// Attach is standard interface of api.Device, it's used to add device to some
// DeviceReceiver
func (device *VhostUserBlkDevice) Attach(ctx context.Context, devReceiver api.DeviceReceiver) (err error) {
	skip, err := device.bumpAttachCount(true)
	if err != nil {
		return err
	}
	if skip {
		return nil
	}

	// From the explanation of function attach in block.go, block index of
	// a general block device is utilized for some situation.
	// Since vhost-user-blk uses "vd" prefix in Linux kernel, not "sd",
	// sandbox block index should be updated only if sandbox default block
	// driver is "virtio-blk"/"virtio-blk-ccw"/"virtio-mmio" which uses
	// "vd" prefix in Linux kernel.
	index := -1
	updateBlockIndex := isVirtioBlkBlockDriver(device.DeviceInfo.DriverOptions)
	if updateBlockIndex {
		index, err = devReceiver.GetAndSetSandboxBlockIndex()
	}

	defer func() {
		if err != nil {
			if updateBlockIndex {
				devReceiver.UnsetSandboxBlockIndex(index)
			}
			device.bumpAttachCount(false)
		}
	}()

	if err != nil {
		return err
	}

	vAttrs := &config.VhostUserDeviceAttrs{
		DevID:      utils.MakeNameID("blk", device.DeviceInfo.ID, maxDevIDSize),
		SocketPath: device.DeviceInfo.HostPath,
		Type:       config.VhostUserBlk,
		Index:      index,
	}

	deviceLogger().WithFields(logrus.Fields{
		"device":     device.DeviceInfo.HostPath,
		"SocketPath": vAttrs.SocketPath,
		"Type":       config.VhostUserBlk,
		"Index":      index,
	}).Info("Attaching device")

	device.VhostUserDeviceAttrs = vAttrs
	if err = devReceiver.HotplugAddDevice(ctx, device, config.VhostUserBlk); err != nil {
		return err
	}

	return nil
}

func isVirtioBlkBlockDriver(customOptions map[string]string) bool {
	var blockDriverOption string

	if customOptions == nil {
		// User has not chosen a specific block device type
		// Default to SCSI
		blockDriverOption = "virtio-scsi"
	} else {
		blockDriverOption = customOptions["block-driver"]
	}

	if blockDriverOption == "virtio-blk" ||
		blockDriverOption == "virtio-blk-ccw" ||
		blockDriverOption == "virtio-mmio" {
		return true
	}

	return false
}

// Detach is standard interface of api.Device, it's used to remove device from some
// DeviceReceiver
func (device *VhostUserBlkDevice) Detach(ctx context.Context, devReceiver api.DeviceReceiver) error {
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
			updateBlockIndex := isVirtioBlkBlockDriver(device.DeviceInfo.DriverOptions)
			if updateBlockIndex {
				devReceiver.UnsetSandboxBlockIndex(device.VhostUserDeviceAttrs.Index)
			}
		}
	}()

	deviceLogger().WithField("device", device.DeviceInfo.HostPath).Info("Unplugging vhost-user-blk device")

	if err = devReceiver.HotplugRemoveDevice(ctx, device, config.VhostUserBlk); err != nil {
		deviceLogger().WithError(err).Error("Failed to unplug vhost-user-blk device")
		return err
	}
	return nil
}

// DeviceType is standard interface of api.Device, it returns device type
func (device *VhostUserBlkDevice) DeviceType() config.DeviceType {
	return config.VhostUserBlk
}

// GetDeviceInfo returns device information used for creating
func (device *VhostUserBlkDevice) GetDeviceInfo() interface{} {
	return device.VhostUserDeviceAttrs
}

// Save converts Device to DeviceState
func (device *VhostUserBlkDevice) Save() persistapi.DeviceState {
	ds := device.GenericDevice.Save()
	ds.Type = string(device.DeviceType())

	vAttr := device.VhostUserDeviceAttrs
	if vAttr != nil {
		ds.VhostUserDev = &persistapi.VhostUserDeviceAttrs{
			DevID:      vAttr.DevID,
			SocketPath: vAttr.SocketPath,
			Type:       string(vAttr.Type),
			PCIPath:    vAttr.PCIPath,
			Index:      vAttr.Index,
		}
	}
	return ds
}

// Load loads DeviceState and converts it to specific device
func (device *VhostUserBlkDevice) Load(ds persistapi.DeviceState) {
	device.GenericDevice = &GenericDevice{}
	device.GenericDevice.Load(ds)

	dev := ds.VhostUserDev
	if dev == nil {
		return
	}

	device.VhostUserDeviceAttrs = &config.VhostUserDeviceAttrs{
		DevID:      dev.DevID,
		SocketPath: dev.SocketPath,
		Type:       config.DeviceType(dev.Type),
		PCIPath:    dev.PCIPath,
		Index:      dev.Index,
	}
}

// It should implement GetAttachCount() and DeviceID() as api.Device implementation
// here it shares function from *GenericDevice so we don't need duplicate codes
