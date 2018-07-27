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
	HotplugAddDevice(Device, config.DeviceType) error
	HotplugRemoveDevice(Device, config.DeviceType) error

	// this is only for virtio-blk and virtio-scsi support
	GetAndSetSandboxBlockIndex() (int, error)
	DecrementSandboxBlockIndex() error

	// this is for vhost_user devices
	AddVhostUserDevice(VhostUserDevice, config.DeviceType) error
}

// VhostUserDevice represents a vhost-user device. Shared
// attributes of a vhost-user device can be retrieved using
// the Attrs() method. Unique data can be obtained by casting
// the object to the proper type.
type VhostUserDevice interface {
	Attrs() *config.VhostUserDeviceAttrs
	Type() config.DeviceType
}

// Device is the virtcontainers device interface.
type Device interface {
	Attach(DeviceReceiver) error
	Detach(DeviceReceiver) error
	DeviceType() config.DeviceType
}

// DeviceManager can be used to create a new device, this can be used as single
// device management object.
type DeviceManager interface {
	NewDevices(devInfos []config.DeviceInfo) ([]Device, error)
}
