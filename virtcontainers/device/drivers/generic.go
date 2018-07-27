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

// GenericDevice refers to a device that is neither a VFIO device or block device.
type GenericDevice struct {
	DevType    config.DeviceType
	DeviceInfo config.DeviceInfo
}

// NewGenericDevice creates a new GenericDevice
func NewGenericDevice(devInfo config.DeviceInfo) *GenericDevice {
	return &GenericDevice{
		DevType:    config.DeviceGeneric,
		DeviceInfo: devInfo,
	}
}

// Attach is standard interface of api.Device
func (device *GenericDevice) Attach(devReceiver api.DeviceReceiver) error {
	return nil
}

// Detach is standard interface of api.Device
func (device *GenericDevice) Detach(devReceiver api.DeviceReceiver) error {
	return nil
}

// DeviceType is standard interface of api.Device, it returns device type
func (device *GenericDevice) DeviceType() config.DeviceType {
	return device.DevType
}
