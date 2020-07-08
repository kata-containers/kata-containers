// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"os"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
)

// MacvtapEndpoint represents a macvtap endpoint
type MacvtapEndpoint struct {
	EndpointProperties NetworkInfo
	EndpointType       EndpointType
	VMFds              []*os.File
	VhostFds           []*os.File
	PCIAddr            string
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
func (endpoint *MacvtapEndpoint) Attach(s *Sandbox) error {
	var err error
	h := s.hypervisor

	endpoint.VMFds, err = createMacvtapFds(endpoint.EndpointProperties.Iface.Index, int(h.hypervisorConfig().NumVCPUs))
	if err != nil {
		return fmt.Errorf("Could not setup macvtap fds %s: %s", endpoint.EndpointProperties.Iface.Name, err)
	}

	if !h.hypervisorConfig().DisableVhostNet {
		vhostFds, err := createVhostFds(int(h.hypervisorConfig().NumVCPUs))
		if err != nil {
			return fmt.Errorf("Could not setup vhost fds %s : %s", endpoint.EndpointProperties.Iface.Name, err)
		}
		endpoint.VhostFds = vhostFds
	}

	return h.addDevice(endpoint, netDev)
}

// Detach for macvtap endpoint does nothing.
func (endpoint *MacvtapEndpoint) Detach(netNsCreated bool, netNsPath string) error {
	return nil
}

// HotAttach for macvtap endpoint not supported yet
func (endpoint *MacvtapEndpoint) HotAttach(h hypervisor) error {
	return fmt.Errorf("MacvtapEndpoint does not support Hot attach")
}

// HotDetach for macvtap endpoint not supported yet
func (endpoint *MacvtapEndpoint) HotDetach(h hypervisor, netNsCreated bool, netNsPath string) error {
	return fmt.Errorf("MacvtapEndpoint does not support Hot detach")
}

// PciAddr returns the PCI address of the endpoint.
func (endpoint *MacvtapEndpoint) PciAddr() string {
	return endpoint.PCIAddr
}

// SetPciAddr sets the PCI address of the endpoint.
func (endpoint *MacvtapEndpoint) SetPciAddr(pciAddr string) {
	endpoint.PCIAddr = pciAddr
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *MacvtapEndpoint) NetworkPair() *NetworkInterfacePair {
	return nil
}

func (endpoint *MacvtapEndpoint) save() persistapi.NetworkEndpoint {
	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),

		Macvtap: &persistapi.MacvtapEndpoint{
			PCIAddr: endpoint.PCIAddr,
		},
	}
}
func (endpoint *MacvtapEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = MacvtapEndpointType

	if s.Macvtap != nil {
		endpoint.PCIAddr = s.Macvtap.PCIAddr
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
