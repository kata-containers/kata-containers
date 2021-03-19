// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018-2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"context"
	"encoding/hex"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// VhostUserSCSIDevice is a SCSI vhost-user based device
type VhostUserSCSIDevice struct {
	*GenericDevice
	config.VhostUserDeviceAttrs
}

//
// VhostUserSCSIDevice's implementation of the device interface:
//

// Attach is standard interface of api.Device, it's used to add device to some
// DeviceReceiver
func (device *VhostUserSCSIDevice) Attach(ctx context.Context, devReceiver api.DeviceReceiver) (err error) {
	skip, err := device.bumpAttachCount(true)
	if err != nil {
		return err
	}
	if skip {
		return nil
	}

	defer func() {
		if err != nil {
			device.bumpAttachCount(false)
		}
	}()

	// generate a unique ID to be used for hypervisor commandline fields
	randBytes, err := utils.GenerateRandomBytes(8)
	if err != nil {
		return err
	}
	id := hex.EncodeToString(randBytes)

	device.DevID = id
	device.Type = device.DeviceType()

	return devReceiver.AppendDevice(ctx, device)
}

// Detach is standard interface of api.Device, it's used to remove device from some
// DeviceReceiver
func (device *VhostUserSCSIDevice) Detach(ctx context.Context, devReceiver api.DeviceReceiver) error {
	_, err := device.bumpAttachCount(false)
	return err
}

// DeviceType is standard interface of api.Device, it returns device type
func (device *VhostUserSCSIDevice) DeviceType() config.DeviceType {
	return config.VhostUserSCSI
}

// GetDeviceInfo returns device information used for creating
func (device *VhostUserSCSIDevice) GetDeviceInfo() interface{} {
	device.Type = device.DeviceType()
	return &device.VhostUserDeviceAttrs
}

// Save converts Device to DeviceState
func (device *VhostUserSCSIDevice) Save() persistapi.DeviceState {
	ds := device.GenericDevice.Save()
	ds.Type = string(device.DeviceType())
	ds.VhostUserDev = &persistapi.VhostUserDeviceAttrs{
		DevID:      device.DevID,
		SocketPath: device.SocketPath,
		Type:       string(device.Type),
		MacAddress: device.MacAddress,
	}
	return ds
}

// Load loads DeviceState and converts it to specific device
func (device *VhostUserSCSIDevice) Load(ds persistapi.DeviceState) {
	device.GenericDevice = &GenericDevice{}
	device.GenericDevice.Load(ds)

	dev := ds.VhostUserDev
	if dev == nil {
		return
	}

	device.VhostUserDeviceAttrs = config.VhostUserDeviceAttrs{
		DevID:      dev.DevID,
		SocketPath: dev.SocketPath,
		Type:       config.DeviceType(dev.Type),
		MacAddress: dev.MacAddress,
	}
}

// It should implement GetAttachCount() and DeviceID() as api.Device implementation
// here it shares function from *GenericDevice so we don't need duplicate codes
