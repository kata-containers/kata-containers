// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/hex"
	"fmt"
	"os"

	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/utils"
)

// Long term, this should be made more configurable.  For now matching path
// provided by CNM VPP and OVS-DPDK plugins, available at github.com/clearcontainers/vpp and
// github.com/clearcontainers/ovsdpdk.  The plugins create the socket on the host system
// using this path.
const hostSocketSearchPath = "/tmp/vhostuser_%s/vhu.sock"

// VhostUserEndpoint represents a vhost-user socket based network interface
type VhostUserEndpoint struct {
	// Path to the vhost-user socket on the host system
	SocketPath string
	// MAC address of the interface
	HardAddr           string
	IfaceName          string
	EndpointProperties NetworkInfo
	EndpointType       EndpointType
	PCIAddr            string
}

// Properties returns the properties of the interface.
func (endpoint *VhostUserEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the interface.
func (endpoint *VhostUserEndpoint) Name() string {
	return endpoint.IfaceName
}

// HardwareAddr returns the mac address of the vhostuser network interface
func (endpoint *VhostUserEndpoint) HardwareAddr() string {
	return endpoint.HardAddr
}

// Type indentifies the endpoint as a vhostuser endpoint.
func (endpoint *VhostUserEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// SetProperties sets the properties of the endpoint.
func (endpoint *VhostUserEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// PciAddr returns the PCI address of the endpoint.
func (endpoint *VhostUserEndpoint) PciAddr() string {
	return endpoint.PCIAddr
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *VhostUserEndpoint) NetworkPair() *NetworkInterfacePair {
	return nil
}

// Attach for vhostuser endpoint
func (endpoint *VhostUserEndpoint) Attach(h hypervisor) error {
	networkLogger().WithField("endpoint-type", "vhostuser").Info("Attaching endpoint")

	// Generate a unique ID to be used for hypervisor commandline fields
	randBytes, err := utils.GenerateRandomBytes(8)
	if err != nil {
		return err
	}
	id := hex.EncodeToString(randBytes)

	d := config.VhostUserDeviceAttrs{
		DevID:      id,
		SocketPath: endpoint.SocketPath,
		MacAddress: endpoint.HardAddr,
		Type:       config.VhostUserNet,
	}

	return h.addDevice(d, vhostuserDev)
}

// Detach for vhostuser endpoint
func (endpoint *VhostUserEndpoint) Detach(netNsCreated bool, netNsPath string) error {
	networkLogger().WithField("endpoint-type", "vhostuser").Info("Detaching endpoint")
	return nil
}

// HotAttach for vhostuser endpoint not supported yet
func (endpoint *VhostUserEndpoint) HotAttach(h hypervisor) error {
	return fmt.Errorf("VhostUserEndpoint does not support Hot attach")
}

// HotDetach for vhostuser endpoint not supported yet
func (endpoint *VhostUserEndpoint) HotDetach(h hypervisor, netNsCreated bool, netNsPath string) error {
	return fmt.Errorf("VhostUserEndpoint does not support Hot detach")
}

// Create a vhostuser endpoint
func createVhostUserEndpoint(netInfo NetworkInfo, socket string) (*VhostUserEndpoint, error) {

	vhostUserEndpoint := &VhostUserEndpoint{
		SocketPath:   socket,
		HardAddr:     netInfo.Iface.HardwareAddr.String(),
		IfaceName:    netInfo.Iface.Name,
		EndpointType: VhostUserEndpointType,
	}
	return vhostUserEndpoint, nil
}

// findVhostUserNetSocketPath checks if an interface is a dummy placeholder
// for a vhost-user socket, and if it is it returns the path to the socket
func findVhostUserNetSocketPath(netInfo NetworkInfo) (string, error) {
	if netInfo.Iface.Name == "lo" {
		return "", nil
	}

	// check for socket file existence at known location.
	for _, addr := range netInfo.Addrs {
		socketPath := fmt.Sprintf(hostSocketSearchPath, addr.IPNet.IP)
		if _, err := os.Stat(socketPath); err == nil {
			return socketPath, nil
		}
	}

	return "", nil
}

// vhostUserSocketPath returns the path of the socket discovered.  This discovery
// will vary depending on the type of vhost-user socket.
//  Today only VhostUserNetDevice is supported.
func vhostUserSocketPath(info interface{}) (string, error) {

	switch v := info.(type) {
	case NetworkInfo:
		return findVhostUserNetSocketPath(v)
	default:
		return "", nil
	}

}
