// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package api

import (
	"context"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/sirupsen/logrus"
)

var devLogger = logrus.WithField("subsystem", "device")

// SetLogger sets the logger for device api package.
func SetLogger(logger *logrus.Entry) {
	fields := devLogger.Data
	devLogger = logger.WithFields(fields)
}

// DeviceLogger returns logger for device management
func DeviceLogger() *logrus.Entry {
	return devLogger
}

// DeviceReceiver is an interface used for accepting devices
// a device should be attached/added/plugged to a DeviceReceiver
type DeviceReceiver interface {
	// these are for hotplug/hot-unplug devices to/from hypervisor
	HotplugAddDevice(context.Context, Device, config.DeviceType) error
	HotplugRemoveDevice(context.Context, Device, config.DeviceType) error

	// this is only for virtio-blk and virtio-scsi support
	GetAndSetSandboxBlockIndex() (int, error)
	UnsetSandboxBlockIndex(int) error
	GetHypervisorType() string

	// this is for appending device to hypervisor boot params
	AppendDevice(context.Context, Device) error
}

// Device is the virtcontainers device interface.
type Device interface {
	Attach(context.Context, DeviceReceiver) error
	Detach(context.Context, DeviceReceiver) error

	// ID returns device identifier
	DeviceID() string

	// DeviceType indicates which kind of device it is
	// e.g. block, vfio or vhost user
	DeviceType() config.DeviceType

	// GetMajorMinor returns major and minor numbers
	GetMajorMinor() (int64, int64)

	// GetHostPath return the device path in the host
	GetHostPath() string

	// GetDeviceInfo returns device specific data used for hotplugging by hypervisor
	// Caller could cast the return value to device specific struct
	// e.g. Block device returns *config.BlockDrive,
	// vfio device returns []*config.VFIODev,
	// VhostUser device returns []*config.VhostUserDeviceAttrs
	GetDeviceInfo() interface{}

	// GetAttachCount returns how many times the device has been attached
	GetAttachCount() uint

	// Reference adds one reference to device then returns final ref count
	Reference() uint

	// Dereference removes one reference to device then returns final ref count
	Dereference() uint

	// Save converts Device to DeviceState
	Save() persistapi.DeviceState

	// Load loads DeviceState and converts it to specific device
	Load(persistapi.DeviceState)
}

// DeviceManager can be used to create a new device, this can be used as single
// device management object.
type DeviceManager interface {
	NewDevice(config.DeviceInfo) (Device, error)
	RemoveDevice(string) error
	AttachDevice(context.Context, string, DeviceReceiver) error
	DetachDevice(context.Context, string, DeviceReceiver) error
	IsDeviceAttached(string) bool
	GetDeviceByID(string) Device
	GetAllDevices() []Device
	LoadDevices([]persistapi.DeviceState)
}
