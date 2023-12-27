// Copyright (c) 2022 Databricks Inc.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
package utils

import (
	b64 "encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

const (
	mountInfoFileName        = "mountInfo.json"
	kataDirectVolumeRootPath = "/run/kata-containers/shared/direct-volumes"
)

// MountInfo contains the information needed by Kata to consume a host block device and mount it as a filesystem inside the guest VM.
type MountInfo struct {
	// The type of the volume (ie. block)
	VolumeType string `json:"volume-type"`
	// The device backing the volume.
	Device string `json:"device"`
	// The filesystem type to be mounted on the volume.
	FsType string `json:"fstype"`
	// Additional metadata to pass to the agent regarding this volume.
	Metadata map[string]string `json:"metadata,omitempty"`
	// Additional mount options.
	Options []string `json:"options,omitempty"`
}

// Add writes the mount info of a direct volume into a filesystem path known to Kata Container.
func Add(volumePath string, mountInfo string) error {
	volumeDir := filepath.Join(kataDirectVolumeRootPath, b64.URLEncoding.EncodeToString([]byte(volumePath)))
	stat, err := os.Stat(volumeDir)
	if err != nil {
		if !errors.Is(err, os.ErrNotExist) {
			return err
		}
		if err := os.MkdirAll(volumeDir, 0700); err != nil {
			return err
		}
	}
	if stat != nil && !stat.IsDir() {
		return status.Error(codes.Unknown, fmt.Sprintf("%s should be a directory", volumeDir))
	}

	var deserialized MountInfo
	if err := json.Unmarshal([]byte(mountInfo), &deserialized); err != nil {
		return err
	}

	return os.WriteFile(filepath.Join(volumeDir, mountInfoFileName), []byte(mountInfo), 0600)
}

// Remove deletes the direct volume path including all the files inside it.
func Remove(volumePath string) error {
	return os.RemoveAll(filepath.Join(kataDirectVolumeRootPath, b64.URLEncoding.EncodeToString([]byte(volumePath))))
}
