// Copyright (c) 2023 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"

	"github.com/vishvananda/netlink"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
)

var endpointNotSupported error = errors.New("Unsupported endpoint on Darwin")

// DarwinNetwork represents a sandbox networking setup.
type DarwinNetwork struct {
	networkID         string
	interworkingModel NetInterworkingModel
	networkCreated    bool
	eps               []Endpoint
}

func NewNetwork(configs ...*NetworkConfig) (Network, error) {
	if len(configs) > 1 {
		return nil, errors.New("too many network configurations")
	}

	// Empty constructor
	if len(configs) == 0 {
		return &DarwinNetwork{}, nil
	}

	config := configs[0]
	if config == nil {
		return nil, errors.New("missing network configuration")
	}

	return &DarwinNetwork{
		config.NetworkID,
		config.InterworkingModel,
		config.NetworkCreated,
		[]Endpoint{},
	}, nil
}

func LoadNetwork(netInfo persistapi.NetworkInfo) Network {
	network := DarwinNetwork{
		networkID:      netInfo.NetworkID,
		networkCreated: netInfo.NetworkCreated,
	}

	return &network
}

func (n *DarwinNetwork) AddEndpoints(context.Context, *Sandbox, []NetworkInfo, bool) ([]Endpoint, error) {
	return nil, endpointNotSupported
}

func (n *DarwinNetwork) RemoveEndpoints(context.Context, *Sandbox, []Endpoint, bool) error {
	return endpointNotSupported
}

func (n *DarwinNetwork) Run(context.Context, func() error) error {
	return nil
}

func (n *DarwinNetwork) NetworkID() string {
	return n.networkID
}

func (n *DarwinNetwork) NetworkCreated() bool {
	return n.networkCreated
}

func (n *DarwinNetwork) NetMonitorThread() int {
	return 0
}

func (n *DarwinNetwork) SetNetMonitorThread(pid int) {
	return
}

func (n *DarwinNetwork) Endpoints() []Endpoint {
	return n.eps
}

func (n *DarwinNetwork) SetEndpoints(endpoints []Endpoint) {
	n.eps = endpoints
}

func (n *DarwinNetwork) GetEndpointsNum() (int, error) {
	return 0, nil
}

func validGuestRoute(route netlink.Route) bool {
	return true
}

func validGuestNeighbor(route netlink.Neigh) bool {
	return true
}
