// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persistapi

import (
	"os"
	"time"
)

// ============= container level resources =============

// DeviceMap saves how host device maps to container device
// one hypervisor device can be
// Refs: virtcontainers/container.go:ContainerDevice
type DeviceMap struct {
	// ID reference to VM device
	ID string

	// ContainerPath is device path displayed in container
	ContainerPath string

	// FileMode permission bits for the device.
	FileMode os.FileMode

	// UID is user ID in the container namespace
	UID uint32

	// GID is group ID in the container namespace
	GID uint32
}

// Mount describes a container mount.
type Mount struct {
	Source      string
	Destination string

	// Type specifies the type of filesystem to mount.
	Type string

	// Options list all the mount options of the filesystem.
	Options []string

	// HostPath used to store host side bind mount path
	HostPath string

	// ReadOnly specifies if the mount should be read only or not
	ReadOnly bool

	// BlockDeviceID represents block device that is attached to the
	// VM in case this mount is a block device file or a directory
	// backed by a block device.
	BlockDeviceID string
}

// RootfsState saves state of container rootfs
type RootfsState struct {
	// BlockDeviceID represents container rootfs block device ID
	// when backed by devicemapper
	BlockDeviceID string

	// RootFStype is file system of the rootfs incase it is block device
	FsType string
}

// Process gathers data related to a container process.
// Refs: virtcontainers/container.go:Process
type Process struct {
	// Token is the process execution context ID. It must be
	// unique per sandbox.
	// Token is used to manipulate processes for containers
	// that have not started yet, and later identify them
	// uniquely within a sandbox.
	Token string

	// Pid is the process ID as seen by the host software
	// stack, e.g. CRI-O, containerd. This is typically the
	// shim PID.
	Pid int

	StartTime time.Time
}

// ContainerState represents container state
type ContainerState struct {
	// State is container running status
	State string

	// Rootfs contains information of container rootfs
	Rootfs RootfsState

	// CgroupPath is the cgroup hierarchy where sandbox's processes
	// including the hypervisor are placed.
	CgroupPath string

	// DeviceMaps is mapping between sandbox device to dest in container
	DeviceMaps []DeviceMap

	// Mounts is mount info from OCI spec
	Mounts []Mount

	// Process on host representing container process
	Process Process

	// BundlePath saves container OCI config.json, which can be unmarshaled
	// and translated to "CompatOCISpec"
	BundlePath string
}
