//go:build linux

// Copyright (c) 2025 contributors to the VirtContainers for Go project
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

var vlanTrace = getNetworkTrace(VlanEndpointType)

// VlanEndpoint represents a vlan endpoint that is bridged to the VM
type VlanEndpoint struct {
	EndpointType       EndpointType
	PCIPath            vcTypes.PciPath
	CCWDevice          *vcTypes.CcwDevice
	EndpointProperties NetworkInfo
	NetPair            NetworkInterfacePair
	RxRateLimiter      bool
	TxRateLimiter      bool
}

func createVlanNetworkEndpoint(idx int, ifName string) (*VlanEndpoint, error) {
	if idx < 0 {
		return &VlanEndpoint{}, fmt.Errorf("invalid network endpoint index: %d", idx)
	}

	// Use tc filtering for vlan, since the other inter networking models will
	// not work for vlan.
	interworkingModel := NetXConnectTCFilterModel
	netPair, err := createNetworkInterfacePair(idx, ifName, interworkingModel)
	if err != nil {
		return nil, err
	}

	endpoint := &VlanEndpoint{
		NetPair:      netPair,
		EndpointType: VlanEndpointType,
	}
	if ifName != "" {
		endpoint.NetPair.VirtIface.Name = ifName
	}

	return endpoint, nil
}

// Properties returns properties of the interface.
func (endpoint *VlanEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the veth interface in the network pair.
func (endpoint *VlanEndpoint) Name() string {
	return endpoint.NetPair.VirtIface.Name
}

// HardwareAddr returns the mac address that is assigned to the tap interface
// in th network pair.
func (endpoint *VlanEndpoint) HardwareAddr() string {
	return endpoint.NetPair.TAPIface.HardAddr
}

// Type identifies the endpoint as a vlan endpoint.
func (endpoint *VlanEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// SetProperties sets the properties for the endpoint.
func (endpoint *VlanEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// PciPath returns the PCI path of the endpoint.
func (endpoint *VlanEndpoint) PciPath() vcTypes.PciPath {
	return endpoint.PCIPath
}

// SetPciPath sets the PCI path of the endpoint.
func (endpoint *VlanEndpoint) SetPciPath(pciPath vcTypes.PciPath) {
	endpoint.PCIPath = pciPath
}

// CcwDevice returns the CCW device of the endpoint.
func (endpoint *VlanEndpoint) CcwDevice() *vcTypes.CcwDevice {
	return endpoint.CCWDevice
}

// SetCcwDevice sets the CCW device of the endpoint.
func (endpoint *VlanEndpoint) SetCcwDevice(ccwDev vcTypes.CcwDevice) {
	endpoint.CCWDevice = &ccwDev
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *VlanEndpoint) NetworkPair() *NetworkInterfacePair {
	return &endpoint.NetPair
}

// Attach for vlan endpoint bridges the network pair and adds the
// tap interface of the network pair to the hypervisor.
func (endpoint *VlanEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	span, ctx := vlanTrace(ctx, "Attach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
		networkLogger().WithError(err).Error("Error bridging vlan ep")
		return err
	}

	return h.AddDevice(ctx, endpoint, NetDev)
}

// Detach for the vlan endpoint tears down the tap and bridge
// created for the veth interface.
func (endpoint *VlanEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	// The network namespace would have been deleted at this point
	// if it has not been created by virtcontainers.
	if !netNsCreated {
		return nil
	}

	span, ctx := vlanTrace(ctx, "Detach", endpoint)
	defer span.End()

	return doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(ctx, endpoint)
	})
}

func (endpoint *VlanEndpoint) HotAttach(ctx context.Context, s *Sandbox) error {
	span, ctx := vlanTrace(ctx, "HotAttach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
		networkLogger().WithError(err).Error("Error bridging vlan ep")
		return err
	}

	if _, err := h.HotplugAddDevice(ctx, endpoint, NetDev); err != nil {
		networkLogger().WithError(err).Error("Error hotplugging vlan ep")
		return err
	}

	return nil
}

func (endpoint *VlanEndpoint) HotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string) error {
	if !netNsCreated {
		return nil
	}

	span, ctx := vlanTrace(ctx, "HotDetach", endpoint)
	defer span.End()

	if err := doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(ctx, endpoint)
	}); err != nil {
		networkLogger().WithError(err).Warn("Error un-bridging vlan ep")
	}

	h := s.hypervisor
	if _, err := h.HotplugRemoveDevice(ctx, endpoint, NetDev); err != nil {
		networkLogger().WithError(err).Error("Error detach vlan ep")
		return err
	}
	return nil
}

func (endpoint *VlanEndpoint) save() persistapi.NetworkEndpoint {
	netpair := saveNetIfPair(&endpoint.NetPair)

	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),
		Vlan: &persistapi.VlanEndpoint{
			NetPair: *netpair,
		},
	}
}

func (endpoint *VlanEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = VlanEndpointType

	if s.Vlan != nil {
		netpair := loadNetIfPair(&s.Vlan.NetPair)
		endpoint.NetPair = *netpair
	}
}

func (endpoint *VlanEndpoint) GetRxRateLimiter() bool {
	return endpoint.RxRateLimiter
}

func (endpoint *VlanEndpoint) SetRxRateLimiter() error {
	endpoint.RxRateLimiter = true
	return nil
}

func (endpoint *VlanEndpoint) GetTxRateLimiter() bool {
	return endpoint.TxRateLimiter
}

func (endpoint *VlanEndpoint) SetTxRateLimiter() error {
	endpoint.TxRateLimiter = true
	return nil
}
