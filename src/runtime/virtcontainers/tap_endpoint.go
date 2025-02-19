//go:build linux

// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/vishvananda/netlink"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/uuid"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

var tapTrace = getNetworkTrace(TapEndpointType)

// TapEndpoint represents just a tap endpoint
type TapEndpoint struct {
	TapInterface       TapInterface
	EndpointProperties NetworkInfo
	EndpointType       EndpointType
	PCIPath            vcTypes.PciPath
	RxRateLimiter      bool
	TxRateLimiter      bool
}

// Properties returns the properties of the tap interface.
func (endpoint *TapEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the tap interface in the network pair.
func (endpoint *TapEndpoint) Name() string {
	return endpoint.TapInterface.Name
}

// HardwareAddr returns the mac address that is assigned to the tap interface
func (endpoint *TapEndpoint) HardwareAddr() string {
	return endpoint.TapInterface.TAPIface.HardAddr
}

// Type identifies the endpoint as a tap endpoint.
func (endpoint *TapEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// PciPath returns the PCI path of the endpoint.
func (endpoint *TapEndpoint) PciPath() vcTypes.PciPath {
	return endpoint.PCIPath
}

// SetPciPath sets the PCI path of the endpoint.
func (endpoint *TapEndpoint) SetPciPath(pciPath vcTypes.PciPath) {
	endpoint.PCIPath = pciPath
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *TapEndpoint) NetworkPair() *NetworkInterfacePair {
	return nil
}

// SetProperties sets the properties for the endpoint.
func (endpoint *TapEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// Attach for tap endpoint adds the tap interface to the hypervisor.
func (endpoint *TapEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	return fmt.Errorf("TapEndpoint does not support Attach, if you're using docker please use --net none")
}

// Detach for the tap endpoint tears down the tap
func (endpoint *TapEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	if !netNsCreated && netNsPath != "" {
		return nil
	}

	span, _ := tapTrace(ctx, "Detach", endpoint)
	defer span.End()

	networkLogger().WithField("endpoint-type", TapEndpointType).Info("Detaching endpoint")
	return doNetNS(netNsPath, func(_ ns.NetNS) error {
		return unTapNetwork(endpoint.TapInterface.TAPIface.Name)
	})
}

// HotAttach for the tap endpoint uses hot plug device
func (endpoint *TapEndpoint) HotAttach(ctx context.Context, s *Sandbox) error {
	networkLogger().Info("Hot attaching tap endpoint")

	span, ctx := tapTrace(ctx, "HotAttach", endpoint)
	defer span.End()

	h := s.hypervisor
	if err := tapNetwork(endpoint, h.HypervisorConfig().NumVCPUs(), h.HypervisorConfig().DisableVhostNet); err != nil {
		networkLogger().WithError(err).Error("Error bridging tap ep")
		return err
	}

	if _, err := h.HotplugAddDevice(ctx, endpoint, NetDev); err != nil {
		networkLogger().WithError(err).Error("Error attach tap ep")
		return err
	}
	return nil
}

// HotDetach for the tap endpoint uses hot pull device
func (endpoint *TapEndpoint) HotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string) error {
	networkLogger().Info("Hot detaching tap endpoint")

	span, ctx := tapTrace(ctx, "HotDetach", endpoint)
	defer span.End()

	if err := doNetNS(netNsPath, func(_ ns.NetNS) error {
		return unTapNetwork(endpoint.TapInterface.TAPIface.Name)
	}); err != nil {
		networkLogger().WithError(err).Warn("Error un-bridging tap ep")
	}

	h := s.hypervisor
	if _, err := h.HotplugRemoveDevice(ctx, endpoint, NetDev); err != nil {
		networkLogger().WithError(err).Error("Error detach tap ep")
		return err
	}
	return nil
}

func createTapNetworkEndpoint(idx int, ifName string) (*TapEndpoint, error) {
	if idx < 0 {
		return &TapEndpoint{}, fmt.Errorf("invalid network endpoint index: %d", idx)
	}
	uniqueID := uuid.Generate().String()

	endpoint := &TapEndpoint{
		TapInterface: TapInterface{
			ID:   uniqueID,
			Name: fmt.Sprintf("eth%d", idx),
			TAPIface: NetworkInterface{
				Name: fmt.Sprintf("tap%d_kata", idx),
			},
		},
		EndpointType: TapEndpointType,
	}
	if ifName != "" {
		endpoint.TapInterface.Name = ifName
	}

	return endpoint, nil
}

func tapNetwork(endpoint *TapEndpoint, numCPUs uint32, disableVhostNet bool) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Close()

	tapLink, fds, err := createLink(netHandle, endpoint.TapInterface.TAPIface.Name, &netlink.Tuntap{}, int(numCPUs))
	if err != nil {
		return fmt.Errorf("Could not create TAP interface: %s", err)
	}
	endpoint.TapInterface.VMFds = fds
	if !disableVhostNet {
		vhostFds, err := createVhostFds(int(numCPUs))
		if err != nil {
			return fmt.Errorf("Could not setup vhost fds %s : %s", endpoint.TapInterface.Name, err)
		}
		endpoint.TapInterface.VhostFds = vhostFds
	}
	linkAttrs := endpoint.Properties().Iface.LinkAttrs

	// Save the MAC address to the TAP so that it can later be used
	// to build the QMP command line. This MAC address has to be
	// the one inside the VM in order to avoid any firewall issues. The
	// bridge created by the network plugin on the host actually expects
	// to see traffic from this MAC address and not another one.
	endpoint.TapInterface.TAPIface.HardAddr = linkAttrs.HardwareAddr.String()
	if err := netHandle.LinkSetMTU(tapLink, linkAttrs.MTU); err != nil {
		return fmt.Errorf("Could not set TAP MTU %d: %s", linkAttrs.MTU, err)
	}
	if err := netHandle.LinkSetUp(tapLink); err != nil {
		return fmt.Errorf("Could not enable TAP %s: %s", endpoint.TapInterface.Name, err)
	}
	return nil
}

func unTapNetwork(name string) error {
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

func (endpoint *TapEndpoint) save() persistapi.NetworkEndpoint {
	tapif := saveTapIf(&endpoint.TapInterface)

	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),
		Tap: &persistapi.TapEndpoint{
			TapInterface: *tapif,
		},
	}
}
func (endpoint *TapEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = TapEndpointType

	if s.Tap != nil {
		tapif := loadTapIf(&s.Tap.TapInterface)
		endpoint.TapInterface = *tapif
	}
}

func (endpoint *TapEndpoint) GetRxRateLimiter() bool {
	return endpoint.RxRateLimiter
}

func (endpoint *TapEndpoint) SetRxRateLimiter() error {
	endpoint.RxRateLimiter = true
	return nil
}

func (endpoint *TapEndpoint) GetTxRateLimiter() bool {
	return endpoint.TxRateLimiter
}

func (endpoint *TapEndpoint) SetTxRateLimiter() error {
	endpoint.TxRateLimiter = true
	return nil
}
