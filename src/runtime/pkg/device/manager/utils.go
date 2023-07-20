// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package manager

import (
	"path/filepath"
	"strings"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
)

const (
	vfioPath = "/dev/vfio/"
)

// IsVFIOControlDevice checks if the device provided is a vfio control device.
// Depending no the vfio_mode we need to know if a device is a VFIO device
// or the VFIO control device
func IsVFIOControlDevice(path string) bool {
	return path == filepath.Join(vfioPath, "vfio")
}

// IsVFIO checks if the device provided is a vfio group.
func IsVFIODevice(hostPath string) bool {
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

// isVhostUserBlk checks if the device is a VhostUserBlk device.
func IsVhostUserBlk(devInfo config.DeviceInfo) bool {
	return devInfo.DevType == "b" && devInfo.Major == config.VhostUserBlkMajor
}

// isVhostUserSCSI checks if the device is a VhostUserSCSI device.
func isVhostUserSCSI(devInfo config.DeviceInfo) bool {
	return devInfo.DevType == "b" && devInfo.Major == config.VhostUserSCSIMajor
}
