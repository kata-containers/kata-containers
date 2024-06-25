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

var macvlanTrace = getNetworkTrace(MacvlanEndpointType)

// MacvlanEndpoint represents a macvlan endpoint that is bridged to the VM
type MacvlanEndpoint struct {
	EndpointType       EndpointType
	PCIPath            vcTypes.PciPath
	EndpointProperties NetworkInfo
	NetPair            NetworkInterfacePair
	RxRateLimiter      bool
	TxRateLimiter      bool
}

func createMacvlanNetworkEndpoint(idx int, ifName string, interworkingModel NetInterworkingModel) (*MacvlanEndpoint, error) {
	if idx < 0 {
		return &MacvlanEndpoint{}, fmt.Errorf("invalid network endpoint index: %d", idx)
	}

	netPair, err := createNetworkInterfacePair(idx, ifName, interworkingModel)
	if err != nil {
		return nil, err
	}

	endpoint := &MacvlanEndpoint{
		NetPair:      netPair,
		EndpointType: MacvlanEndpointType,
	}
	if ifName != "" {
		endpoint.NetPair.VirtIface.Name = ifName
	}

	return endpoint, nil
}

// Properties returns properties of the interface.
func (endpoint *MacvlanEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the veth interface in the network pair.
func (endpoint *MacvlanEndpoint) Name() string {
	return endpoint.NetPair.VirtIface.Name
}

// HardwareAddr returns the mac address that is assigned to the tap interface
// in th network pair.
func (endpoint *MacvlanEndpoint) HardwareAddr() string {
	return endpoint.NetPair.TAPIface.HardAddr
}

// Type identifies the endpoint as a bridged macvlan endpoint.
func (endpoint *MacvlanEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// SetProperties sets the properties for the endpoint.
func (endpoint *MacvlanEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// PciPath returns the PCI path of the endpoint.
func (endpoint *MacvlanEndpoint) PciPath() vcTypes.PciPath {
	return endpoint.PCIPath
}

// SetPciPath sets the PCI path of the endpoint.
func (endpoint *MacvlanEndpoint) SetPciPath(pciPath vcTypes.PciPath) {
	endpoint.PCIPath = pciPath
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *MacvlanEndpoint) NetworkPair() *NetworkInterfacePair {
	return &endpoint.NetPair
}

// Attach for virtual endpoint bridges the network pair and adds the
// tap interface of the network pair to the hypervisor.
func (endpoint *MacvlanEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	span, ctx := macvlanTrace(ctx, "Attach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
		networkLogger().WithError(err).Error("Error bridging bridged macvlan ep")
		return err
	}

	return h.AddDevice(ctx, endpoint, NetDev)
}

// Detach for the virtual endpoint tears down the tap and bridge
// created for the veth interface.
func (endpoint *MacvlanEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	// The network namespace would have been deleted at this point
	// if it has not been created by virtcontainers.
	if !netNsCreated {
		return nil
	}

	span, ctx := macvlanTrace(ctx, "Detach", endpoint)
	defer span.End()

	return doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(ctx, endpoint)
	})
}

func (endpoint *MacvlanEndpoint) HotAttach(ctx context.Context, s *Sandbox) error {
	span, ctx := macvlanTrace(ctx, "HotAttach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
		networkLogger().WithError(err).Error("Error bridging macvlan ep")
		return err
	}

	if _, err := h.HotplugAddDevice(ctx, endpoint, NetDev); err != nil {
		networkLogger().WithError(err).Error("Error hotplugging macvlan ep")
		return err
	}

	return nil
}

func (endpoint *MacvlanEndpoint) HotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string) error {
	if !netNsCreated {
		return nil
	}

	span, ctx := macvlanTrace(ctx, "HotDetach", endpoint)
	defer span.End()

	if err := doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(ctx, endpoint)
	}); err != nil {
		networkLogger().WithError(err).Warn("Error un-bridging macvlan ep")
	}

	h := s.hypervisor
	if _, err := h.HotplugRemoveDevice(ctx, endpoint, NetDev); err != nil {
		networkLogger().WithError(err).Error("Error detach macvlan ep")
		return err
	}
	return nil
}

func (endpoint *MacvlanEndpoint) save() persistapi.NetworkEndpoint {
	netpair := saveNetIfPair(&endpoint.NetPair)

	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),
		Macvlan: &persistapi.MacvlanEndpoint{
			NetPair: *netpair,
		},
	}
}

func (endpoint *MacvlanEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = MacvlanEndpointType

	if s.Macvlan != nil {
		netpair := loadNetIfPair(&s.Macvlan.NetPair)
		endpoint.NetPair = *netpair
	}
}

func (endpoint *MacvlanEndpoint) GetRxRateLimiter() bool {
	return endpoint.RxRateLimiter
}

func (endpoint *MacvlanEndpoint) SetRxRateLimiter() error {
	endpoint.RxRateLimiter = true
	return nil
}

func (endpoint *MacvlanEndpoint) GetTxRateLimiter() bool {
	return endpoint.TxRateLimiter
}

func (endpoint *MacvlanEndpoint) SetTxRateLimiter() error {
	endpoint.TxRateLimiter = true
	return nil
}
