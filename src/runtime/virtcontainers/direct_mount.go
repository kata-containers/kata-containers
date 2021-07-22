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

const SandboxInfoJSON = "sandboxInfo.json"

type SandboxInfo struct {
	SandboxID string `json:"id,omitempty"`
}

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

// writeSandboxInfo writes out the SandboxInfoJSON file at the given path.
// The file for now just contains the sandbox ID.
func writeSandboxInfo(path, id string) error {
	sandboxInfoPath := filepath.Join(path, SandboxInfoJSON)
	sandboxInfo := SandboxInfo{
		SandboxID: id,
	}
	byteValue, err := json.Marshal(sandboxInfo)
	if err != nil {
		return err
	}
	if err = ioutil.WriteFile(sandboxInfoPath, byteValue, 0644); err != nil {
		return err
	}
	return nil
}
