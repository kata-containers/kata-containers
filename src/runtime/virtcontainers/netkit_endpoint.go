//go:build linux

// Copyright (c) 2025 Datadog, Inc
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
)

var netkitTrace = getNetworkTrace(NetkitEndpointType)

// NetkitEndpoint gathers a network pair and its properties.
type NetkitEndpoint struct {
	NetworkPairEndpointBase
}

func createNetkitNetworkEndpoint(idx int, ifName string, interworkingModel NetInterworkingModel) (*NetkitEndpoint, error) {
	base, err := createNetworkPairEndpoint(idx, ifName, interworkingModel, NetkitEndpointType)
	if err != nil {
		return nil, err
	}

	return &NetkitEndpoint{
		NetworkPairEndpointBase: *base,
	}, nil
}

// Attach for netkit endpoint bridges the network pair and adds the
// tap interface of the network pair to the hypervisor.
func (endpoint *NetkitEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	return endpoint.attach(ctx, s, endpoint, netkitTrace, "netkit")
}

// Detach for the netkit endpoint tears down the tap and bridge
// created for the netkit interface.
func (endpoint *NetkitEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	return endpoint.detach(ctx, netNsCreated, netNsPath, endpoint, netkitTrace)
}

// HotAttach for the netkit endpoint uses hot plug device
func (endpoint *NetkitEndpoint) HotAttach(ctx context.Context, s *Sandbox) error {
	return endpoint.hotAttach(ctx, s, endpoint, netkitTrace, "netkit")
}

// HotDetach for the netkit endpoint uses hot pull device
func (endpoint *NetkitEndpoint) HotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string) error {
	return endpoint.hotDetach(ctx, s, netNsCreated, netNsPath, endpoint, netkitTrace, "netkit")
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
		endpoint.loadNetPair(&s.Netkit.NetPair)
	}
}
