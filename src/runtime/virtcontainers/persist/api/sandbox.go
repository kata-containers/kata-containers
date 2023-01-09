// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persistapi

import (
	dev "github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	hv "github.com/kata-containers/kata-containers/src/runtime/pkg/hypervisors"
)

// ============= sandbox level resources =============

// AgentState save agent state data
type AgentState struct {
	// URL to connect to agent
	URL string
}

// SandboxState contains state information of sandbox
// nolint: maligned
type SandboxState struct {
	// CgroupPath is the cgroup hierarchy where sandbox's processes
	// including the hypervisor are placed.
	CgroupPaths map[string]string

	// Devices plugged to sandbox(hypervisor)
	Devices []dev.DeviceState

	// State is sandbox running status
	State string

	// SandboxContainer specifies which container is used to start the sandbox/vm
	SandboxContainer string

	// SandboxCgroupPath is the sandbox cgroup path
	SandboxCgroupPath string

	// OverheadCgroupPath is the sandbox overhead cgroup path.
	// It can be an empty string if sandbox_cgroup_only is set.
	OverheadCgroupPath string

	// HypervisorState saves hypervisor specific data
	HypervisorState hv.HypervisorState

	// AgentState saves state data of agent
	AgentState AgentState

	// Network saves network configuration of sandbox
	Network NetworkInfo

	// Config saves config information of sandbox
	Config SandboxConfig

	// PersistVersion of persist data format, can be used for keeping compatibility later
	PersistVersion uint

	// GuestMemoryBlockSizeMB is the size of memory block of guestos
	GuestMemoryBlockSizeMB uint32

	// GuestMemoryHotplugProbe determines whether guest kernel supports memory hotplug probe interface
	GuestMemoryHotplugProbe bool
}
