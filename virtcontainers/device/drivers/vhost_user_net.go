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

// VhostUserNetDevice is a network vhost-user based device
type VhostUserNetDevice struct {
	config.VhostUserDeviceAttrs
	MacAddress string
}

// Attrs returns the VhostUserDeviceAttrs associated with the vhost-user device
func (vhostUserNetDevice *VhostUserNetDevice) Attrs() *config.VhostUserDeviceAttrs {
	return &vhostUserNetDevice.VhostUserDeviceAttrs
}

// Type returns the type associated with the vhost-user device
func (vhostUserNetDevice *VhostUserNetDevice) Type() config.DeviceType {
	return config.VhostUserNet
}

//
// VhostUserNetDevice's implementation of the device interface:
//

// Attach is standard interface of api.Device, it's used to add device to some
// DeviceReceiver
func (vhostUserNetDevice *VhostUserNetDevice) Attach(devReceiver api.DeviceReceiver) (err error) {
	return vhostUserAttach(vhostUserNetDevice, devReceiver)
}

// Detach is standard interface of api.Device, it's used to remove device from some
// DeviceReceiver
func (vhostUserNetDevice *VhostUserNetDevice) Detach(devReceiver api.DeviceReceiver) error {
	return nil
}

// DeviceType is standard interface of api.Device, it returns device type
func (vhostUserNetDevice *VhostUserNetDevice) DeviceType() config.DeviceType {
	return vhostUserNetDevice.DevType
}
