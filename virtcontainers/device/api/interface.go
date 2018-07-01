// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package api

import (
	"github.com/sirupsen/logrus"

	"github.com/kata-containers/runtime/virtcontainers/device/config"
)

var devLogger = logrus.FieldLogger(logrus.New())

// SetLogger sets the logger for device api package.
func SetLogger(logger logrus.FieldLogger) {
	devLogger = logger
}

// DeviceLogger returns logger for device management
func DeviceLogger() *logrus.Entry {
	return devLogger.WithField("subsystem", "device")
}

// DeviceReceiver is an interface used for accepting devices
// a device should be attached/added/plugged to a DeviceReceiver
type DeviceReceiver interface {
	// these are for hotplug/hot-unplug devices to/from hypervisor
	HotplugAddDevice(Device, config.DeviceType) error
	HotplugRemoveDevice(Device, config.DeviceType) error

	// this is only for virtio-blk and virtio-scsi support
	GetAndSetSandboxBlockIndex() (int, error)
	DecrementSandboxBlockIndex() error

	// this is for appending device to hypervisor boot params
	AppendDevice(Device) error
}

// Device is the virtcontainers device interface.
type Device interface {
	Attach(DeviceReceiver) error
	Detach(DeviceReceiver) error
	// ID returns device identifier
	DeviceID() string
	// DeviceType indicates which kind of device it is
	// e.g. block, vfio or vhost user
	DeviceType() config.DeviceType
	// GetDeviceInfo returns device information that the device is created based on
	GetDeviceInfo() *config.DeviceInfo
	// GetDeviceDrive returns device specific data used for hotplugging by hypervisor
	// Caller could cast the return value to device specific struct
	// e.g. Block device returns *config.BlockDrive and
	// vfio device returns *config.VFIODrive
	GetDeviceDrive() interface{}
	// IsAttached checks if the device is attached
	IsAttached() bool
}

// DeviceManager can be used to create a new device, this can be used as single
// device management object.
type DeviceManager interface {
	NewDevice(config.DeviceInfo) (Device, error)
	AttachDevice(string, DeviceReceiver) error
	DetachDevice(string, DeviceReceiver) error
	IsDeviceAttached(string) bool
	GetDeviceByID(string) Device
	GetAllDevices() []Device
}
