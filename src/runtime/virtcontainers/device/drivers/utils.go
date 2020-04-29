// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"fmt"
	"io/ioutil"
	"path/filepath"
	"strings"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/sirupsen/logrus"
)

const (
	intMax = ^uint(0)

	PCIDomain   = "0000"
	PCIeKeyword = "PCIe"
)

type PCISysFsType string

var (
	PCISysFsDevices PCISysFsType = "devices" // /sys/bus/pci/devices
	PCISysFsSlots   PCISysFsType = "slots"   // /sys/bus/pci/slots
)

type PCISysFsProperty string

var (
	PCISysFsDevicesClass     PCISysFsProperty = "class"         // /sys/bus/pci/devices/xxx/class
	PCISysFsSlotsAddress     PCISysFsProperty = "address"       // /sys/bus/pci/slots/xxx/address
	PCISysFsSlotsMaxBusSpeed PCISysFsProperty = "max_bus_speed" // /sys/bus/pci/slots/xxx/max_bus_speed
)

func deviceLogger() *logrus.Entry {
	return api.DeviceLogger()
}

/*
Identify PCIe device by /sys/bus/pci/slots/xx/max_bus_speed, sample content "8.0 GT/s PCIe"
The /sys/bus/pci/slots/xx/address contains bdf, sample content "0000:04:00"
bdf format: bus:slot.function
*/
func isPCIeDevice(bdf string) bool {
	if len(strings.Split(bdf, ":")) == 2 {
		bdf = PCIDomain + ":" + bdf
	}
	slots, err := ioutil.ReadDir(config.SysBusPciSlotsPath)
	if err != nil {
		deviceLogger().WithError(err).WithField("path", config.SysBusPciSlotsPath).Warn("failed to list pci slots")
		return false
	}
	b := strings.Split(bdf, ".")[0]
	for _, slot := range slots {
		address := getPCISlotProperty(slot.Name(), PCISysFsSlotsAddress)
		if b == address {
			maxBusSpeed := getPCISlotProperty(slot.Name(), PCISysFsSlotsMaxBusSpeed)
			if strings.Contains(maxBusSpeed, PCIeKeyword) {
				return true
			}
		}
	}
	deviceLogger().WithField("dev-bdf", bdf).Debug("can not find slot for bdf of pci device")
	return false
}

// read from /sys/bus/pci/devices/xxx/property
func getPCIDeviceProperty(bdf string, property PCISysFsProperty) string {
	if len(strings.Split(bdf, ":")) == 2 {
		bdf = PCIDomain + ":" + bdf
	}
	propertyPath := filepath.Join(config.SysBusPciDevicesPath, bdf, string(property))
	rlt, err := readPCIProperty(propertyPath)
	if err != nil {
		deviceLogger().WithError(err).WithField("path", propertyPath).Warn("failed to read pci device property")
		return ""
	}
	return rlt
}

// read from /sys/bus/pci/slots/xxx/property
func getPCISlotProperty(slot string, property PCISysFsProperty) string {
	propertyPath := filepath.Join(config.SysBusPciSlotsPath, slot, string(property))
	rlt, err := readPCIProperty(propertyPath)
	if err != nil {
		deviceLogger().WithError(err).WithField("path", propertyPath).Warn("failed to read pci slot property")
		return ""
	}
	return rlt
}

func readPCIProperty(propertyPath string) (string, error) {
	var (
		buf []byte
		err error
	)
	if buf, err = ioutil.ReadFile(propertyPath); err != nil {
		return "", fmt.Errorf("failed to read pci sysfs %v, error:%v", propertyPath, err)
	}
	return strings.Split(string(buf), "\n")[0], nil
}

func GetVFIODeviceType(deviceFileName string) config.VFIODeviceType {
	//For example, 0000:04:00.0
	tokens := strings.Split(deviceFileName, ":")
	vfioDeviceType := config.VFIODeviceErrorType
	if len(tokens) == 3 {
		vfioDeviceType = config.VFIODeviceNormalType
	} else {
		//For example, 83b8f4f2-509f-382f-3c1e-e6bfe0fa1001
		tokens = strings.Split(deviceFileName, "-")
		if len(tokens) == 5 {
			vfioDeviceType = config.VFIODeviceMediatedType
		}
	}
	return vfioDeviceType
}
