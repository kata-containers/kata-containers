// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/json"
	"io/ioutil"
	"os"
	"path/filepath"

	directvolume "github.com/kata-containers/directvolume"
)

// getDirectAssignedDiskInfo reads the `file` and unmarshalls it to DiskMountInfo
func getDirectAssignedDiskMountInfo(file string) (directvolume.DiskMountInfo, error) {

	jsonFile, err := os.Open(file)
	if err != nil {
		return directvolume.DiskMountInfo{}, err
	}
	defer jsonFile.Close()

	// read the json file:
	byteValue, err := ioutil.ReadAll(jsonFile)
	if err != nil {
		return directvolume.DiskMountInfo{}, err
	}

	mountInfo := directvolume.DiskMountInfo{}
	if err := json.Unmarshal(byteValue, &mountInfo); err != nil {
		return directvolume.DiskMountInfo{}, err
	}

	return mountInfo, nil
}

// isFileOnSameDeviceAsParent checks if the file resides on the same device as its parent directory.
// This is by getting the device info for the file directory and the parent
// directory of the file directory and comparing their mount point.
// The file would be on the same device as the parent directory if the file device major/minor
// is not the same as the parent directory device major/minor.
func isFileOnSameDeviceAsParent(file string) (bool, error) {
	fileDir := filepath.Dir(file)
	parentDir := filepath.Dir(fileDir)

	fileDeviceMajor := 0
	fileDeviceMinor := 0
	if fileDevice, err := getDeviceForPath(fileDir); err == nil {
		fileDeviceMajor = fileDevice.major
		fileDeviceMinor = fileDevice.minor
	} else {
		if err != errMountPointNotFound {
			return false, err
		}
	}

	parentDeviceMajor := 0
	parentDeviceMinor := 0
	if parentDevice, err := getDeviceForPath(parentDir); err == nil {
		parentDeviceMajor = parentDevice.major
		parentDeviceMinor = parentDevice.minor
	} else {
		if err != errMountPointNotFound {
			return false, err
		}
	}

	return fileDeviceMajor != parentDeviceMajor || fileDeviceMinor != parentDeviceMinor, nil
}
