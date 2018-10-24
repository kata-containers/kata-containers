// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
)

// Endpoint represents a physical or virtual network interface.
type Endpoint interface {
	Properties() NetworkInfo
	Name() string
	HardwareAddr() string
	Type() EndpointType
	PciAddr() string
	NetworkPair() *NetworkInterfacePair

	SetProperties(NetworkInfo)
	SetPciAddr(string)
	Attach(hypervisor) error
	Detach(netNsCreated bool, netNsPath string) error
	HotAttach(h hypervisor) error
	HotDetach(h hypervisor, netNsCreated bool, netNsPath string) error
}

// EndpointType identifies the type of the network endpoint.
type EndpointType string

const (
	// PhysicalEndpointType is the physical network interface.
	PhysicalEndpointType EndpointType = "physical"

	// VethEndpointType is the virtual network interface.
	VethEndpointType EndpointType = "virtual"

	// VhostUserEndpointType is the vhostuser network interface.
	VhostUserEndpointType EndpointType = "vhost-user"

	// BridgedMacvlanEndpointType is macvlan network interface.
	BridgedMacvlanEndpointType EndpointType = "macvlan"

	// MacvtapEndpointType is macvtap network interface.
	MacvtapEndpointType EndpointType = "macvtap"

	// TapEndpointType is tap network interface.
	TapEndpointType EndpointType = "tap"
)

// Set sets an endpoint type based on the input string.
func (endpointType *EndpointType) Set(value string) error {
	switch value {
	case "physical":
		*endpointType = PhysicalEndpointType
		return nil
	case "virtual":
		*endpointType = VethEndpointType
		return nil
	case "vhost-user":
		*endpointType = VhostUserEndpointType
		return nil
	case "macvlan":
		*endpointType = BridgedMacvlanEndpointType
		return nil
	case "macvtap":
		*endpointType = MacvtapEndpointType
		return nil
	case "tap":
		*endpointType = TapEndpointType
		return nil
	default:
		return fmt.Errorf("Unknown endpoint type %s", value)
	}
}

// String converts an endpoint type to a string.
func (endpointType *EndpointType) String() string {
	switch *endpointType {
	case PhysicalEndpointType:
		return string(PhysicalEndpointType)
	case VethEndpointType:
		return string(VethEndpointType)
	case VhostUserEndpointType:
		return string(VhostUserEndpointType)
	case BridgedMacvlanEndpointType:
		return string(BridgedMacvlanEndpointType)
	case MacvtapEndpointType:
		return string(MacvtapEndpointType)
	case TapEndpointType:
		return string(TapEndpointType)
	default:
		return ""
	}
}
