// Copyright (c) 2024 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

// VfioEndpoint represents a VFIO endpoint which claimed by guest kernel
type VfioEndpoint struct {
	EndpointType       EndpointType
	HostBDF            string
	PCIPath            vcTypes.PciPath
	Iface              NetworkInterface
	EndpointProperties NetworkInfo
}

// Implements Endpoint interface

// Properties returns the properties of the interface.
func (endpoint *VfioEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the interface.
func (endpoint *VfioEndpoint) Name() string {
	return endpoint.Iface.Name
}

// HardwareAddr returns the mac address of the network interface
func (endpoint *VfioEndpoint) HardwareAddr() string {
	return endpoint.Iface.HardAddr
}

// Type indentifies the endpoint as a vfio endpoint.
func (endpoint *VfioEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// PciPath returns the PCI path of the endpoint.
func (endpoint *VfioEndpoint) PciPath() vcTypes.PciPath {
	return endpoint.PCIPath
}

// NetworkPair always return nil
func (endpoint *VfioEndpoint) NetworkPair() *NetworkInterfacePair {
	return nil
}

// SetProperties sets the properties of the endpoint.
func (endpoint *VfioEndpoint) SetProperties(info NetworkInfo) {
	endpoint.EndpointProperties = info
}

// SetPciPath sets the PCI path of the endpoint.
func (endpoint *VfioEndpoint) SetPciPath(path vcTypes.PciPath) {
	endpoint.PCIPath = path
}

// Attach for VFIO endpoint
func (endpoint *VfioEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	return fmt.Errorf("attach is unsupported for VFIO endpoint")
}

// Detach for VFIO endpoint
func (endpoint *VfioEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	return fmt.Errorf("detach is unsupported for VFIO endpoint")
}

func (endpoint *VfioEndpoint) HotAttach(context.Context, *Sandbox) error {
	return fmt.Errorf("VfioEndpoint does not support Hot attach")
}

func (endpoint *VfioEndpoint) HotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string) error {
	return fmt.Errorf("VfioEndpoint does not support Hot detach")
}

func (endpoint *VfioEndpoint) save() persistapi.NetworkEndpoint {
	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),
		Vfio: &persistapi.VfioEndpoint{},
	}
}

func (endpoint *VfioEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = VfioEndpointType

	if s.Vfio != nil {
		endpoint.Iface.Name = s.Vfio.IfaceName
	}
}

func (endpoint *VfioEndpoint) GetRxRateLimiter() bool {
	return false
}

func (endpoint *VfioEndpoint) SetRxRateLimiter() error {
	return fmt.Errorf("rx rate limiter is unsupported for VFIO endpoint")
}

func (endpoint *VfioEndpoint) GetTxRateLimiter() bool {
	return false
}

func (endpoint *VfioEndpoint) SetTxRateLimiter() error {
	return fmt.Errorf("tx rate limiter is unsupported for VFIO endpoint")
}

// Create a VFIO endpoint
func createVfioEndpoint(hostBDF string, netInfo *NetworkInfo) (*VfioEndpoint, error) {
	endpoint := &VfioEndpoint{
		EndpointType: VfioEndpointType,
		HostBDF:      hostBDF,
		Iface: NetworkInterface{
			Name:     netInfo.Iface.Name,
			HardAddr: netInfo.Iface.HardwareAddr.String(),
			Addrs:    netInfo.Addrs,
		},
		EndpointProperties: *netInfo,
	}

	return endpoint, nil
}
