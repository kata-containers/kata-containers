// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018-2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"context"
	"strconv"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
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
		DevID:         utils.MakeNameID("blk", device.DeviceInfo.ID, maxDevIDSize),
		SocketPath:    device.DeviceInfo.HostPath,
		Type:          config.VhostUserBlk,
		Index:         index,
		ReconnectTime: vhostUserReconnect(device.DeviceInfo.DriverOptions),
	}

	deviceLogger().WithFields(logrus.Fields{
		"device":        device.DeviceInfo.HostPath,
		"SocketPath":    vAttrs.SocketPath,
		"Type":          config.VhostUserBlk,
		"Index":         index,
		"ReconnectTime": vAttrs.ReconnectTime,
	}).Info("Attaching device")

	device.VhostUserDeviceAttrs = vAttrs
	if err = devReceiver.HotplugAddDevice(ctx, device, config.VhostUserBlk); err != nil {
		return err
	}

	return nil
}

func vhostUserReconnect(customOptions map[string]string) uint32 {
	var vhostUserReconnectTimeout uint32

	if customOptions == nil {
		vhostUserReconnectTimeout = config.DefaultVhostUserReconnectTimeOut
	} else {
		reconnectTimeoutStr := customOptions[config.VhostUserReconnectTimeOutOpt]
		if reconnectTimeout, err := strconv.Atoi(reconnectTimeoutStr); err != nil {
			vhostUserReconnectTimeout = config.DefaultVhostUserReconnectTimeOut
			deviceLogger().WithField("reconnect", reconnectTimeoutStr).WithError(err).Warn("Failed to get reconnect timeout for  vhost-user-blk device")
		} else {
			vhostUserReconnectTimeout = uint32(reconnectTimeout)
		}
	}

	return vhostUserReconnectTimeout
}

func isVirtioBlkBlockDriver(customOptions map[string]string) bool {
	var blockDriverOption string

	if customOptions == nil {
		// User has not chosen a specific block device type
		// Default to SCSI
		blockDriverOption = config.VirtioSCSI
	} else {
		blockDriverOption = customOptions[config.BlockDriverOpt]
	}

	if blockDriverOption == config.VirtioBlock ||
		blockDriverOption == config.VirtioBlockCCW ||
		blockDriverOption == config.VirtioMmio {
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
func (device *VhostUserBlkDevice) Save() config.DeviceState {
	ds := device.GenericDevice.Save()
	ds.Type = string(device.DeviceType())
	ds.VhostUserDev = device.VhostUserDeviceAttrs

	return ds
}

// Load loads DeviceState and converts it to specific device
func (device *VhostUserBlkDevice) Load(ds config.DeviceState) {
	device.GenericDevice = &GenericDevice{}
	device.GenericDevice.Load(ds)
	device.VhostUserDeviceAttrs = ds.VhostUserDev
}

// It should implement GetAttachCount() and DeviceID() as api.Device implementation
// here it shares function from *GenericDevice so we don't need duplicate codes
