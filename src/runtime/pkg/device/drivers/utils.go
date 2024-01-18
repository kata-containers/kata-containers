// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/sirupsen/logrus"
)

const (
	intMax = ^uint(0)

	PCIDomain   = "0000"
	PCIeKeyword = "PCIe"

	PCIConfigSpaceSize = 256
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
	PCISysFsDevicesVendor    PCISysFsProperty = "vendor"        // /sys/bus/pci/devices/xxx/vendor
	PCISysFsDevicesDevice    PCISysFsProperty = "device"        // /sys/bus/pci/devices/xxx/device
)

func deviceLogger() *logrus.Entry {
	return api.DeviceLogger()
}

// IsPCIeDevice identifies PCIe device by reading the size of the PCI config space
// Plain PCI device have 256 bytes of config space where PCIe devices have 4K
func IsPCIeDevice(bdf string) bool {
	if len(strings.Split(bdf, ":")) == 2 {
		bdf = PCIDomain + ":" + bdf
	}

	configPath := filepath.Join(config.SysBusPciDevicesPath, bdf, "config")
	fi, err := os.Stat(configPath)
	if err != nil {
		deviceLogger().WithField("dev-bdf", bdf).WithError(err).Warning("Couldn't stat() configuration space file")
		return false //Who knows?
	}

	// Plain PCI devices have 256 bytes of configuration space,
	// PCI-Express devices have 4096 bytes
	return fi.Size() > PCIConfigSpaceSize
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

func readPCIProperty(propertyPath string) (string, error) {
	var (
		buf []byte
		err error
	)
	if buf, err = os.ReadFile(propertyPath); err != nil {
		return "", fmt.Errorf("failed to read pci sysfs %v, error:%v", propertyPath, err)
	}
	return strings.Split(string(buf), "\n")[0], nil
}

func GetVFIODeviceType(deviceFilePath string) (config.VFIODeviceType, error) {
	deviceFileName := filepath.Base(deviceFilePath)

	//For example, 0000:04:00.0
	tokens := strings.Split(deviceFileName, ":")
	if len(tokens) == 3 {
		return config.VFIOPCIDeviceNormalType, nil
	}

	//For example, 83b8f4f2-509f-382f-3c1e-e6bfe0fa1001
	tokens = strings.Split(deviceFileName, "-")
	if len(tokens) != 5 {
		return config.VFIODeviceErrorType, fmt.Errorf("Incorrect tokens found while parsing VFIO details: %s", deviceFileName)
	}

	deviceSysfsDev, err := GetSysfsDev(deviceFilePath)
	if err != nil {
		return config.VFIODeviceErrorType, err
	}

	if strings.Contains(deviceSysfsDev, vfioAPSysfsDir) {
		return config.VFIOAPDeviceMediatedType, nil
	}

	return config.VFIOPCIDeviceMediatedType, nil
}

// GetSysfsDev returns the sysfsdev of mediated device
// Expected input string format is absolute path to the sysfs dev node
// eg. /sys/kernel/iommu_groups/0/devices/f79944e4-5a3d-11e8-99ce-479cbab002e4
func GetSysfsDev(sysfsDevStr string) (string, error) {
	return filepath.EvalSymlinks(sysfsDevStr)
}

// GetAPVFIODevices retrieves all APQNs associated with a mediated VFIO-AP
// device
func GetAPVFIODevices(sysfsdev string) ([]string, error) {
	data, err := os.ReadFile(filepath.Join(sysfsdev, "matrix"))
	if err != nil {
		return []string{}, err
	}
	// Split by newlines, omitting final newline
	return strings.Split(string(data[:len(data)-1]), "\n"), nil
}

// Ignore specific PCI devices, supply the pciClass and the bitmask to check
// against the device class, deviceBDF for meaningfull info message
func checkIgnorePCIClass(pciClass string, deviceBDF string, bitmask uint64) (bool, error) {
	if pciClass == "" {
		return false, nil
	}
	pciClassID, err := strconv.ParseUint(pciClass, 0, 32)
	if err != nil {
		return false, err
	}
	// ClassID is 16 bits, remove the two trailing zeros
	pciClassID = pciClassID >> 8
	if pciClassID&bitmask == bitmask {
		deviceLogger().Infof("Ignoring PCI (Host) Bridge deviceBDF %v Class %x", deviceBDF, pciClassID)
		return true, nil
	}
	return false, nil
}

// GetAllVFIODevicesFromIOMMUGroup returns all the VFIO devices in the IOMMU group
// We can reuse this function at various levels, sandbox, container.
func GetAllVFIODevicesFromIOMMUGroup(device config.DeviceInfo) ([]*config.VFIODev, error) {

	vfioDevs := []*config.VFIODev{}

	vfioGroup := filepath.Base(device.HostPath)
	iommuDevicesPath := filepath.Join(config.SysIOMMUGroupPath, vfioGroup, "devices")

	deviceFiles, err := os.ReadDir(iommuDevicesPath)
	if err != nil {
		return nil, err
	}

	// Pass all devices in iommu group
	for i, deviceFile := range deviceFiles {
		//Get bdf of device eg 0000:00:1c.0
		deviceBDF, deviceSysfsDev, vfioDeviceType, err := GetVFIODetails(deviceFile.Name(), iommuDevicesPath)
		if err != nil {
			return nil, err
		}
		id := utils.MakeNameID("vfio", device.ID+strconv.Itoa(i), maxDevIDSize)

		var vfio config.VFIODev

		switch vfioDeviceType {
		case config.VFIOPCIDeviceNormalType, config.VFIOPCIDeviceMediatedType:
			// This is vfio-pci and vfio-mdev specific
			pciClass := getPCIDeviceProperty(deviceBDF, PCISysFsDevicesClass)
			// We need to ignore Host or PCI Bridges that are in the same IOMMU group as the
			// passed-through devices. One CANNOT pass-through a PCI bridge or Host bridge.
			// Class 0x0604 is PCI bridge, 0x0600 is Host bridge
			ignorePCIDevice, err := checkIgnorePCIClass(pciClass, deviceBDF, 0x0600)
			if err != nil {
				return nil, err
			}
			if ignorePCIDevice {
				continue
			}
			// Fetch the PCI Vendor ID and Device ID
			vendorID := getPCIDeviceProperty(deviceBDF, PCISysFsDevicesVendor)
			deviceID := getPCIDeviceProperty(deviceBDF, PCISysFsDevicesDevice)

			// Do not directly assign to `vfio` -- need to access field still
			vfio = config.VFIODev{
				ID:       id,
				Type:     vfioDeviceType,
				BDF:      deviceBDF,
				SysfsDev: deviceSysfsDev,
				IsPCIe:   IsPCIeDevice(deviceBDF),
				Class:    pciClass,
				VendorID: vendorID,
				DeviceID: deviceID,
				Port:     device.Port,
				HostPath: device.HostPath,
			}

		case config.VFIOAPDeviceMediatedType:
			devices, err := GetAPVFIODevices(deviceSysfsDev)
			if err != nil {
				return nil, err
			}
			vfio = config.VFIODev{
				ID:        id,
				SysfsDev:  deviceSysfsDev,
				Type:      config.VFIOAPDeviceMediatedType,
				APDevices: devices,
				Port:      device.Port,
			}
		default:
			return nil, fmt.Errorf("Failed to append device: VFIO device type unrecognized")
		}

		vfioDevs = append(vfioDevs, &vfio)
	}

	return vfioDevs, nil
}
