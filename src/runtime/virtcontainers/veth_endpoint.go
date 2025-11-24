//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
)

var vethTrace = getNetworkTrace(VethEndpointType)

// VethEndpoint gathers a network pair and its properties.
type VethEndpoint struct {
	NetworkPairEndpointBase
}

func createVethNetworkEndpoint(idx int, ifName string, interworkingModel NetInterworkingModel) (*VethEndpoint, error) {
	base, err := createNetworkPairEndpoint(idx, ifName, interworkingModel, VethEndpointType)
	if err != nil {
		return nil, err
	}

	return &VethEndpoint{
		NetworkPairEndpointBase: *base,
	}, nil
}

// Attach for veth endpoint bridges the network pair and adds the
// tap interface of the network pair to the hypervisor.
func (endpoint *VethEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	return endpoint.attach(ctx, s, endpoint, vethTrace, "virtual")
}

// Detach for the veth endpoint tears down the tap and bridge
// created for the veth interface.
func (endpoint *VethEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	return endpoint.detach(ctx, netNsCreated, netNsPath, endpoint, vethTrace)
}

// HotAttach for the veth endpoint uses hot plug device
func (endpoint *VethEndpoint) HotAttach(ctx context.Context, s *Sandbox) error {
	return endpoint.hotAttach(ctx, s, endpoint, vethTrace, "virtual")
}

// HotDetach for the veth endpoint uses hot pull device
func (endpoint *VethEndpoint) HotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string) error {
	return endpoint.hotDetach(ctx, s, netNsCreated, netNsPath, endpoint, vethTrace, "virtual")
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
		endpoint.loadNetPair(&s.Veth.NetPair)
	}
}
