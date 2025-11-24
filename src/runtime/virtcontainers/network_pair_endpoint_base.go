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
	"go.opentelemetry.io/otel/trace"
)

// traceFunc is the type for endpoint-specific trace functions
type traceFunc func(context.Context, string, any) (trace.Span, context.Context)

// NetworkPairEndpointBase contains the common implementation for
// network pair-based endpoints (veth, netkit).
type NetworkPairEndpointBase struct {
	EndpointType       EndpointType
	PCIPath            vcTypes.PciPath
	CCWDevice          *vcTypes.CcwDevice
	EndpointProperties NetworkInfo
	NetPair            NetworkInterfacePair
	RxRateLimiter      bool
	TxRateLimiter      bool
}

// createNetworkPairEndpoint creates a network pair endpoint with the given type
func createNetworkPairEndpoint(idx int, ifName string, interworkingModel NetInterworkingModel, endpointType EndpointType) (*NetworkPairEndpointBase, error) {
	if idx < 0 {
		return nil, fmt.Errorf("invalid network endpoint index: %d", idx)
	}

	netPair, err := createNetworkInterfacePair(idx, ifName, interworkingModel)
	if err != nil {
		return nil, err
	}

	endpoint := &NetworkPairEndpointBase{
		// TODO This is too specific. We may need to create multiple
		// end point types here and then decide how to connect them
		// at the time of hypervisor attach and not here
		NetPair:      netPair,
		EndpointType: endpointType,
	}
	if ifName != "" {
		endpoint.NetPair.VirtIface.Name = ifName
	}

	return endpoint, nil
}

// Properties returns properties for the interface in the network pair.
func (endpoint *NetworkPairEndpointBase) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the interface in the network pair.
func (endpoint *NetworkPairEndpointBase) Name() string {
	return endpoint.NetPair.VirtIface.Name
}

// HardwareAddr returns the mac address that is assigned to the tap interface
// in the network pair.
func (endpoint *NetworkPairEndpointBase) HardwareAddr() string {
	return endpoint.NetPair.TAPIface.HardAddr
}

// Type identifies the endpoint type.
func (endpoint *NetworkPairEndpointBase) Type() EndpointType {
	return endpoint.EndpointType
}

// PciPath returns the PCI path of the endpoint.
func (endpoint *NetworkPairEndpointBase) PciPath() vcTypes.PciPath {
	return endpoint.PCIPath
}

// SetPciPath sets the PCI path of the endpoint.
func (endpoint *NetworkPairEndpointBase) SetPciPath(pciPath vcTypes.PciPath) {
	endpoint.PCIPath = pciPath
}

// CcwDevice returns the CCW device of the endpoint.
func (endpoint *NetworkPairEndpointBase) CcwDevice() *vcTypes.CcwDevice {
	return endpoint.CCWDevice
}

// SetCcwDevice sets the CCW device of the endpoint.
func (endpoint *NetworkPairEndpointBase) SetCcwDevice(ccwDev vcTypes.CcwDevice) {
	endpoint.CCWDevice = &ccwDev
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *NetworkPairEndpointBase) NetworkPair() *NetworkInterfacePair {
	return &endpoint.NetPair
}

// SetProperties sets the properties for the endpoint.
func (endpoint *NetworkPairEndpointBase) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// GetRxRateLimiter returns the RX rate limiter status.
func (endpoint *NetworkPairEndpointBase) GetRxRateLimiter() bool {
	return endpoint.RxRateLimiter
}

// SetRxRateLimiter sets the RX rate limiter.
func (endpoint *NetworkPairEndpointBase) SetRxRateLimiter() error {
	endpoint.RxRateLimiter = true
	return nil
}

// GetTxRateLimiter returns the TX rate limiter status.
func (endpoint *NetworkPairEndpointBase) GetTxRateLimiter() bool {
	return endpoint.TxRateLimiter
}

// SetTxRateLimiter sets the TX rate limiter.
func (endpoint *NetworkPairEndpointBase) SetTxRateLimiter() error {
	endpoint.TxRateLimiter = true
	return nil
}

// attach implements the common attach logic for network pair endpoints.
func (endpoint *NetworkPairEndpointBase) attach(ctx context.Context, s *Sandbox, e Endpoint, traceFn traceFunc, logPrefix string) error {
	span, ctx := traceFn(ctx, "Attach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, e, h); err != nil {
		networkLogger().WithError(err).Errorf("Error bridging %s endpoint", logPrefix)
		return err
	}

	return h.AddDevice(ctx, e, NetDev)
}

// detach implements the common detach logic for network pair endpoints.
func (endpoint *NetworkPairEndpointBase) detach(ctx context.Context, netNsCreated bool, netNsPath string, e Endpoint, traceFn traceFunc) error {
	// The network namespace would have been deleted at this point
	// if it has not been created by virtcontainers.
	if !netNsCreated {
		return nil
	}

	span, ctx := traceFn(ctx, "Detach", endpoint)
	defer span.End()

	return doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(ctx, e)
	})
}

// hotAttach implements the common hot attach logic for network pair endpoints.
func (endpoint *NetworkPairEndpointBase) hotAttach(ctx context.Context, s *Sandbox, e Endpoint, traceFn traceFunc, logPrefix string) error {
	span, ctx := traceFn(ctx, "HotAttach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, e, h); err != nil {
		networkLogger().WithError(err).Errorf("Error bridging %s ep", logPrefix)
		return err
	}

	if _, err := h.HotplugAddDevice(ctx, e, NetDev); err != nil {
		networkLogger().WithError(err).Errorf("Error attach %s ep", logPrefix)
		return err
	}
	return nil
}

// hotDetach implements the common hot detach logic for network pair endpoints.
func (endpoint *NetworkPairEndpointBase) hotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string, e Endpoint, traceFn traceFunc, logPrefix string) error {
	if !netNsCreated {
		return nil
	}

	span, ctx := traceFn(ctx, "HotDetach", endpoint)
	defer span.End()

	if err := doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xDisconnectVMNetwork(ctx, e)
	}); err != nil {
		networkLogger().WithError(err).Warnf("Error un-bridging %s ep", logPrefix)
	}

	h := s.hypervisor
	if _, err := h.HotplugRemoveDevice(ctx, e, NetDev); err != nil {
		networkLogger().WithError(err).Errorf("Error detach %s ep", logPrefix)
		return err
	}
	return nil
}

// loadNetPair is a helper to load the network pair from persistence.
func (endpoint *NetworkPairEndpointBase) loadNetPair(netpair *persistapi.NetworkInterfacePair) {
	if netpair != nil {
		loaded := loadNetIfPair(netpair)
		endpoint.NetPair = *loaded
	}
}
