// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

const DirectAssignedVolumeJson = "csiPlugin.json"

type DiskMountInfo struct {
	// Device: source device for this volume
	Device string `json:"device,omitempty"`

	// VolumeType: type associated with this volume (ie, block)
	VolumeType string `json:"volumeType,omitempty"`

	// TargetPath: path which this device should be mounted within the guest
	TargetPath string `json:"targetPath,omitempty"`

	// FsType: filesystem that needs to be used to mount the storage inside the VM
	FsType string `json:"fsType,omitempty"`

	// Options: additional options that might be needed to mount the storage filesystem
	Options string `json:options,omitempty"`
}
