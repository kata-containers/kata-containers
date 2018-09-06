// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"fmt"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
)

// GenericDevice refers to a device that is neither a VFIO device or block device.
type GenericDevice struct {
	ID         string
	DeviceInfo *config.DeviceInfo

	RefCount    uint
	AttachCount uint
}

// NewGenericDevice creates a new GenericDevice
func NewGenericDevice(devInfo *config.DeviceInfo) *GenericDevice {
	return &GenericDevice{
		ID:         devInfo.ID,
		DeviceInfo: devInfo,
	}
}

// Attach is standard interface of api.Device
func (device *GenericDevice) Attach(devReceiver api.DeviceReceiver) error {
	skip, err := device.bumpAttachCount(true)
	if err != nil {
		return err
	}
	if skip {
		return nil
	}
	device.AttachCount = 1
	return nil
}

// Detach is standard interface of api.Device
func (device *GenericDevice) Detach(devReceiver api.DeviceReceiver) error {
	skip, err := device.bumpAttachCount(false)
	if err != nil {
		return err
	}
	if skip {
		return nil
	}
	device.AttachCount = 0
	return nil
}

// DeviceType is standard interface of api.Device, it returns device type
func (device *GenericDevice) DeviceType() config.DeviceType {
	return config.DeviceGeneric
}

// GetDeviceInfo returns device information used for creating
func (device *GenericDevice) GetDeviceInfo() interface{} {
	return device.DeviceInfo
}

// GetAttachCount returns how many times the device has been attached
func (device *GenericDevice) GetAttachCount() uint {
	return device.AttachCount
}

// DeviceID returns device ID
func (device *GenericDevice) DeviceID() string {
	return device.ID
}

// GetMajorMinor returns device major and minor numbers
func (device *GenericDevice) GetMajorMinor() (int64, int64) {
	return device.DeviceInfo.Major, device.DeviceInfo.Minor
}

// Reference adds one reference to device
func (device *GenericDevice) Reference() uint {
	if device.RefCount != intMax {
		device.RefCount++
	}
	return device.RefCount
}

// Dereference remove one reference from device
func (device *GenericDevice) Dereference() uint {
	if device.RefCount != 0 {
		device.RefCount--
	}
	return device.RefCount
}

// bumpAttachCount is used to add/minus attach count for a device
// * attach bool: true means attach, false means detach
// return values:
// * skip bool: no need to do real attach/detach, skip following actions.
// * err error: error while do attach count bump
func (device *GenericDevice) bumpAttachCount(attach bool) (skip bool, err error) {
	if attach { // attach use case
		switch device.AttachCount {
		case 0:
			// do real attach
			return false, nil
		case intMax:
			return true, fmt.Errorf("device was attached too many times")
		default:
			device.AttachCount++
			return true, nil
		}
	} else { // detach use case
		switch device.AttachCount {
		case 0:
			return true, fmt.Errorf("detaching a device that wasn't attached")
		case 1:
			// do real work
			return false, nil
		default:
			device.AttachCount--
			return true, nil
		}
	}
}
