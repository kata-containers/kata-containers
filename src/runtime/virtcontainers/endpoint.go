// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"
)

// Endpoint represents a physical or virtual network interface.
type Endpoint interface {
	Properties() NetworkInfo
	Name() string
	HardwareAddr() string
	Type() EndpointType
	PciPath() vcTypes.PciPath
	NetworkPair() *NetworkInterfacePair

	SetProperties(NetworkInfo)
	SetPciPath(vcTypes.PciPath)
	Attach(context.Context, *Sandbox) error
	Detach(ctx context.Context, netNsCreated bool, netNsPath string) error
	HotAttach(ctx context.Context, h hypervisor) error
	HotDetach(ctx context.Context, h hypervisor, netNsCreated bool, netNsPath string) error

	save() persistapi.NetworkEndpoint
	load(persistapi.NetworkEndpoint)

	GetRxRateLimiter() bool
	SetRxRateLimiter() error
	GetTxRateLimiter() bool
	SetTxRateLimiter() error
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

	// TuntapEndpointType is a tap network interface.
	TuntapEndpointType EndpointType = "tuntap"

	// IPVlanEndpointType is ipvlan network interface.
	IPVlanEndpointType EndpointType = "ipvlan"
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
	case "tuntap":
		*endpointType = TuntapEndpointType
		return nil
	case "ipvlan":
		*endpointType = IPVlanEndpointType
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
	case TuntapEndpointType:
		return string(TuntapEndpointType)
	case IPVlanEndpointType:
		return string(IPVlanEndpointType)
	default:
		return ""
	}
}

func saveTapIf(tapif *TapInterface) *persistapi.TapInterface {
	if tapif == nil {
		return nil
	}

	return &persistapi.TapInterface{
		ID:   tapif.ID,
		Name: tapif.Name,
		TAPIface: persistapi.NetworkInterface{
			Name:     tapif.TAPIface.Name,
			HardAddr: tapif.TAPIface.HardAddr,
			Addrs:    tapif.TAPIface.Addrs,
		},
	}
}

func loadTapIf(tapif *persistapi.TapInterface) *TapInterface {
	if tapif == nil {
		return nil
	}

	return &TapInterface{
		ID:   tapif.ID,
		Name: tapif.Name,
		TAPIface: NetworkInterface{
			Name:     tapif.TAPIface.Name,
			HardAddr: tapif.TAPIface.HardAddr,
			Addrs:    tapif.TAPIface.Addrs,
		},
	}
}

func saveNetIfPair(pair *NetworkInterfacePair) *persistapi.NetworkInterfacePair {
	if pair == nil {
		return nil
	}

	epVirtIf := pair.VirtIface

	tapif := saveTapIf(&pair.TapInterface)

	virtif := persistapi.NetworkInterface{
		Name:     epVirtIf.Name,
		HardAddr: epVirtIf.HardAddr,
		Addrs:    epVirtIf.Addrs,
	}

	return &persistapi.NetworkInterfacePair{
		TapInterface:         *tapif,
		VirtIface:            virtif,
		NetInterworkingModel: int(pair.NetInterworkingModel),
	}
}

func loadNetIfPair(pair *persistapi.NetworkInterfacePair) *NetworkInterfacePair {
	if pair == nil {
		return nil
	}

	savedVirtIf := pair.VirtIface

	tapif := loadTapIf(&pair.TapInterface)

	virtif := NetworkInterface{
		Name:     savedVirtIf.Name,
		HardAddr: savedVirtIf.HardAddr,
		Addrs:    savedVirtIf.Addrs,
	}

	return &NetworkInterfacePair{
		TapInterface:         *tapif,
		VirtIface:            virtif,
		NetInterworkingModel: NetInterworkingModel(pair.NetInterworkingModel),
	}
}

func saveTuntapIf(tuntapif *TuntapInterface) *persistapi.TuntapInterface {
	if tuntapif == nil {
		return nil
	}

	return &persistapi.TuntapInterface{
		Name: tuntapif.Name,
		TAPIface: persistapi.NetworkInterface{
			Name:     tuntapif.TAPIface.Name,
			HardAddr: tuntapif.TAPIface.HardAddr,
			Addrs:    tuntapif.TAPIface.Addrs,
		},
	}
}

func loadTuntapIf(tuntapif *persistapi.TuntapInterface) *TuntapInterface {
	if tuntapif == nil {
		return nil
	}

	return &TuntapInterface{
		Name: tuntapif.Name,
		TAPIface: NetworkInterface{
			Name:     tuntapif.TAPIface.Name,
			HardAddr: tuntapif.TAPIface.HardAddr,
			Addrs:    tuntapif.TAPIface.Addrs,
		},
	}
}
