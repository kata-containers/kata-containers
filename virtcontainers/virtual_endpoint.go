// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"

	"github.com/containernetworking/plugins/pkg/ns"
)

// VirtualEndpoint gathers a network pair and its properties.
type VirtualEndpoint struct {
	NetPair            NetworkInterfacePair
	EndpointProperties NetworkInfo
	Physical           bool
	EndpointType       EndpointType
	PCIAddr            string
}

func createVirtualNetworkEndpoint(idx int, ifName string, interworkingModel NetInterworkingModel) (*VirtualEndpoint, error) {
	if idx < 0 {
		return &VirtualEndpoint{}, fmt.Errorf("invalid network endpoint index: %d", idx)
	}

	netPair, err := createNetworkInterfacePair(idx, ifName, interworkingModel)
	if err != nil {
		return nil, err
	}

	endpoint := &VirtualEndpoint{
		// TODO This is too specific. We may need to create multiple
		// end point types here and then decide how to connect them
		// at the time of hypervisor attach and not here
		NetPair:      netPair,
		EndpointType: VirtualEndpointType,
	}
	if ifName != "" {
		endpoint.NetPair.VirtIface.Name = ifName
	}

	return endpoint, nil
}

// Properties returns properties for the veth interface in the network pair.
func (endpoint *VirtualEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the veth interface in the network pair.
func (endpoint *VirtualEndpoint) Name() string {
	return endpoint.NetPair.VirtIface.Name
}

// HardwareAddr returns the mac address that is assigned to the tap interface
// in th network pair.
func (endpoint *VirtualEndpoint) HardwareAddr() string {
	return endpoint.NetPair.TAPIface.HardAddr
}

// Type identifies the endpoint as a virtual endpoint.
func (endpoint *VirtualEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// PciAddr returns the PCI address of the endpoint.
func (endpoint *VirtualEndpoint) PciAddr() string {
	return endpoint.PCIAddr
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *VirtualEndpoint) NetworkPair() *NetworkInterfacePair {
	return &endpoint.NetPair
}

// SetProperties sets the properties for the endpoint.
func (endpoint *VirtualEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// Attach for virtual endpoint bridges the network pair and adds the
// tap interface of the network pair to the hypervisor.
func (endpoint *VirtualEndpoint) Attach(h hypervisor) error {
	networkLogger().WithField("endpoint-type", "virtual").Info("Attaching endpoint")

	if err := xconnectVMNetwork(endpoint, true, h.hypervisorConfig().NumVCPUs, h.hypervisorConfig().DisableVhostNet); err != nil {
		networkLogger().WithError(err).Error("Error bridging virtual endpoint")
		return err
	}

	return h.addDevice(endpoint, netDev)
}

// Detach for the virtual endpoint tears down the tap and bridge
// created for the veth interface.
func (endpoint *VirtualEndpoint) Detach(netNsCreated bool, netNsPath string) error {
	// The network namespace would have been deleted at this point
	// if it has not been created by virtcontainers.
	if !netNsCreated {
		return nil
	}

	networkLogger().WithField("endpoint-type", "virtual").Info("Detaching endpoint")

	return doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xconnectVMNetwork(endpoint, false, 0, false)
	})
}

// HotAttach for the virtual endpoint uses hot plug device
func (endpoint *VirtualEndpoint) HotAttach(h hypervisor) error {
	networkLogger().Info("Hot attaching virtual endpoint")
	if err := xconnectVMNetwork(endpoint, true, h.hypervisorConfig().NumVCPUs, h.hypervisorConfig().DisableVhostNet); err != nil {
		networkLogger().WithError(err).Error("Error bridging virtual ep")
		return err
	}

	if _, err := h.hotplugAddDevice(endpoint, netDev); err != nil {
		networkLogger().WithError(err).Error("Error attach virtual ep")
		return err
	}
	return nil
}

// HotDetach for the virtual endpoint uses hot pull device
func (endpoint *VirtualEndpoint) HotDetach(h hypervisor, netNsCreated bool, netNsPath string) error {
	if !netNsCreated {
		return nil
	}
	networkLogger().Info("Hot detaching virtual endpoint")
	if err := doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xconnectVMNetwork(endpoint, false, 0, h.hypervisorConfig().DisableVhostNet)
	}); err != nil {
		networkLogger().WithError(err).Warn("Error un-bridging virtual ep")
	}

	if _, err := h.hotplugRemoveDevice(endpoint, netDev); err != nil {
		networkLogger().WithError(err).Error("Error detach virtual ep")
		return err
	}
	return nil
}
