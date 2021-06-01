// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persistapi

// ============= sandbox level resources =============

// AgentState save agent state data
type AgentState struct {
	// URL to connect to agent
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
	CgroupPath string

	// CgroupPath is the cgroup hierarchy where sandbox's processes
	// including the hypervisor are placed.
	CgroupPaths map[string]string

	// Devices plugged to sandbox(hypervisor)
	Devices []DeviceState

	// HypervisorState saves hypervisor specific data
	HypervisorState HypervisorState

	// AgentState saves state data of agent
	AgentState AgentState

	// Network saves network configuration of sandbox
	Network NetworkInfo

	// Config saves config information of sandbox
	Config SandboxConfig
}
