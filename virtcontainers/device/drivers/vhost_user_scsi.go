// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
)

// VhostUserSCSIDevice is a SCSI vhost-user based device
type VhostUserSCSIDevice struct {
	config.VhostUserDeviceAttrs
}

// Attrs returns the VhostUserDeviceAttrs associated with the vhost-user device
func (vhostUserSCSIDevice *VhostUserSCSIDevice) Attrs() *config.VhostUserDeviceAttrs {
	return &vhostUserSCSIDevice.VhostUserDeviceAttrs
}

// Type returns the type associated with the vhost-user device
func (vhostUserSCSIDevice *VhostUserSCSIDevice) Type() config.DeviceType {
	return config.VhostUserSCSI
}

//
// VhostUserSCSIDevice's implementation of the device interface:
//

// Attach is standard interface of api.Device, it's used to add device to some
// DeviceReceiver
func (vhostUserSCSIDevice *VhostUserSCSIDevice) Attach(devReceiver api.DeviceReceiver) (err error) {
	return vhostUserAttach(vhostUserSCSIDevice, devReceiver)
}

// Detach is standard interface of api.Device, it's used to remove device from some
// DeviceReceiver
func (vhostUserSCSIDevice *VhostUserSCSIDevice) Detach(devReceiver api.DeviceReceiver) error {
	return nil
}

// DeviceType is standard interface of api.Device, it returns device type
func (vhostUserSCSIDevice *VhostUserSCSIDevice) DeviceType() config.DeviceType {
	return vhostUserSCSIDevice.DevType
}
