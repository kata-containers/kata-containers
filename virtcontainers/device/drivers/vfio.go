// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"encoding/hex"
	"fmt"
	"io/ioutil"
	"path/filepath"
	"strings"

	"github.com/sirupsen/logrus"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/utils"
)

// VFIODevice is a vfio device meant to be passed to the hypervisor
// to be used by the Virtual Machine.
type VFIODevice struct {
	DevType    config.DeviceType
	DeviceInfo config.DeviceInfo
	BDF        string
}

// NewVFIODevice create a new VFIO device
func NewVFIODevice(devInfo config.DeviceInfo) *VFIODevice {
	return &VFIODevice{
		DevType:    config.DeviceVFIO,
		DeviceInfo: devInfo,
	}
}

// Attach is standard interface of api.Device, it's used to add device to some
// DeviceReceiver
func (device *VFIODevice) Attach(devReceiver api.DeviceReceiver) error {
	vfioGroup := filepath.Base(device.DeviceInfo.HostPath)
	iommuDevicesPath := filepath.Join(config.SysIOMMUPath, vfioGroup, "devices")

	deviceFiles, err := ioutil.ReadDir(iommuDevicesPath)
	if err != nil {
		return err
	}

	// Pass all devices in iommu group
	for _, deviceFile := range deviceFiles {

		//Get bdf of device eg 0000:00:1c.0
		deviceBDF, err := getBDF(deviceFile.Name())
		if err != nil {
			return err
		}

		device.BDF = deviceBDF

		randBytes, err := utils.GenerateRandomBytes(8)
		if err != nil {
			return err
		}
		device.DeviceInfo.ID = hex.EncodeToString(randBytes)

		if err := devReceiver.HotplugAddDevice(device, config.DeviceVFIO); err != nil {
			deviceLogger().WithError(err).Error("Failed to add device")
			return err
		}

		deviceLogger().WithFields(logrus.Fields{
			"device-group": device.DeviceInfo.HostPath,
			"device-type":  "vfio-passthrough",
		}).Info("Device group attached")
	}

	return nil
}

// Detach is standard interface of api.Device, it's used to remove device from some
// DeviceReceiver
func (device *VFIODevice) Detach(devReceiver api.DeviceReceiver) error {
	return nil
}

// DeviceType is standard interface of api.Device, it returns device type
func (device *VFIODevice) DeviceType() config.DeviceType {
	return device.DevType
}

// getBDF returns the BDF of pci device
// Expected input strng format is [<domain>]:[<bus>][<slot>].[<func>] eg. 0000:02:10.0
func getBDF(deviceSysStr string) (string, error) {
	tokens := strings.Split(deviceSysStr, ":")

	if len(tokens) != 3 {
		return "", fmt.Errorf("Incorrect number of tokens found while parsing bdf for device : %s", deviceSysStr)
	}

	tokens = strings.SplitN(deviceSysStr, ":", 2)
	return tokens[1], nil
}
