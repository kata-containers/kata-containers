//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"os"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

var macvtapTrace = getNetworkTrace(MacvtapEndpointType)

// MacvtapEndpoint represents a macvtap endpoint
type MacvtapEndpoint struct {
	EndpointProperties NetworkInfo
	EndpointType       EndpointType
	VMFds              []*os.File
	VhostFds           []*os.File
	PCIPath            vcTypes.PciPath
	RxRateLimiter      bool
	TxRateLimiter      bool
}

func createMacvtapNetworkEndpoint(netInfo NetworkInfo) (*MacvtapEndpoint, error) {
	endpoint := &MacvtapEndpoint{
		EndpointType:       MacvtapEndpointType,
		EndpointProperties: netInfo,
	}

	return endpoint, nil
}

// Properties returns the properties of the macvtap interface.
func (endpoint *MacvtapEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// HardwareAddr returns the mac address of the macvtap network interface.
func (endpoint *MacvtapEndpoint) HardwareAddr() string {
	return endpoint.EndpointProperties.Iface.HardwareAddr.String()
}

// Name returns name of the macvtap interface.
func (endpoint *MacvtapEndpoint) Name() string {
	return endpoint.EndpointProperties.Iface.Name
}

// Type indentifies the endpoint as a macvtap endpoint.
func (endpoint *MacvtapEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// SetProperties sets the properties of the macvtap endpoint.
func (endpoint *MacvtapEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// Attach for macvtap endpoint passes macvtap device to the hypervisor.
func (endpoint *MacvtapEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	var err error
	span, ctx := macvtapTrace(ctx, "Attach", endpoint)
	defer span.End()

	h := s.hypervisor

	endpoint.VMFds, err = createMacvtapFds(endpoint.EndpointProperties.Iface.Index, int(h.HypervisorConfig().NumVCPUs()))
	if err != nil {
		return fmt.Errorf("Could not setup macvtap fds %s: %s", endpoint.EndpointProperties.Iface.Name, err)
	}

	if !h.HypervisorConfig().DisableVhostNet {
		vhostFds, err := createVhostFds(int(h.HypervisorConfig().NumVCPUs()))
		if err != nil {
			return fmt.Errorf("Could not setup vhost fds %s : %s", endpoint.EndpointProperties.Iface.Name, err)
		}
		endpoint.VhostFds = vhostFds
	}

	return h.AddDevice(ctx, endpoint, NetDev)
}

// Detach for macvtap endpoint does nothing.
func (endpoint *MacvtapEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	return nil
}

// HotAttach for macvtap endpoint not supported yet
func (endpoint *MacvtapEndpoint) HotAttach(ctx context.Context, s *Sandbox) error {
	return fmt.Errorf("MacvtapEndpoint does not support Hot attach")
}

// HotDetach for macvtap endpoint not supported yet
func (endpoint *MacvtapEndpoint) HotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string) error {
	return fmt.Errorf("MacvtapEndpoint does not support Hot detach")
}

// PciPath returns the PCI path of the endpoint.
func (endpoint *MacvtapEndpoint) PciPath() vcTypes.PciPath {
	return endpoint.PCIPath
}

// SetPciPath sets the PCI path of the endpoint.
func (endpoint *MacvtapEndpoint) SetPciPath(pciPath vcTypes.PciPath) {
	endpoint.PCIPath = pciPath
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *MacvtapEndpoint) NetworkPair() *NetworkInterfacePair {
	return nil
}

func (endpoint *MacvtapEndpoint) save() persistapi.NetworkEndpoint {
	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),

		Macvtap: &persistapi.MacvtapEndpoint{
			PCIPath: endpoint.PCIPath,
		},
	}
}
func (endpoint *MacvtapEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = MacvtapEndpointType

	if s.Macvtap != nil {
		endpoint.PCIPath = s.Macvtap.PCIPath
	}
}

func (endpoint *MacvtapEndpoint) GetRxRateLimiter() bool {
	return endpoint.RxRateLimiter
}

func (endpoint *MacvtapEndpoint) SetRxRateLimiter() error {
	endpoint.RxRateLimiter = true
	return nil
}

func (endpoint *MacvtapEndpoint) GetTxRateLimiter() bool {
	return endpoint.TxRateLimiter
}

func (endpoint *MacvtapEndpoint) SetTxRateLimiter() error {
	endpoint.TxRateLimiter = true
	return nil
}
