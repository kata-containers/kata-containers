// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package hypervisors

import "fmt"

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

// PCIePort distinguish only between root and switch port
type PCIePort string

const (
	// RootPort attach VFIO devices to a root-port
	RootPort PCIePort = "root-port"
	// SwitchPort attach VFIO devices to a switch-port
	SwitchPort = "switch-port"
	// BridgePort is the default
	BridgePort = "bridge-port"
	// NoPort is for disabling VFIO hotplug/coldplug
	NoPort = "no-port"
)

func (p PCIePort) String() string {
	switch p {
	case RootPort:
		return "root-port"
	case SwitchPort:
		return "switch-port"
	case BridgePort:
		return "bridge-port"
	case NoPort:
		return "no-port"
	}
	return fmt.Sprintf("<unknown PCIePort: %s>", string(p))
}

type HypervisorState struct {
	BlockIndexMap map[int]struct{}

	// Type of hypervisor, E.g. qemu/firecracker/acrn.
	Type string
	UUID string
	// clh sepcific: refer to 'virtcontainers/clh.go:CloudHypervisorState'
	APISocket string

	// Belows are qemu specific
	// Refs: virtcontainers/qemu.go:QemuState
	Bridges []Bridge
	// HotpluggedCPUs is the list of CPUs that were hot-added
	HotpluggedVCPUs []CPUDevice

	HotpluggedMemory     int
	VirtiofsDaemonPid    int
	Pid                  int
	PCIeRootPort         int
	ColdPlugVFIO         PCIePort
	HotplugVFIOOnRootBus bool
}
