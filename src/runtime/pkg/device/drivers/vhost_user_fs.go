// Copyright (C) 2019 Red Hat, Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"context"
	"encoding/hex"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// VhostUserFSDevice is a virtio-fs vhost-user device
type VhostUserFSDevice struct {
	*GenericDevice
	config.VhostUserDeviceAttrs
}

// Device interface

func (device *VhostUserFSDevice) Attach(ctx context.Context, devReceiver api.DeviceReceiver) (err error) {
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

func (device *VhostUserFSDevice) Detach(ctx context.Context, devReceiver api.DeviceReceiver) error {
	_, err := device.bumpAttachCount(false)
	return err
}

func (device *VhostUserFSDevice) DeviceType() config.DeviceType {
	return config.VhostUserFS
}

// GetDeviceInfo returns device information that the device is created based on
func (device *VhostUserFSDevice) GetDeviceInfo() interface{} {
	device.Type = device.DeviceType()
	return &device.VhostUserDeviceAttrs
}
