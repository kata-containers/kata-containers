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

// VhostUserBlkDevice is a block vhost-user based device
type VhostUserBlkDevice struct {
	config.VhostUserDeviceAttrs
}

// Attrs returns the VhostUserDeviceAttrs associated with the vhost-user device
func (vhostUserBlkDevice *VhostUserBlkDevice) Attrs() *config.VhostUserDeviceAttrs {
	return &vhostUserBlkDevice.VhostUserDeviceAttrs
}

// Type returns the type associated with the vhost-user device
func (vhostUserBlkDevice *VhostUserBlkDevice) Type() config.DeviceType {
	return config.VhostUserBlk
}

//
// VhostUserBlkDevice's implementation of the device interface:
//

// Attach is standard interface of api.Device, it's used to add device to some
// DeviceReceiver
func (vhostUserBlkDevice *VhostUserBlkDevice) Attach(devReceiver api.DeviceReceiver) (err error) {
	return vhostUserAttach(vhostUserBlkDevice, devReceiver)
}

// Detach is standard interface of api.Device, it's used to remove device from some
// DeviceReceiver
func (vhostUserBlkDevice *VhostUserBlkDevice) Detach(devReceiver api.DeviceReceiver) error {
	return nil
}

// DeviceType is standard interface of api.Device, it returns device type
func (vhostUserBlkDevice *VhostUserBlkDevice) DeviceType() config.DeviceType {
	return vhostUserBlkDevice.DevType
}
