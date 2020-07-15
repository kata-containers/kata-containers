// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package manager

import (
	"fmt"
	"io/ioutil"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/sirupsen/logrus"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/drivers"
)

const (
	vfioPath = "/dev/vfio/"
)

// isVFIO checks if the device provided is a vfio group.
func isVFIO(hostPath string) bool {
	// Ignore /dev/vfio/vfio character device
	if strings.HasPrefix(hostPath, filepath.Join(vfioPath, "vfio")) {
		return false
	}

	if strings.HasPrefix(hostPath, vfioPath) && len(hostPath) > len(vfioPath) {
		return true
	}

	return false
}

// isBlock checks if the device is a block device.
func isBlock(devInfo config.DeviceInfo) bool {
	return devInfo.DevType == "b"
}

// IsVFIOLargeBarSpaceDevice checks if the device is a large bar space device.
func IsVFIOLargeBarSpaceDevice(hostPath string) (bool, error) {
	if !isVFIO(hostPath) {
		return false, nil
	}

	iommuDevicesPath := filepath.Join(config.SysIOMMUPath, filepath.Base(hostPath), "devices")
	deviceFiles, err := ioutil.ReadDir(iommuDevicesPath)
	if err != nil {
		return false, err
	}

	// Pass all devices in iommu group
	for _, deviceFile := range deviceFiles {
		vfioDeviceType := drivers.GetVFIODeviceType(deviceFile.Name())
		var isLarge bool
		switch vfioDeviceType {
		case config.VFIODeviceNormalType:
			sysfsResource := filepath.Join(iommuDevicesPath, deviceFile.Name(), "resource")
			if isLarge, err = isLargeBarSpace(sysfsResource); err != nil {
				return false, err
			}
			deviceLogger().WithFields(logrus.Fields{
				"device-file":     deviceFile.Name(),
				"device-type":     vfioDeviceType,
				"resource":        sysfsResource,
				"large-bar-space": isLarge,
			}).Info("Detect large bar space device")
			return isLarge, nil
		case config.VFIODeviceMediatedType:
			//TODO: support VFIODeviceMediatedType
			deviceLogger().WithFields(logrus.Fields{
				"device-file": deviceFile.Name(),
				"device-type": vfioDeviceType,
			}).Warn("Detect large bar space device is not yet supported for VFIODeviceMediatedType")
		default:
			deviceLogger().WithFields(logrus.Fields{
				"device-file": deviceFile.Name(),
				"device-type": vfioDeviceType,
			}).Warn("Incorrect token found when detecting large bar space devices")
		}
	}

	return false, nil
}

func isLargeBarSpace(resourcePath string) (bool, error) {
	buf, err := ioutil.ReadFile(resourcePath)
	if err != nil {
		return false, fmt.Errorf("failed to read sysfs resource: %v", err)
	}

	// The resource file contains host addresses of PCI resources:
	// For example:
	// $ cat /sys/bus/pci/devices/0000:04:00.0/resource
	// 0x00000000c6000000 0x00000000c6ffffff 0x0000000000040200
	// 0x0000383800000000 0x0000383bffffffff 0x000000000014220c
	// Refer:
	// resource format: https://github.com/torvalds/linux/blob/63623fd44972d1ed2bfb6e0fb631dfcf547fd1e7/drivers/pci/pci-sysfs.c#L145
	// calculate size : https://github.com/pciutils/pciutils/blob/61ecc14a327de030336f1ff3fea9c7e7e55a90ca/lspci.c#L388
	for rIdx, line := range strings.Split(string(buf), "\n") {
		cols := strings.Fields(line)
		// start and end columns are required to calculate the size
		if len(cols) < 2 {
			deviceLogger().WithField("resource-line", line).Debug("not enough columns to calculate PCI size")
			continue
		}
		start, _ := strconv.ParseUint(cols[0], 0, 64)
		end, _ := strconv.ParseUint(cols[1], 0, 64)
		if start > end {
			deviceLogger().WithFields(logrus.Fields{
				"start": start,
				"end":   end,
			}).Debug("start is greater than end")
			continue
		}
		// Use right shift to convert Bytes to GBytes
		// This is equivalent to ((end - start + 1) / 1024 / 1024 / 1024)
		gbSize := (end - start + 1) >> 30
		deviceLogger().WithFields(logrus.Fields{
			"resource": resourcePath,
			"region":   rIdx,
			"start":    cols[0],
			"end":      cols[1],
			"gb-size":  gbSize,
		}).Debug("Check large bar space device")
		//size is large than 4G
		if gbSize > 4 {
			return true, nil
		}
	}

	return false, nil
}

// isVhostUserBlk checks if the device is a VhostUserBlk device.
func isVhostUserBlk(devInfo config.DeviceInfo) bool {
	return devInfo.DevType == "b" && devInfo.Major == config.VhostUserBlkMajor
}

// isVhostUserSCSI checks if the device is a VhostUserSCSI device.
func isVhostUserSCSI(devInfo config.DeviceInfo) bool {
	return devInfo.DevType == "b" && devInfo.Major == config.VhostUserSCSIMajor
}
