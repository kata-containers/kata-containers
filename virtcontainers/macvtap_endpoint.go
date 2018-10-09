// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"os"
)

// MacvtapEndpoint represents a macvtap endpoint
type MacvtapEndpoint struct {
	EndpointProperties NetworkInfo
	EndpointType       EndpointType
	VMFds              []*os.File
	VhostFds           []*os.File
	PCIAddr            string
}

func createMacvtapNetworkEndpoint(netInfo NetworkInfo) (*MacvtapEndpoint, error) {
	endpoint := &MacvtapEndpoint{
		EndpointType:       MacvtapEndpointType,
		EndpointProperties: netInfo,
	}

	return endpoint, nil
}

// Properties returns the properties of the macvtap interface.
func (endpoint *MacvtapEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// HardwareAddr returns the mac address of the macvtap network interface.
func (endpoint *MacvtapEndpoint) HardwareAddr() string {
	return endpoint.EndpointProperties.Iface.HardwareAddr.String()
}

// Name returns name of the macvtap interface.
func (endpoint *MacvtapEndpoint) Name() string {
	return endpoint.EndpointProperties.Iface.Name
}

// Type indentifies the endpoint as a macvtap endpoint.
func (endpoint *MacvtapEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// SetProperties sets the properties of the macvtap endpoint.
func (endpoint *MacvtapEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// Attach for macvtap endpoint passes macvtap device to the hypervisor.
func (endpoint *MacvtapEndpoint) Attach(h hypervisor) error {
	networkLogger().WithField("endpoint-type", "macvtap").Info("Attaching endpoint")
	var err error

	endpoint.VMFds, err = createMacvtapFds(endpoint.EndpointProperties.Iface.Index, int(h.hypervisorConfig().NumVCPUs))
	if err != nil {
		return fmt.Errorf("Could not setup macvtap fds %s: %s", endpoint.EndpointProperties.Iface.Name, err)
	}

	if !h.hypervisorConfig().DisableVhostNet {
		vhostFds, err := createVhostFds(int(h.hypervisorConfig().NumVCPUs))
		if err != nil {
			return fmt.Errorf("Could not setup vhost fds %s : %s", endpoint.EndpointProperties.Iface.Name, err)
		}
		endpoint.VhostFds = vhostFds
	}

	return h.addDevice(endpoint, netDev)
}

// Detach for macvtap endpoint does nothing.
func (endpoint *MacvtapEndpoint) Detach(netNsCreated bool, netNsPath string) error {
	networkLogger().WithField("endpoint-type", "macvtap").Info("Detaching endpoint")
	return nil
}

// HotAttach for macvtap endpoint not supported yet
func (endpoint *MacvtapEndpoint) HotAttach(h hypervisor) error {
	return fmt.Errorf("MacvtapEndpoint does not support Hot attach")
}

// HotDetach for macvtap endpoint not supported yet
func (endpoint *MacvtapEndpoint) HotDetach(h hypervisor, netNsCreated bool, netNsPath string) error {
	return fmt.Errorf("MacvtapEndpoint does not support Hot detach")
}

// PciAddr returns the PCI address of the endpoint.
func (endpoint *MacvtapEndpoint) PciAddr() string {
	return endpoint.PCIAddr
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *MacvtapEndpoint) NetworkPair() *NetworkInterfacePair {
	return nil
}
