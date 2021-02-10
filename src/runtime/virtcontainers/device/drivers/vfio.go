// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018-2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"context"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/sirupsen/logrus"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// bind/unbind paths to aid in SRIOV VF bring-up/restore
const (
	pciDriverUnbindPath = "/sys/bus/pci/devices/%s/driver/unbind"
	pciDriverBindPath   = "/sys/bus/pci/drivers/%s/bind"
	vfioNewIDPath       = "/sys/bus/pci/drivers/vfio-pci/new_id"
	vfioRemoveIDPath    = "/sys/bus/pci/drivers/vfio-pci/remove_id"
	iommuGroupPath      = "/sys/bus/pci/devices/%s/iommu_group"
	vfioDevPath         = "/dev/vfio/%s"
	pcieRootPortPrefix  = "rp"
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

	vfioGroup := filepath.Base(device.DeviceInfo.HostPath)
	iommuDevicesPath := filepath.Join(config.SysIOMMUPath, vfioGroup, "devices")

	deviceFiles, err := ioutil.ReadDir(iommuDevicesPath)
	if err != nil {
		return err
	}

	// Pass all devices in iommu group
	for i, deviceFile := range deviceFiles {
		//Get bdf of device eg 0000:00:1c.0
		deviceBDF, deviceSysfsDev, vfioDeviceType, err := getVFIODetails(deviceFile.Name(), iommuDevicesPath)
		if err != nil {
			return err
		}
		vfio := &config.VFIODev{
			ID:       utils.MakeNameID("vfio", device.DeviceInfo.ID+strconv.Itoa(i), maxDevIDSize),
			Type:     vfioDeviceType,
			BDF:      deviceBDF,
			SysfsDev: deviceSysfsDev,
			IsPCIe:   isPCIeDevice(deviceBDF),
			Class:    getPCIDeviceProperty(deviceBDF, PCISysFsDevicesClass),
		}
		device.VfioDevs = append(device.VfioDevs, vfio)
		if vfio.IsPCIe {
			vfio.Bus = fmt.Sprintf("%s%d", pcieRootPortPrefix, len(AllPCIeDevs))
			AllPCIeDevs[vfio.BDF] = true
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
func (device *VFIODevice) Save() persistapi.DeviceState {
	ds := device.GenericDevice.Save()
	ds.Type = string(device.DeviceType())

	devs := device.VfioDevs
	for _, dev := range devs {
		if dev != nil {
			ds.VFIODevs = append(ds.VFIODevs, &persistapi.VFIODev{
				ID:       dev.ID,
				Type:     uint32(dev.Type),
				BDF:      dev.BDF,
				SysfsDev: dev.SysfsDev,
			})
		}
	}
	return ds
}

// Load loads DeviceState and converts it to specific device
func (device *VFIODevice) Load(ds persistapi.DeviceState) {
	device.GenericDevice = &GenericDevice{}
	device.GenericDevice.Load(ds)

	for _, dev := range ds.VFIODevs {
		device.VfioDevs = append(device.VfioDevs, &config.VFIODev{
			ID:       dev.ID,
			Type:     config.VFIODeviceType(dev.Type),
			BDF:      dev.BDF,
			SysfsDev: dev.SysfsDev,
		})
	}
}

// It should implement GetAttachCount() and DeviceID() as api.Device implementation
// here it shares function from *GenericDevice so we don't need duplicate codes
func getVFIODetails(deviceFileName, iommuDevicesPath string) (deviceBDF, deviceSysfsDev string, vfioDeviceType config.VFIODeviceType, err error) {
	vfioDeviceType = GetVFIODeviceType(deviceFileName)

	switch vfioDeviceType {
	case config.VFIODeviceNormalType:
		// Get bdf of device eg. 0000:00:1c.0
		deviceBDF = getBDF(deviceFileName)
		// Get sysfs path used by cloud-hypervisor
		deviceSysfsDev = filepath.Join(config.SysBusPciDevicesPath, deviceFileName)
	case config.VFIODeviceMediatedType:
		// Get sysfsdev of device eg. /sys/devices/pci0000:00/0000:00:02.0/f79944e4-5a3d-11e8-99ce-479cbab002e4
		sysfsDevStr := filepath.Join(iommuDevicesPath, deviceFileName)
		deviceSysfsDev, err = getSysfsDev(sysfsDevStr)
	default:
		err = fmt.Errorf("Incorrect tokens found while parsing vfio details: %s", deviceFileName)
	}

	return deviceBDF, deviceSysfsDev, vfioDeviceType, err
}

// getBDF returns the BDF of pci device
// Expected input string format is [<domain>]:[<bus>][<slot>].[<func>] eg. 0000:02:10.0
func getBDF(deviceSysStr string) string {
	tokens := strings.SplitN(deviceSysStr, ":", 2)
	return tokens[1]
}

// getSysfsDev returns the sysfsdev of mediated device
// Expected input string format is absolute path to the sysfs dev node
// eg. /sys/kernel/iommu_groups/0/devices/f79944e4-5a3d-11e8-99ce-479cbab002e4
func getSysfsDev(sysfsDevStr string) (string, error) {
	return filepath.EvalSymlinks(sysfsDevStr)
}

// BindDevicetoVFIO binds the device to vfio driver after unbinding from host.
// Will be called by a network interface or a generic pcie device.
func BindDevicetoVFIO(bdf, hostDriver, vendorDeviceID string) (string, error) {

	// Unbind from the host driver
	unbindDriverPath := fmt.Sprintf(pciDriverUnbindPath, bdf)
	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":  bdf,
		"driver-path": unbindDriverPath,
	}).Info("Unbinding device from driver")

	if err := utils.WriteToFile(unbindDriverPath, []byte(bdf)); err != nil {
		return "", err
	}

	// Add device id to vfio driver.
	deviceLogger().WithFields(logrus.Fields{
		"vendor-device-id": vendorDeviceID,
		"vfio-new-id-path": vfioNewIDPath,
	}).Info("Writing vendor-device-id to vfio new-id path")

	if err := utils.WriteToFile(vfioNewIDPath, []byte(vendorDeviceID)); err != nil {
		return "", err
	}

	// Bind to vfio-pci driver.
	bindDriverPath := fmt.Sprintf(pciDriverBindPath, "vfio-pci")

	api.DeviceLogger().WithFields(logrus.Fields{
		"device-bdf":  bdf,
		"driver-path": bindDriverPath,
	}).Info("Binding device to vfio driver")

	// Device may be already bound at this time because of earlier write to new_id, ignore error
	utils.WriteToFile(bindDriverPath, []byte(bdf))

	groupPath, err := os.Readlink(fmt.Sprintf(iommuGroupPath, bdf))
	if err != nil {
		return "", err
	}

	return fmt.Sprintf(vfioDevPath, filepath.Base(groupPath)), nil
}

// BindDevicetoHost binds the device to the host driver driver after unbinding from vfio-pci.
func BindDevicetoHost(bdf, hostDriver, vendorDeviceID string) error {
	// Unbind from vfio-pci driver
	unbindDriverPath := fmt.Sprintf(pciDriverUnbindPath, bdf)
	api.DeviceLogger().WithFields(logrus.Fields{
		"device-bdf":  bdf,
		"driver-path": unbindDriverPath,
	}).Info("Unbinding device from driver")

	if err := utils.WriteToFile(unbindDriverPath, []byte(bdf)); err != nil {
		return err
	}

	// To prevent new VFs from binding to VFIO-PCI, remove_id
	if err := utils.WriteToFile(vfioRemoveIDPath, []byte(vendorDeviceID)); err != nil {
		return err
	}

	// Bind back to host driver
	bindDriverPath := fmt.Sprintf(pciDriverBindPath, hostDriver)
	api.DeviceLogger().WithFields(logrus.Fields{
		"device-bdf":  bdf,
		"driver-path": bindDriverPath,
	}).Info("Binding back device to host driver")

	return utils.WriteToFile(bindDriverPath, []byte(bdf))
}
