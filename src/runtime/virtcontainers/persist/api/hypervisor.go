// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persistapi

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

type HypervisorState struct {
	Pid int
	// Type of hypervisor, E.g. qemu/firecracker/acrn.
	Type          string
	BlockIndexMap map[int]struct{}
	UUID          string

	// Belows are qemu specific
	// Refs: virtcontainers/qemu.go:QemuState
	Bridges []Bridge
	// HotpluggedCPUs is the list of CPUs that were hot-added
	HotpluggedVCPUs      []CPUDevice
	HotpluggedMemory     int
	VirtiofsdPid         int
	HotplugVFIOOnRootBus bool
	PCIeRootPort         int

	// clh sepcific: refer to 'virtcontainers/clh.go:CloudHypervisorState'
	APISocket string
}
