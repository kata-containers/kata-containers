// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package api

import (
	"context"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
)

// MockDeviceReceiver is a fake DeviceReceiver API implementation only used for test
type MockDeviceReceiver struct{}

// HotplugAddDevice adds a new device
func (mockDC *MockDeviceReceiver) HotplugAddDevice(context.Context, Device, config.DeviceType) error {
	return nil
}

// HotplugRemoveDevice removes a device
func (mockDC *MockDeviceReceiver) HotplugRemoveDevice(context.Context, Device, config.DeviceType) error {
	return nil
}

// GetAndSetSandboxBlockIndex is used for get and set virtio-blk indexes
func (mockDC *MockDeviceReceiver) GetAndSetSandboxBlockIndex() (int, error) {
	return 0, nil
}

// DecrementSandboxBlockIndex decreases virtio-blk index by one
func (mockDC *MockDeviceReceiver) UnsetSandboxBlockIndex(int) error {
	return nil
}

// AppendDevice adds new vhost user device
func (mockDC *MockDeviceReceiver) AppendDevice(context.Context, Device) error {
	return nil
}

// GetHypervisorType is used for getting Hypervisor name currently used.
func (mockDC *MockDeviceReceiver) GetHypervisorType() string {
	return ""
}
