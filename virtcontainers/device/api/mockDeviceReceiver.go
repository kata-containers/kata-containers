// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package api

import (
	"github.com/kata-containers/runtime/virtcontainers/device/config"
)

// MockDeviceReceiver is a fake DeviceReceiver API implementation only used for test
type MockDeviceReceiver struct{}

// HotplugAddDevice adds a new device
func (mockDC *MockDeviceReceiver) HotplugAddDevice(Device, config.DeviceType) error {
	return nil
}

// HotplugRemoveDevice removes a device
func (mockDC *MockDeviceReceiver) HotplugRemoveDevice(Device, config.DeviceType) error {
	return nil
}

// GetAndSetSandboxBlockIndex is used for get and set virtio-blk indexes
func (mockDC *MockDeviceReceiver) GetAndSetSandboxBlockIndex() (int, error) {
	return 0, nil
}

// DecrementSandboxBlockIndex decreases virtio-blk index by one
func (mockDC *MockDeviceReceiver) DecrementSandboxBlockIndex() error {
	return nil
}

// AddVhostUserDevice adds new vhost user device
func (mockDC *MockDeviceReceiver) AddVhostUserDevice(VhostUserDevice, config.DeviceType) error {
	return nil
}
