// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/json"
	"io/ioutil"
	"os"

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
