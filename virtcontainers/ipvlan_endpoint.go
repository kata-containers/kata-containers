// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"

	"github.com/containernetworking/plugins/pkg/ns"
)

// IPVlanEndpoint represents a ipvlan endpoint that is bridged to the VM
type IPVlanEndpoint struct {
	NetPair            NetworkInterfacePair
	EndpointProperties NetworkInfo
	EndpointType       EndpointType
	PCIAddr            string
}

func createIPVlanNetworkEndpoint(idx int, ifName string) (*IPVlanEndpoint, error) {
	if idx < 0 {
		return &IPVlanEndpoint{}, fmt.Errorf("invalid network endpoint index: %d", idx)
	}

	// Use tc filtering for ipvlan, since the other inter networking models will
	// not work for ipvlan.
	interworkingModel := NetXConnectTCFilterModel
	netPair, err := createNetworkInterfacePair(idx, ifName, interworkingModel)
	if err != nil {
		return nil, err
	}

	endpoint := &IPVlanEndpoint{
		NetPair:      netPair,
		EndpointType: IPVlanEndpointType,
	}
	if ifName != "" {
		endpoint.NetPair.VirtIface.Name = ifName
	}

	return endpoint, nil
}

// Properties returns properties of the interface.
func (endpoint *IPVlanEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the veth interface in the network pair.
func (endpoint *IPVlanEndpoint) Name() string {
	return endpoint.NetPair.VirtIface.Name
}

// HardwareAddr returns the mac address that is assigned to the tap interface
// in th network pair.
func (endpoint *IPVlanEndpoint) HardwareAddr() string {
	return endpoint.NetPair.TAPIface.HardAddr
}

// Type identifies the endpoint as a virtual endpoint.
func (endpoint *IPVlanEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// SetProperties sets the properties for the endpoint.
func (endpoint *IPVlanEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// PciAddr returns the PCI address of the endpoint.
func (endpoint *IPVlanEndpoint) PciAddr() string {
	return endpoint.PCIAddr
}

// SetPciAddr sets the PCI address of the endpoint.
func (endpoint *IPVlanEndpoint) SetPciAddr(pciAddr string) {
	endpoint.PCIAddr = pciAddr
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *IPVlanEndpoint) NetworkPair() *NetworkInterfacePair {
	return &endpoint.NetPair
}

// Attach for virtual endpoint bridges the network pair and adds the
// tap interface of the network pair to the hypervisor.
func (endpoint *IPVlanEndpoint) Attach(h hypervisor) error {
	if err := xconnectVMNetwork(endpoint, true, h.hypervisorConfig().NumVCPUs, h.hypervisorConfig().DisableVhostNet); err != nil {
		networkLogger().WithError(err).Error("Error bridging virtual ep")
		return err
	}

	return h.addDevice(endpoint, netDev)
}

// Detach for the virtual endpoint tears down the tap and bridge
// created for the veth interface.
func (endpoint *IPVlanEndpoint) Detach(netNsCreated bool, netNsPath string) error {
	// The network namespace would have been deleted at this point
	// if it has not been created by virtcontainers.
	if !netNsCreated {
		return nil
	}

	return doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xconnectVMNetwork(endpoint, false, 0, false)
	})
}

// HotAttach for physical endpoint not supported yet
func (endpoint *IPVlanEndpoint) HotAttach(h hypervisor) error {
	return fmt.Errorf("IPVlanEndpoint does not support Hot attach")
}

// HotDetach for physical endpoint not supported yet
func (endpoint *IPVlanEndpoint) HotDetach(h hypervisor, netNsCreated bool, netNsPath string) error {
	return fmt.Errorf("IPVlanEndpoint does not support Hot detach")
}
