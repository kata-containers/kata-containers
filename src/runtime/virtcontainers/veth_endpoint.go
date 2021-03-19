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
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"
)

// VethEndpoint gathers a network pair and its properties.
type VethEndpoint struct {
	NetPair            NetworkInterfacePair
	EndpointProperties NetworkInfo
	EndpointType       EndpointType
	PCIPath            vcTypes.PciPath
	RxRateLimiter      bool
	TxRateLimiter      bool
}

func createVethNetworkEndpoint(idx int, ifName string, interworkingModel NetInterworkingModel) (*VethEndpoint, error) {
	if idx < 0 {
		return &VethEndpoint{}, fmt.Errorf("invalid network endpoint index: %d", idx)
	}

	netPair, err := createNetworkInterfacePair(idx, ifName, interworkingModel)
	if err != nil {
		return nil, err
	}

	endpoint := &VethEndpoint{
		// TODO This is too specific. We may need to create multiple
		// end point types here and then decide how to connect them
		// at the time of hypervisor attach and not here
		NetPair:      netPair,
		EndpointType: VethEndpointType,
	}
	if ifName != "" {
		endpoint.NetPair.VirtIface.Name = ifName
	}

	return endpoint, nil
}

// Properties returns properties for the veth interface in the network pair.
func (endpoint *VethEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the veth interface in the network pair.
func (endpoint *VethEndpoint) Name() string {
	return endpoint.NetPair.VirtIface.Name
}

// HardwareAddr returns the mac address that is assigned to the tap interface
// in th network pair.
func (endpoint *VethEndpoint) HardwareAddr() string {
	return endpoint.NetPair.TAPIface.HardAddr
}

// Type identifies the endpoint as a veth endpoint.
func (endpoint *VethEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// PciPath returns the PCI path of the endpoint.
func (endpoint *VethEndpoint) PciPath() vcTypes.PciPath {
	return endpoint.PCIPath
}

// SetPciPath sets the PCI path of the endpoint.
func (endpoint *VethEndpoint) SetPciPath(pciPath vcTypes.PciPath) {
	endpoint.PCIPath = pciPath
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *VethEndpoint) NetworkPair() *NetworkInterfacePair {
	return &endpoint.NetPair
}

// SetProperties sets the properties for the endpoint.
func (endpoint *VethEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// Attach for veth endpoint bridges the network pair and adds the
// tap interface of the network pair to the hypervisor.
func (endpoint *VethEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
		networkLogger().WithError(err).Error("Error bridging virtual endpoint")
		return err
	}

	return h.addDevice(ctx, endpoint, netDev)
}

// Detach for the veth endpoint tears down the tap and bridge
// created for the veth interface.
func (endpoint *VethEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	// The network namespace would have been deleted at this point
	// if it has not been created by virtcontainers.
	if !netNsCreated {
		return nil
	}

	return doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(endpoint)
	})
}

// HotAttach for the veth endpoint uses hot plug device
func (endpoint *VethEndpoint) HotAttach(ctx context.Context, h hypervisor) error {
	if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
		networkLogger().WithError(err).Error("Error bridging virtual ep")
		return err
	}

	if _, err := h.hotplugAddDevice(ctx, endpoint, netDev); err != nil {
		networkLogger().WithError(err).Error("Error attach virtual ep")
		return err
	}
	return nil
}

// HotDetach for the veth endpoint uses hot pull device
func (endpoint *VethEndpoint) HotDetach(ctx context.Context, h hypervisor, netNsCreated bool, netNsPath string) error {
	if !netNsCreated {
		return nil
	}

	if err := doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(endpoint)
	}); err != nil {
		networkLogger().WithError(err).Warn("Error un-bridging virtual ep")
	}

	if _, err := h.hotplugRemoveDevice(ctx, endpoint, netDev); err != nil {
		networkLogger().WithError(err).Error("Error detach virtual ep")
		return err
	}
	return nil
}

func (endpoint *VethEndpoint) save() persistapi.NetworkEndpoint {
	netpair := saveNetIfPair(&endpoint.NetPair)

	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),
		Veth: &persistapi.VethEndpoint{
			NetPair: *netpair,
		},
	}
}

func (endpoint *VethEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = VethEndpointType

	if s.Veth != nil {
		netpair := loadNetIfPair(&s.Veth.NetPair)
		endpoint.NetPair = *netpair
	}
}

func (endpoint *VethEndpoint) GetRxRateLimiter() bool {
	return endpoint.RxRateLimiter
}

func (endpoint *VethEndpoint) SetRxRateLimiter() error {
	endpoint.RxRateLimiter = true
	return nil
}

func (endpoint *VethEndpoint) GetTxRateLimiter() bool {
	return endpoint.TxRateLimiter
}

func (endpoint *VethEndpoint) SetTxRateLimiter() error {
	endpoint.TxRateLimiter = true
	return nil
}
