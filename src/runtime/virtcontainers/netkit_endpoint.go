//go:build linux

// Copyright (c) 2025 Datadog, Inc
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

var netkitTrace = getNetworkTrace(NetkitEndpointType)

// NetkitEndpoint gathers a network pair and its properties.
type NetkitEndpoint struct {
	EndpointType       EndpointType
	PCIPath            vcTypes.PciPath
	CCWDevice          *vcTypes.CcwDevice
	EndpointProperties NetworkInfo
	NetPair            NetworkInterfacePair
	RxRateLimiter      bool
	TxRateLimiter      bool
}

func createNetkitNetworkEndpoint(idx int, ifName string, interworkingModel NetInterworkingModel) (*NetkitEndpoint, error) {
	if idx < 0 {
		return &NetkitEndpoint{}, fmt.Errorf("invalid network endpoint index: %d", idx)
	}

	netPair, err := createNetworkInterfacePair(idx, ifName, interworkingModel)
	if err != nil {
		return nil, err
	}

	endpoint := &NetkitEndpoint{
		// TODO This is too specific. We may need to create multiple
		// end point types here and then decide how to connect them
		// at the time of hypervisor attach and not here
		NetPair:      netPair,
		EndpointType: NetkitEndpointType,
	}
	if ifName != "" {
		endpoint.NetPair.VirtIface.Name = ifName
	}

	return endpoint, nil
}

// Properties returns properties for the netkit interface in the network pair.
func (endpoint *NetkitEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the netkit interface in the network pair.
func (endpoint *NetkitEndpoint) Name() string {
	return endpoint.NetPair.VirtIface.Name
}

// HardwareAddr returns the mac address that is assigned to the tap interface
// in th network pair.
func (endpoint *NetkitEndpoint) HardwareAddr() string {
	return endpoint.NetPair.TAPIface.HardAddr
}

// Type identifies the endpoint as a netkit endpoint.
func (endpoint *NetkitEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// PciPath returns the PCI path of the endpoint.
func (endpoint *NetkitEndpoint) PciPath() vcTypes.PciPath {
	return endpoint.PCIPath
}

// SetPciPath sets the PCI path of the endpoint.
func (endpoint *NetkitEndpoint) SetPciPath(pciPath vcTypes.PciPath) {
	endpoint.PCIPath = pciPath
}

// CcwDevice returns the CCW device of the endpoint.
func (endpoint *NetkitEndpoint) CcwDevice() *vcTypes.CcwDevice {
	return endpoint.CCWDevice
}

// SetCcwDevice sets the CCW device of the endpoint.
func (endpoint *NetkitEndpoint) SetCcwDevice(ccwDev vcTypes.CcwDevice) {
	endpoint.CCWDevice = &ccwDev
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *NetkitEndpoint) NetworkPair() *NetworkInterfacePair {
	return &endpoint.NetPair
}

// SetProperties sets the properties for the endpoint.
func (endpoint *NetkitEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// Attach for netkit endpoint bridges the network pair and adds the
// tap interface of the network pair to the hypervisor.
func (endpoint *NetkitEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	span, ctx := netkitTrace(ctx, "Attach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
		networkLogger().WithError(err).Error("Error bridging netkit endpoint")
		return err
	}

	return h.AddDevice(ctx, endpoint, NetDev)
}

// Detach for the netkit endpoint tears down the tap and bridge
// created for the netkit interface.
func (endpoint *NetkitEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	// The network namespace would have been deleted at this point
	// if it has not been created by virtcontainers.
	if !netNsCreated {
		return nil
	}

	span, ctx := netkitTrace(ctx, "Detach", endpoint)
	defer span.End()

	return doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(ctx, endpoint)
	})
}

// HotAttach for the netkit endpoint uses hot plug device
func (endpoint *NetkitEndpoint) HotAttach(ctx context.Context, s *Sandbox) error {
	span, ctx := netkitTrace(ctx, "HotAttach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
		networkLogger().WithError(err).Error("Error bridging netkit ep")
		return err
	}

	if _, err := h.HotplugAddDevice(ctx, endpoint, NetDev); err != nil {
		networkLogger().WithError(err).Error("Error attach netkit ep")
		return err
	}
	return nil
}

// HotDetach for the netkit endpoint uses hot pull device
func (endpoint *NetkitEndpoint) HotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string) error {
	if !netNsCreated {
		return nil
	}

	span, ctx := netkitTrace(ctx, "HotDetach", endpoint)
	defer span.End()

	if err := doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(ctx, endpoint)
	}); err != nil {
		networkLogger().WithError(err).Warn("Error un-bridging netkit ep")
	}

	h := s.hypervisor
	if _, err := h.HotplugRemoveDevice(ctx, endpoint, NetDev); err != nil {
		networkLogger().WithError(err).Error("Error detach netkit ep")
		return err
	}
	return nil
}

func (endpoint *NetkitEndpoint) save() persistapi.NetworkEndpoint {
	netpair := saveNetIfPair(&endpoint.NetPair)

	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),
		Netkit: &persistapi.NetkitEndpoint{
			NetPair: *netpair,
		},
	}
}

func (endpoint *NetkitEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = NetkitEndpointType

	if s.Netkit != nil {
		netpair := loadNetIfPair(&s.Netkit.NetPair)
		endpoint.NetPair = *netpair
	}
}

func (endpoint *NetkitEndpoint) GetRxRateLimiter() bool {
	return endpoint.RxRateLimiter
}

func (endpoint *NetkitEndpoint) SetRxRateLimiter() error {
	endpoint.RxRateLimiter = true
	return nil
}

func (endpoint *NetkitEndpoint) GetTxRateLimiter() bool {
	return endpoint.TxRateLimiter
}

func (endpoint *NetkitEndpoint) SetTxRateLimiter() error {
	endpoint.TxRateLimiter = true
	return nil
}
