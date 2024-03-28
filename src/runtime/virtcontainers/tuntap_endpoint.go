//go:build linux

// Copyright (c) 2018 Huawei Corporation
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"net"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/vishvananda/netlink"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

var tuntapTrace = getNetworkTrace(TuntapEndpointType)

// TuntapEndpoint represents just a tap endpoint
type TuntapEndpoint struct {
	EndpointType       EndpointType
	PCIPath            vcTypes.PciPath
	TuntapInterface    TuntapInterface
	EndpointProperties NetworkInfo
	NetPair            NetworkInterfacePair
	RxRateLimiter      bool
	TxRateLimiter      bool
}

// Properties returns the properties of the tun/tap interface.
func (endpoint *TuntapEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the tap interface in the network pair.
func (endpoint *TuntapEndpoint) Name() string {
	return endpoint.TuntapInterface.Name
}

// HardwareAddr returns the mac address that is assigned to the tun/tap interface
func (endpoint *TuntapEndpoint) HardwareAddr() string {
	return endpoint.TuntapInterface.TAPIface.HardAddr
}

// Type identifies the endpoint as a tun/tap endpoint.
func (endpoint *TuntapEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// PciPath returns the PCI path of the endpoint.
func (endpoint *TuntapEndpoint) PciPath() vcTypes.PciPath {
	return endpoint.PCIPath
}

// SetPciPath sets the PCI path of the endpoint.
func (endpoint *TuntapEndpoint) SetPciPath(pciPath vcTypes.PciPath) {
	endpoint.PCIPath = pciPath
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *TuntapEndpoint) NetworkPair() *NetworkInterfacePair {
	return &endpoint.NetPair
}

// SetProperties sets the properties for the endpoint.
func (endpoint *TuntapEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// Attach for tun/tap endpoint adds the tap interface to the hypervisor.
func (endpoint *TuntapEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	span, ctx := tuntapTrace(ctx, "Attach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
		networkLogger().WithError(err).Error("Error bridging virtual endpoint")
		return err
	}

	return h.AddDevice(ctx, endpoint, NetDev)
}

// Detach for the tun/tap endpoint tears down the tap
func (endpoint *TuntapEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	if !netNsCreated && netNsPath != "" {
		return nil
	}

	span, _ := tuntapTrace(ctx, "Detach", endpoint)
	defer span.End()

	networkLogger().WithField("endpoint-type", TuntapEndpointType).Info("Detaching endpoint")
	return doNetNS(netNsPath, func(_ ns.NetNS) error {
		return unTuntapNetwork(endpoint.TuntapInterface.TAPIface.Name)
	})
}

// HotAttach for the tun/tap endpoint uses hot plug device
func (endpoint *TuntapEndpoint) HotAttach(ctx context.Context, s *Sandbox) error {
	networkLogger().Info("Hot attaching tun/tap endpoint")

	span, ctx := tuntapTrace(ctx, "HotAttach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := tuntapNetwork(endpoint, h.HypervisorConfig().NumVCPUs(), h.HypervisorConfig().DisableVhostNet); err != nil {
		networkLogger().WithError(err).Error("Error bridging tun/tap ep")
		return err
	}

	if _, err := h.HotplugAddDevice(ctx, endpoint, NetDev); err != nil {
		networkLogger().WithError(err).Error("Error attach tun/tap ep")
		return err
	}
	return nil
}

// HotDetach for the tun/tap endpoint uses hot pull device
func (endpoint *TuntapEndpoint) HotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string) error {
	networkLogger().Info("Hot detaching tun/tap endpoint")

	span, ctx := tuntapTrace(ctx, "HotDetach", endpoint)
	defer span.End()

	if err := doNetNS(netNsPath, func(_ ns.NetNS) error {
		return unTuntapNetwork(endpoint.TuntapInterface.TAPIface.Name)
	}); err != nil {
		networkLogger().WithError(err).Warn("Error un-bridging tun/tap ep")
	}

	h := s.hypervisor
	if _, err := h.HotplugRemoveDevice(ctx, endpoint, NetDev); err != nil {
		networkLogger().WithError(err).Error("Error detach tun/tap ep")
		return err
	}
	return nil
}

func createTuntapNetworkEndpoint(idx int, ifName string, hwName net.HardwareAddr, internetworkingModel NetInterworkingModel) (*TuntapEndpoint, error) {
	if idx < 0 {
		return &TuntapEndpoint{}, fmt.Errorf("invalid network endpoint index: %d", idx)
	}

	netPair, err := createNetworkInterfacePair(idx, ifName, internetworkingModel)
	if err != nil {
		return nil, err
	}

	endpoint := &TuntapEndpoint{
		NetPair: netPair,
		TuntapInterface: TuntapInterface{
			Name: fmt.Sprintf("eth%d", idx),
			TAPIface: NetworkInterface{
				Name:     fmt.Sprintf("tap%d_kata", idx),
				HardAddr: fmt.Sprintf("%s", hwName), //nolint:gosimple
			},
		},
		EndpointType: TuntapEndpointType,
	}

	if ifName != "" {
		endpoint.TuntapInterface.Name = ifName
	}

	return endpoint, nil
}

func tuntapNetwork(endpoint *TuntapEndpoint, numCPUs uint32, disableVhostNet bool) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Close()

	tapLink, _, err := createLink(netHandle, endpoint.TuntapInterface.TAPIface.Name, &netlink.Tuntap{}, int(numCPUs))
	if err != nil {
		return fmt.Errorf("Could not create TAP interface: %s", err)
	}
	linkAttrs := endpoint.Properties().Iface.LinkAttrs

	// Save the MAC address to the TAP so that it can later be used
	// to build the QMP command line. This MAC address has to be
	// the one inside the VM in order to avoid any firewall issues. The
	// bridge created by the network plugin on the host actually expects
	// to see traffic from this MAC address and not another one.
	endpoint.TuntapInterface.TAPIface.HardAddr = linkAttrs.HardwareAddr.String()
	if err := netHandle.LinkSetMTU(tapLink, linkAttrs.MTU); err != nil {
		return fmt.Errorf("Could not set TAP MTU %d: %s", linkAttrs.MTU, err)
	}
	if err := netHandle.LinkSetUp(tapLink); err != nil {
		return fmt.Errorf("Could not enable TAP %s: %s", endpoint.TuntapInterface.Name, err)
	}
	return nil
}

func unTuntapNetwork(name string) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Close()
	tapLink, err := getLinkByName(netHandle, name, &netlink.Tuntap{})
	if err != nil {
		return fmt.Errorf("Could not get TAP interface: %s", err)
	}
	if err := netHandle.LinkSetDown(tapLink); err != nil {
		return fmt.Errorf("Could not disable TAP %s: %s", name, err)
	}
	if err := netHandle.LinkDel(tapLink); err != nil {
		return fmt.Errorf("Could not remove TAP %s: %s", name, err)
	}
	return nil

}
func (endpoint *TuntapEndpoint) save() persistapi.NetworkEndpoint {
	tuntapif := saveTuntapIf(&endpoint.TuntapInterface)

	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),
		Tuntap: &persistapi.TuntapEndpoint{
			TuntapInterface: *tuntapif,
		},
	}
}

func (endpoint *TuntapEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = TuntapEndpointType

	if s.Tuntap != nil {
		tuntapif := loadTuntapIf(&s.Tuntap.TuntapInterface)
		endpoint.TuntapInterface = *tuntapif
	}
}

func (endpoint *TuntapEndpoint) GetRxRateLimiter() bool {
	return endpoint.RxRateLimiter
}

func (endpoint *TuntapEndpoint) SetRxRateLimiter() error {
	endpoint.RxRateLimiter = true
	return nil
}

func (endpoint *TuntapEndpoint) GetTxRateLimiter() bool {
	return endpoint.TxRateLimiter
}

func (endpoint *TuntapEndpoint) SetTxRateLimiter() error {
	endpoint.TxRateLimiter = true
	return nil
}
