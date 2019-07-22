// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persistapi

// ============= sandbox level resources =============

// ProxyState save proxy state data
type ProxyState struct {
	// Pid of proxy process
	Pid int

	// URL to connect to proxy
	URL string
}

// SandboxState contains state information of sandbox
// nolint: maligned
type SandboxState struct {
	// PersistVersion of persist data format, can be used for keeping compatibility later
	PersistVersion uint

	// State is sandbox running status
	State string

	// GuestMemoryBlockSizeMB is the size of memory block of guestos
	GuestMemoryBlockSizeMB uint32

	// GuestMemoryHotplugProbe determines whether guest kernel supports memory hotplug probe interface
	GuestMemoryHotplugProbe bool

	// SandboxContainer specifies which container is used to start the sandbox/vm
	SandboxContainer string

	// CgroupPath is the cgroup hierarchy where sandbox's processes
	// including the hypervisor are placed.
	// FIXME: sandbox can reuse "SandboxContainer"'s CgroupPath so we can remove this field.
	CgroupPath string

	// Devices plugged to sandbox(hypervisor)
	Devices []DeviceState

	// HypervisorState saves hypervisor specific data
	HypervisorState HypervisorState

	// ProxyState saves state data of proxy process
	ProxyState ProxyState

	// Network saves network configuration of sandbox
	Network NetworkInfo

	// Config saves config information of sandbox
	Config SandboxConfig
}
