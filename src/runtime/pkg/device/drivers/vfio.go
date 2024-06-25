// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018-2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/sirupsen/logrus"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// bind/unbind paths to aid in SRIOV VF bring-up/restore
const (
	pciDriverUnbindPath   = "/sys/bus/pci/devices/%s/driver/unbind"
	pciDriverOverridePath = "/sys/bus/pci/devices/%s/driver_override"
	driversProbePath      = "/sys/bus/pci/drivers_probe"
	iommuGroupPath        = "/sys/bus/pci/devices/%s/iommu_group"
	vfioDevPath           = "/dev/vfio/%s"
	vfioAPSysfsDir        = "/sys/devices/vfio_ap"
)

// VFIODevice is a vfio device meant to be passed to the hypervisor
// to be used by the Virtual Machine.
type VFIODevice struct {
	*GenericDevice
	VfioDevs []*config.VFIODev
}

// NewVFIODevice create a new VFIO device
func NewVFIODevice(devInfo *config.DeviceInfo) *VFIODevice {
	return &VFIODevice{
		GenericDevice: &GenericDevice{
			ID:         devInfo.ID,
			DeviceInfo: devInfo,
		},
	}
}

// Attach is standard interface of api.Device, it's used to add device to some
// DeviceReceiver
func (device *VFIODevice) Attach(ctx context.Context, devReceiver api.DeviceReceiver) (retErr error) {
	skip, err := device.bumpAttachCount(true)
	if err != nil {
		return err
	}
	if skip {
		return nil
	}

	defer func() {
		if retErr != nil {
			device.bumpAttachCount(false)
		}
	}()

	device.VfioDevs, err = GetAllVFIODevicesFromIOMMUGroup(*device.DeviceInfo)
	if err != nil {
		return err
	}

	for _, vfio := range device.VfioDevs {
		// If vfio.Port is not set we bail out, users should set
		// explicitly the port in the config file
		if vfio.Port == "" {
			return fmt.Errorf("cold_plug_vfio= or hot_plug_vfio= port is not set for device %s (BridgePort | RootPort | SwitchPort)", vfio.BDF)
		}

		if vfio.IsPCIe {
			busIndex := len(config.PCIeDevicesPerPort[vfio.Port])
			vfio.Bus = fmt.Sprintf("%s%d", config.PCIePortPrefixMapping[vfio.Port], busIndex)
			// We need to keep track the number of devices per port to deduce
			// the corectu bus number, additionally we can use the VFIO device
			// info to act upon different Vendor IDs and Device IDs.
			config.PCIeDevicesPerPort[vfio.Port] = append(config.PCIeDevicesPerPort[vfio.Port], *vfio)
		}
	}

	coldPlug := device.DeviceInfo.ColdPlug
	deviceLogger().WithField("cold-plug", coldPlug).Info("Attaching VFIO device")

	if coldPlug {
		if err := devReceiver.AppendDevice(ctx, device); err != nil {
			deviceLogger().WithError(err).Error("Failed to append device")
			return err
		}
	} else {
		// hotplug a VFIO device is actually hotplugging a group of iommu devices
		if err := devReceiver.HotplugAddDevice(ctx, device, config.DeviceVFIO); err != nil {
			deviceLogger().WithError(err).Error("Failed to add device")
			return err
		}
	}

	deviceLogger().WithFields(logrus.Fields{
		"device-group": device.DeviceInfo.HostPath,
		"device-type":  "vfio-passthrough",
	}).Info("Device group attached")
	return nil
}

// Detach is standard interface of api.Device, it's used to remove device from some
// DeviceReceiver
func (device *VFIODevice) Detach(ctx context.Context, devReceiver api.DeviceReceiver) (retErr error) {
	skip, err := device.bumpAttachCount(false)
	if err != nil {
		return err
	}
	if skip {
		return nil
	}

	defer func() {
		if retErr != nil {
			device.bumpAttachCount(true)
		}
	}()

	if device.GenericDevice.DeviceInfo.ColdPlug {
		// nothing to detach, device was cold plugged
		deviceLogger().WithFields(logrus.Fields{
			"device-group": device.DeviceInfo.HostPath,
			"device-type":  "vfio-passthrough",
		}).Info("Nothing to detach. VFIO device was cold plugged")
		return nil
	}

	// hotplug a VFIO device is actually hotplugging a group of iommu devices
	if err := devReceiver.HotplugRemoveDevice(ctx, device, config.DeviceVFIO); err != nil {
		deviceLogger().WithError(err).Error("Failed to remove device")
		return err
	}
	for _, vfio := range device.VfioDevs {
		if vfio.IsPCIe {
			for ix, dev := range config.PCIeDevicesPerPort[vfio.Port] {
				if dev.BDF == vfio.BDF {
					config.PCIeDevicesPerPort[vfio.Port] = append(config.PCIeDevicesPerPort[vfio.Port][:ix], config.PCIeDevicesPerPort[vfio.Port][ix+1:]...)
					break
				}
			}
		}
	}

	deviceLogger().WithFields(logrus.Fields{
		"device-group": device.DeviceInfo.HostPath,
		"device-type":  "vfio-passthrough",
	}).Info("Device group detached")
	return nil
}

// DeviceType is standard interface of api.Device, it returns device type
func (device *VFIODevice) DeviceType() config.DeviceType {
	return config.DeviceVFIO
}

// GetDeviceInfo returns device information used for creating
func (device *VFIODevice) GetDeviceInfo() interface{} {
	return device.VfioDevs
}

// Save converts Device to DeviceState
func (device *VFIODevice) Save() config.DeviceState {
	ds := device.GenericDevice.Save()
	ds.Type = string(device.DeviceType())

	devs := device.VfioDevs
	for _, dev := range devs {
		if dev != nil {
			ds.VFIODevs = append(ds.VFIODevs, dev)
		}
	}
	return ds
}

// Load loads DeviceState and converts it to specific device
func (device *VFIODevice) Load(ds config.DeviceState) {
	device.GenericDevice = &GenericDevice{}
	device.GenericDevice.Load(ds)

	for _, dev := range ds.VFIODevs {
		var vfio config.VFIODev

		switch dev.Type {
		case config.VFIOPCIDeviceNormalType, config.VFIOPCIDeviceMediatedType:
			vfio = config.VFIODev{
				ID:       dev.ID,
				Type:     config.VFIODeviceType(dev.Type),
				BDF:      dev.BDF,
				SysfsDev: dev.SysfsDev,
			}
		case config.VFIOAPDeviceMediatedType:
			vfio = config.VFIODev{
				ID:       dev.ID,
				SysfsDev: dev.SysfsDev,
			}
		default:
			deviceLogger().WithError(
				fmt.Errorf("VFIO device type unrecognized"),
			).Error("Failed to append device")
			return
		}

		device.VfioDevs = append(device.VfioDevs, &vfio)
	}
}

// It should implement GetAttachCount() and DeviceID() as api.Device implementation
// here it shares function from *GenericDevice so we don't need duplicate codes
func GetVFIODetails(deviceFileName, iommuDevicesPath string) (deviceBDF, deviceSysfsDev string, vfioDeviceType config.VFIODeviceType, err error) {
	sysfsDevStr := filepath.Join(iommuDevicesPath, deviceFileName)
	vfioDeviceType, err = GetVFIODeviceType(sysfsDevStr)
	if err != nil {
		return deviceBDF, deviceSysfsDev, vfioDeviceType, err
	}

	switch vfioDeviceType {
	case config.VFIOPCIDeviceNormalType:
		// Get bdf of device eg. 0000:00:1c.0
		// OLD IMPL: deviceBDF = getBDF(deviceFileName)
		// The old implementation did not consider the case where
		// vfio devices are located on different root busses. The
		// kata-agent will handle the case now, here, use the full PCI addr
		deviceBDF = deviceFileName
		// Get sysfs path used by cloud-hypervisor
		deviceSysfsDev = filepath.Join(config.SysBusPciDevicesPath, deviceFileName)
	case config.VFIOPCIDeviceMediatedType:
		// Get sysfsdev of device eg. /sys/devices/pci0000:00/0000:00:02.0/f79944e4-5a3d-11e8-99ce-479cbab002e4
		sysfsDevStr := filepath.Join(iommuDevicesPath, deviceFileName)
		deviceSysfsDev, err = GetSysfsDev(sysfsDevStr)
		deviceBDF = GetBDF(getMediatedBDF(deviceSysfsDev))
	case config.VFIOAPDeviceMediatedType:
		sysfsDevStr := filepath.Join(iommuDevicesPath, deviceFileName)
		deviceSysfsDev, err = GetSysfsDev(sysfsDevStr)
	default:
		err = fmt.Errorf("Incorrect tokens found while parsing vfio details: %s", deviceFileName)
	}

	return deviceBDF, deviceSysfsDev, vfioDeviceType, err
}

// getMediatedBDF returns the BDF of a VF
// Expected input string format is /sys/devices/pci0000:d7/BDF0/BDF1/.../MDEVBDF/UUID
func getMediatedBDF(deviceSysfsDev string) string {
	tokens := strings.SplitN(deviceSysfsDev, "/", -1)
	if len(tokens) < 4 {
		return ""
	}
	return tokens[len(tokens)-2]
}

// getBDF returns the BDF of pci device
// Expected input string format is [<domain>]:[<bus>][<slot>].[<func>] eg. 0000:02:10.0
func GetBDF(deviceSysStr string) string {
	tokens := strings.SplitN(deviceSysStr, ":", 2)
	if len(tokens) == 1 {
		return ""
	}
	return tokens[1]
}

func GetVFIODevPath(bdf string) (string, error) {
	// Determine the iommu group that the device belongs to.
	groupPath, err := os.Readlink(fmt.Sprintf(iommuGroupPath, bdf))
	if err != nil {
		return "", err
	}

	return fmt.Sprintf(vfioDevPath, filepath.Base(groupPath)), nil
}

// BindDevicetoVFIO binds the device to vfio driver after unbinding from host
// driver if present.
// Will be called by a network interface or a generic pcie device.
func BindDevicetoVFIO(bdf, hostDriver string) (string, error) {

	overrideDriverPath := fmt.Sprintf(pciDriverOverridePath, bdf)
	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":           bdf,
		"driver-override-path": overrideDriverPath,
	}).Info("Write vfio-pci to driver_override")

	// Write vfio-pci to driver_override file to allow the device to bind to vfio-pci
	// Reference: https://www.kernel.org/doc/Documentation/ABI/testing/sysfs-bus-platform
	if err := utils.WriteToFile(overrideDriverPath, []byte("vfio-pci")); err != nil {
		return "", err
	}

	unbindDriverPath := fmt.Sprintf(pciDriverUnbindPath, bdf)
	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":  bdf,
		"driver-path": unbindDriverPath,
	}).Info("Unbinding device from driver")

	// Unbind device from the host driver. In some cases, a driver may not be bound
	// to the device, in which case this step may fail. Hence ignore error for this step.
	utils.WriteToFile(unbindDriverPath, []byte(bdf))

	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":         bdf,
		"drivers-probe-path": driversProbePath,
	}).Info("Writing bdf to drivers-probe-path")

	// Invoke drivers_probe so that the driver matching driver_override, in our case
	// the vfio-pci driver will probe the device.
	if err := utils.WriteToFile(driversProbePath, []byte(bdf)); err != nil {
		return "", err
	}

	return GetVFIODevPath(bdf)
}

// BindDevicetoHost unbinds the device from vfio-pci driver and binds it to the
// previously bound driver.
func BindDevicetoHost(bdf, hostDriver string) error {
	overrideDriverPath := fmt.Sprintf(pciDriverOverridePath, bdf)
	api.DeviceLogger().WithFields(logrus.Fields{
		"device-bdf":           bdf,
		"driver-override-path": overrideDriverPath,
	}).Infof("Write %s to driver_override", hostDriver)

	// write previously bound host driver to driver_override to allow the
	// device to bind to it. This could be empty which means the device will not be
	// bound to any driver later on.
	if err := utils.WriteToFile(overrideDriverPath, []byte(hostDriver)); err != nil {
		return err
	}

	// Unbind device from vfio-pci driver.
	unbindDriverPath := fmt.Sprintf(pciDriverUnbindPath, bdf)
	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":  bdf,
		"driver-path": unbindDriverPath,
	}).Info("Unbinding device from driver")

	if err := utils.WriteToFile(unbindDriverPath, []byte(bdf)); err != nil {
		return err
	}

	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":         bdf,
		"drivers-probe-path": driversProbePath,
	}).Info("Writing bdf to drivers-probe-path")

	// Invoke drivers_probe so that the driver matching driver_override, in this case
	// the previous host driver will probe the device.
	return utils.WriteToFile(driversProbePath, []byte(bdf))
}
