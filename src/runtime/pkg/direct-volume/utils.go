// Copyright (c) 2022 Databricks Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package volume

import (
	b64 "encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

const (
	mountInfoFileName = "mountInfo.json"

	FSGroupMetadataKey             = "fsGroup"
	FSGroupChangePolicyMetadataKey = "fsGroupChangePolicy"
)

// FSGroupChangePolicy holds policies that will be used for applying fsGroup to a volume.
// This type and the allowed values are tracking the PodFSGroupChangePolicy defined in
// https://github.com/kubernetes/kubernetes/blob/master/staging/src/k8s.io/api/core/v1/types.go
// It is up to the client using the direct-assigned volume feature (e.g. CSI drivers) to determine
// the optimal setting for this change policy (i.e. from Pod spec or assuming volume ownership
// based on the storage offering).
type FSGroupChangePolicy string

const (
	// FSGroupChangeAlways indicates that volume's ownership should always be changed.
	FSGroupChangeAlways FSGroupChangePolicy = "Always"
	// FSGroupChangeOnRootMismatch indicates that volume's ownership will be changed
	// only when ownership of root directory does not match with the desired group id.
	FSGroupChangeOnRootMismatch FSGroupChangePolicy = "OnRootMismatch"
)

var kataDirectVolumeRootPath = "/run/kata-containers/shared/direct-volumes"

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
		return fmt.Errorf("%s should be a directory", volumeDir)
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

// VolumeMountInfo retrieves the mount info of a direct volume.
func VolumeMountInfo(volumePath string) (*MountInfo, error) {
	mountInfoFilePath := filepath.Join(kataDirectVolumeRootPath, b64.URLEncoding.EncodeToString([]byte(volumePath)), mountInfoFileName)
	if _, err := os.Stat(mountInfoFilePath); err != nil {
		return nil, err
	}
	buf, err := os.ReadFile(mountInfoFilePath)
	if err != nil {
		return nil, err
	}
	var mountInfo MountInfo
	if err := json.Unmarshal(buf, &mountInfo); err != nil {
		return nil, err
	}
	return &mountInfo, nil
}

// RecordSandboxId associates a sandbox id with a direct volume.
func RecordSandboxId(sandboxId string, volumePath string) error {
	encodedPath := b64.URLEncoding.EncodeToString([]byte(volumePath))
	mountInfoFilePath := filepath.Join(kataDirectVolumeRootPath, encodedPath, mountInfoFileName)
	if _, err := os.Stat(mountInfoFilePath); err != nil {
		return err
	}

	return os.WriteFile(filepath.Join(kataDirectVolumeRootPath, encodedPath, sandboxId), []byte(""), 0600)
}

func GetSandboxIdForVolume(volumePath string) (string, error) {
	files, err := os.ReadDir(filepath.Join(kataDirectVolumeRootPath, b64.URLEncoding.EncodeToString([]byte(volumePath))))
	if err != nil {
		return "", err
	}
	// Find the id of the first sandbox.
	// We expect a direct-assigned volume is associated with only a sandbox at a time.
	for _, file := range files {
		if file.Name() != mountInfoFileName {
			return file.Name(), nil
		}
	}
	return "", fmt.Errorf("no sandbox found for %s", volumePath)
}
