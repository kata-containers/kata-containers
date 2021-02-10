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

// BridgedMacvlanEndpoint represents a macvlan endpoint that is bridged to the VM
type BridgedMacvlanEndpoint struct {
	NetPair            NetworkInterfacePair
	EndpointProperties NetworkInfo
	EndpointType       EndpointType
	PCIPath            vcTypes.PciPath
	RxRateLimiter      bool
	TxRateLimiter      bool
}

func createBridgedMacvlanNetworkEndpoint(idx int, ifName string, interworkingModel NetInterworkingModel) (*BridgedMacvlanEndpoint, error) {
	if idx < 0 {
		return &BridgedMacvlanEndpoint{}, fmt.Errorf("invalid network endpoint index: %d", idx)
	}

	netPair, err := createNetworkInterfacePair(idx, ifName, interworkingModel)
	if err != nil {
		return nil, err
	}

	endpoint := &BridgedMacvlanEndpoint{
		NetPair:      netPair,
		EndpointType: BridgedMacvlanEndpointType,
	}
	if ifName != "" {
		endpoint.NetPair.VirtIface.Name = ifName
	}

	return endpoint, nil
}

// Properties returns properties of the interface.
func (endpoint *BridgedMacvlanEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the veth interface in the network pair.
func (endpoint *BridgedMacvlanEndpoint) Name() string {
	return endpoint.NetPair.VirtIface.Name
}

// HardwareAddr returns the mac address that is assigned to the tap interface
// in th network pair.
func (endpoint *BridgedMacvlanEndpoint) HardwareAddr() string {
	return endpoint.NetPair.TAPIface.HardAddr
}

// Type identifies the endpoint as a virtual endpoint.
func (endpoint *BridgedMacvlanEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// SetProperties sets the properties for the endpoint.
func (endpoint *BridgedMacvlanEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// PciPath returns the PCI path of the endpoint.
func (endpoint *BridgedMacvlanEndpoint) PciPath() vcTypes.PciPath {
	return endpoint.PCIPath
}

// SetPciPath sets the PCI path of the endpoint.
func (endpoint *BridgedMacvlanEndpoint) SetPciPath(pciPath vcTypes.PciPath) {
	endpoint.PCIPath = pciPath
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *BridgedMacvlanEndpoint) NetworkPair() *NetworkInterfacePair {
	return &endpoint.NetPair
}

// Attach for virtual endpoint bridges the network pair and adds the
// tap interface of the network pair to the hypervisor.
func (endpoint *BridgedMacvlanEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
		networkLogger().WithError(err).Error("Error bridging virtual ep")
		return err
	}

	return h.addDevice(ctx, endpoint, netDev)
}

// Detach for the virtual endpoint tears down the tap and bridge
// created for the veth interface.
func (endpoint *BridgedMacvlanEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	// The network namespace would have been deleted at this point
	// if it has not been created by virtcontainers.
	if !netNsCreated {
		return nil
	}

	return doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(endpoint)
	})
}

// HotAttach for physical endpoint not supported yet
func (endpoint *BridgedMacvlanEndpoint) HotAttach(ctx context.Context, h hypervisor) error {
	return fmt.Errorf("BridgedMacvlanEndpoint does not support Hot attach")
}

// HotDetach for physical endpoint not supported yet
func (endpoint *BridgedMacvlanEndpoint) HotDetach(ctx context.Context, h hypervisor, netNsCreated bool, netNsPath string) error {
	return fmt.Errorf("BridgedMacvlanEndpoint does not support Hot detach")
}

func (endpoint *BridgedMacvlanEndpoint) save() persistapi.NetworkEndpoint {
	netpair := saveNetIfPair(&endpoint.NetPair)

	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),
		BridgedMacvlan: &persistapi.BridgedMacvlanEndpoint{
			NetPair: *netpair,
		},
	}
}

func (endpoint *BridgedMacvlanEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = BridgedMacvlanEndpointType

	if s.BridgedMacvlan != nil {
		netpair := loadNetIfPair(&s.BridgedMacvlan.NetPair)
		endpoint.NetPair = *netpair
	}
}

func (endpoint *BridgedMacvlanEndpoint) GetRxRateLimiter() bool {
	return endpoint.RxRateLimiter
}

func (endpoint *BridgedMacvlanEndpoint) SetRxRateLimiter() error {
	endpoint.RxRateLimiter = true
	return nil
}

func (endpoint *BridgedMacvlanEndpoint) GetTxRateLimiter() bool {
	return endpoint.TxRateLimiter
}

func (endpoint *BridgedMacvlanEndpoint) SetTxRateLimiter() error {
	endpoint.TxRateLimiter = true
	return nil
}
