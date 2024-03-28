//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"

	"github.com/containernetworking/plugins/pkg/ns"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

var ipvlanTrace = getNetworkTrace(IPVlanEndpointType)

// IPVlanEndpoint represents a ipvlan endpoint that is bridged to the VM
type IPVlanEndpoint struct {
	EndpointType       EndpointType
	PCIPath            vcTypes.PciPath
	EndpointProperties NetworkInfo
	NetPair            NetworkInterfacePair
	RxRateLimiter      bool
	TxRateLimiter      bool
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

// Type identifies the endpoint as a ipvlan endpoint.
func (endpoint *IPVlanEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// SetProperties sets the properties for the endpoint.
func (endpoint *IPVlanEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// PciPath returns the PCI path of the endpoint.
func (endpoint *IPVlanEndpoint) PciPath() vcTypes.PciPath {
	return endpoint.PCIPath
}

// SetPciPath sets the PCI path of the endpoint.
func (endpoint *IPVlanEndpoint) SetPciPath(pciPath vcTypes.PciPath) {
	endpoint.PCIPath = pciPath
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *IPVlanEndpoint) NetworkPair() *NetworkInterfacePair {
	return &endpoint.NetPair
}

// Attach for ipvlan endpoint bridges the network pair and adds the
// tap interface of the network pair to the hypervisor.
func (endpoint *IPVlanEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	span, ctx := ipvlanTrace(ctx, "Attach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
		networkLogger().WithError(err).Error("Error bridging ipvlan ep")
		return err
	}

	return h.AddDevice(ctx, endpoint, NetDev)
}

// Detach for the ipvlan endpoint tears down the tap and bridge
// created for the veth interface.
func (endpoint *IPVlanEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	// The network namespace would have been deleted at this point
	// if it has not been created by virtcontainers.
	if !netNsCreated {
		return nil
	}

	span, ctx := ipvlanTrace(ctx, "Detach", endpoint)
	defer span.End()

	return doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(ctx, endpoint)
	})
}

func (endpoint *IPVlanEndpoint) HotAttach(ctx context.Context, s *Sandbox) error {
	span, ctx := ipvlanTrace(ctx, "HotAttach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
		networkLogger().WithError(err).Error("Error bridging ipvlan ep")
		return err
	}

	if _, err := h.HotplugAddDevice(ctx, endpoint, NetDev); err != nil {
		networkLogger().WithError(err).Error("Error hotplugging ipvlan ep")
		return err
	}

	return nil
}

func (endpoint *IPVlanEndpoint) HotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string) error {
	if !netNsCreated {
		return nil
	}

	span, ctx := ipvlanTrace(ctx, "HotDetach", endpoint)
	defer span.End()

	if err := doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(ctx, endpoint)
	}); err != nil {
		networkLogger().WithError(err).Warn("Error un-bridging ipvlan ep")
	}

	h := s.hypervisor
	if _, err := h.HotplugRemoveDevice(ctx, endpoint, NetDev); err != nil {
		networkLogger().WithError(err).Error("Error detach ipvlan ep")
		return err
	}
	return nil
}

func (endpoint *IPVlanEndpoint) save() persistapi.NetworkEndpoint {
	netpair := saveNetIfPair(&endpoint.NetPair)

	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),
		IPVlan: &persistapi.IPVlanEndpoint{
			NetPair: *netpair,
		},
	}
}

func (endpoint *IPVlanEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = IPVlanEndpointType

	if s.IPVlan != nil {
		netpair := loadNetIfPair(&s.IPVlan.NetPair)
		endpoint.NetPair = *netpair
	}
}

func (endpoint *IPVlanEndpoint) GetRxRateLimiter() bool {
	return endpoint.RxRateLimiter
}

func (endpoint *IPVlanEndpoint) SetRxRateLimiter() error {
	endpoint.RxRateLimiter = true
	return nil
}

func (endpoint *IPVlanEndpoint) GetTxRateLimiter() bool {
	return endpoint.TxRateLimiter
}

func (endpoint *IPVlanEndpoint) SetTxRateLimiter() error {
	endpoint.TxRateLimiter = true
	return nil
}
