// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persistapi

// ============= sandbox level resources =============

// SetFunc is function hook used for setting sandbox/container state
// It can be registered to dynamically set state files when dump
type SetFunc (func(*SandboxState, map[string]ContainerState) error)

// Bridge is a bridge where devices can be hot plugged
type Bridge struct {
	// DeviceAddr contains information about devices plugged and its address in the bridge
	DeviceAddr map[uint32]string

	// Type is the type of the bridge (pci, pcie, etc)
	Type string

	//ID is used to identify the bridge in the hypervisor
	ID string

	// Addr is the PCI/e slot of the bridge
	Addr int
}

// CPUDevice represents a CPU device which was hot-added in a running VM
type CPUDevice struct {
	// ID is used to identify this CPU in the hypervisor options.
	ID string
}

// HypervisorState saves state of hypervisor
// Refs: virtcontainers/qemu.go:QemuState
type HypervisorState struct {
	Pid     int
	Bridges []Bridge
	// HotpluggedCPUs is the list of CPUs that were hot-added
	HotpluggedVCPUs      []CPUDevice
	HotpluggedMemory     int
	UUID                 string
	HotplugVFIOOnRootBus bool
	BlockIndex           int
}

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
