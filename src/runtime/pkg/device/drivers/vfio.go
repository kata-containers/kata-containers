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
	pcieRootPortPrefix    = "rp"
	vfioAPSysfsDir        = "/sys/devices/vfio_ap"
)

var (
	AllPCIeDevs = map[string]bool{}
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

	device.VfioDevs, err = GetAllVFIODevicesFromIOMMUGroup(*device.DeviceInfo, false)
	if err != nil {
		return err
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

		vfioDeviceType := (*device.VfioDevs[0]).GetType()
		switch vfioDeviceType {
		case config.VFIOPCIDeviceNormalType, config.VFIOPCIDeviceMediatedType:
			bdf := ""
			if pciDev, ok := (*dev).(config.VFIOPCIDev); ok {
				bdf = pciDev.BDF
			}
			vfio = config.VFIOPCIDev{
				ID:       *(*dev).GetID(),
				Type:     config.VFIODeviceType((*dev).GetType()),
				BDF:      bdf,
				SysfsDev: *(*dev).GetSysfsDev(),
			}
		case config.VFIOAPDeviceMediatedType:
			vfio = config.VFIOAPDev{
				ID:       *(*dev).GetID(),
				SysfsDev: *(*dev).GetSysfsDev(),
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
func getVFIODetails(deviceFileName, iommuDevicesPath string) (deviceBDF, deviceSysfsDev string, vfioDeviceType config.VFIODeviceType, err error) {
	sysfsDevStr := filepath.Join(iommuDevicesPath, deviceFileName)
	vfioDeviceType, err = GetVFIODeviceType(sysfsDevStr)
	if err != nil {
		return deviceBDF, deviceSysfsDev, vfioDeviceType, err
	}

	switch vfioDeviceType {
	case config.VFIOPCIDeviceNormalType:
		// Get bdf of device eg. 0000:00:1c.0
		deviceBDF = getBDF(deviceFileName)
		// Get sysfs path used by cloud-hypervisor
		deviceSysfsDev = filepath.Join(config.SysBusPciDevicesPath, deviceFileName)
	case config.VFIOPCIDeviceMediatedType:
		// Get sysfsdev of device eg. /sys/devices/pci0000:00/0000:00:02.0/f79944e4-5a3d-11e8-99ce-479cbab002e4
		sysfsDevStr := filepath.Join(iommuDevicesPath, deviceFileName)
		deviceSysfsDev, err = GetSysfsDev(sysfsDevStr)
		deviceBDF = getBDF(getMediatedBDF(deviceSysfsDev))
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
func getBDF(deviceSysStr string) string {
	tokens := strings.SplitN(deviceSysStr, ":", 2)
	if len(tokens) == 1 {
		return ""
	}
	return tokens[1]
}

// BindDevicetoVFIO binds the device to vfio driver after unbinding from host.
// Will be called by a network interface or a generic pcie device.
func BindDevicetoVFIO(bdf, hostDriver string) (string, error) {

	// write vfio-pci to driver_override
	overrideDriverPath := fmt.Sprintf(pciDriverOverridePath, bdf)
	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":           bdf,
		"driver-override-path": overrideDriverPath,
	}).Info("Write vfio-pci to driver_override")

	if err := utils.WriteToFile(overrideDriverPath, []byte("vfio-pci")); err != nil {
		return "", err
	}

    // Unbind from the host driver
	unbindDriverPath := fmt.Sprintf(pciDriverUnbindPath, bdf)
	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":  bdf,
		"driver-path": unbindDriverPath,
	}).Info("Unbinding device from driver")

	// driver may not exist on system. ignore error
	utils.WriteToFile(unbindDriverPath, []byte(bdf))

	// probe vfio driver.
	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":         bdf,
		"drivers-probe-path": driversProbePath,
	}).Info("Writing bdf to drivers-probe-path")

	if err := utils.WriteToFile(driversProbePath, []byte(bdf)); err != nil {
		return "", err
	}

	groupPath, err := os.Readlink(fmt.Sprintf(iommuGroupPath, bdf))
	if err != nil {
		return "", err
	}

	return fmt.Sprintf(vfioDevPath, filepath.Base(groupPath)), nil
}

// BindDevicetoHost binds the device to the host driver after unbinding from vfio-pci.
func BindDevicetoHost(bdf, hostDriver string) error {
	// write hostDriver to driver_override
	overrideDriverPath := fmt.Sprintf(pciDriverOverridePath, bdf)
	api.DeviceLogger().WithFields(logrus.Fields{
		"device-bdf":           bdf,
		"driver-override-path": overrideDriverPath,
	}).Infof("Write %s to driver_override", hostDriver)

	if err := utils.WriteToFile(overrideDriverPath, []byte(hostDriver)); err != nil {
		return err
	}

    // Unbind from the user driver
	unbindDriverPath := fmt.Sprintf(pciDriverUnbindPath, bdf)
	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":  bdf,
		"driver-path": unbindDriverPath,
	}).Info("Unbinding device from driver")

	// driver may not exist on system. ignore error
	utils.WriteToFile(unbindDriverPath, []byte(bdf))

	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":         bdf,
		"drivers-probe-path": driversProbePath,
	}).Info("Writing bdf to drivers-probe-path")

	return utils.WriteToFile(driversProbePath, []byte(bdf))
}
