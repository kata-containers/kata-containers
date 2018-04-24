// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package manager

import (
	"github.com/sirupsen/logrus"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
)

const (
	// VirtioBlock indicates block driver is virtio-blk based
	VirtioBlock string = "virtio-blk"
	// VirtioSCSI indicates block driver is virtio-scsi based
	VirtioSCSI string = "virtio-scsi"
)

type deviceManager struct {
	blockDriver string
}

func deviceLogger() *logrus.Entry {
	return api.DeviceLogger().WithField("subsystem", "device")
}

// CreateDevice creates one device based on DeviceInfo
func (dm *deviceManager) CreateDevice(devInfo config.DeviceInfo) api.Device {
	path := devInfo.HostPath

	if isVFIO(path) {
		return drivers.NewVFIODevice(devInfo)
	} else if isBlock(devInfo) {
		if devInfo.DriverOptions == nil {
			devInfo.DriverOptions = make(map[string]string)
		}
		devInfo.DriverOptions["block-driver"] = dm.blockDriver
		return drivers.NewBlockDevice(devInfo)
	} else {
		deviceLogger().WithField("device", path).Info("Device has not been passed to the container")
		return drivers.NewGenericDevice(devInfo)
	}
}

// NewDevices creates bundles of devices based on array of DeviceInfo
func (dm *deviceManager) NewDevices(devInfos []config.DeviceInfo) ([]api.Device, error) {
	var devices []api.Device

	for _, devInfo := range devInfos {
		hostPath, err := config.GetHostPathFunc(devInfo)
		if err != nil {
			return nil, err
		}

		devInfo.HostPath = hostPath
		device := dm.CreateDevice(devInfo)
		devices = append(devices, device)
	}

	return devices, nil
}

// NewDeviceManager creates a deviceManager object bahaved as DeviceManager
func NewDeviceManager(blockDriver string) api.DeviceManager {
	dm := &deviceManager{}
	if blockDriver == VirtioBlock {
		dm.blockDriver = VirtioBlock
	} else {
		dm.blockDriver = VirtioSCSI
	}

	return dm
}
