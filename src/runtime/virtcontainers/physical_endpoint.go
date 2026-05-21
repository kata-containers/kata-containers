//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/drivers"
	resCtrl "github.com/kata-containers/kata-containers/src/runtime/pkg/resourcecontrol"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/safchain/ethtool"
	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"
)

var physicalTrace = getNetworkTrace(PhysicalEndpointType)

// PhysicalEndpoint gathers a physical network interface and its properties
type PhysicalEndpoint struct {
	IfaceName          string
	HardAddr           string
	EndpointProperties NetworkInfo
	EndpointType       EndpointType
	BDF                string
	Driver             string
	VendorDeviceID     string
	PCIPath            vcTypes.PciPath
	CCWDevice          *vcTypes.CcwDevice
}

// Properties returns the properties of the physical interface.
func (endpoint *PhysicalEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// HardwareAddr returns the mac address of the physical network interface.
func (endpoint *PhysicalEndpoint) HardwareAddr() string {
	return endpoint.HardAddr
}

// Name returns name of the physical interface.
func (endpoint *PhysicalEndpoint) Name() string {
	return endpoint.IfaceName
}

// Type indentifies the endpoint as a physical endpoint.
func (endpoint *PhysicalEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// PciPath returns the PCI path of the endpoint.
func (endpoint *PhysicalEndpoint) PciPath() vcTypes.PciPath {
	return endpoint.PCIPath
}

// SetPciPath sets the PCI path of the endpoint.
func (endpoint *PhysicalEndpoint) SetPciPath(pciPath vcTypes.PciPath) {
	endpoint.PCIPath = pciPath
}

// CcwDevice returns the CCW device of the endpoint.
func (endpoint *PhysicalEndpoint) CcwDevice() *vcTypes.CcwDevice {
	return endpoint.CCWDevice
}

// SetCcwDevice sets the CCW device of the endpoint.
func (endpoint *PhysicalEndpoint) SetCcwDevice(ccwDev vcTypes.CcwDevice) {
	endpoint.CCWDevice = &ccwDev
}

// SetProperties sets the properties of the physical endpoint.
func (endpoint *PhysicalEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// NetworkPair returns the network pair of the endpoint.
func (endpoint *PhysicalEndpoint) NetworkPair() *NetworkInterfacePair {
	return nil
}

// Attach for physical endpoint binds the physical network interface to
// vfio-pci and adds device to the hypervisor with vfio-passthrough.
func (endpoint *PhysicalEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	span, ctx := physicalTrace(ctx, "Attach", endpoint)
	defer span.End()

	// [coldplug-vf-roce] Push the desired netdev MAC down to the VF as
	// an "admin MAC" via the PF before we rebind to vfio-pci. Without
	// this the guest's mlx5_core inherits whatever firmware-default MAC
	// the VF was created with, the guest netdev MAC ends up different
	// from the IB port's HCA MAC, and `mlx5_ib`'s GID cache refuses to
	// populate `/sys/class/infiniband/mlx5_*/ports/N/gids/*`. RoCE then
	// looks like it works (port = ACTIVE, link_layer = Ethernet) but
	// every actual verb that needs a GID — RoCEv2 packets, address
	// handles, librdmacm bind — fails. The bind-to-vfio-pci step is the
	// VF's reset, so the firmware applies the admin MAC during that
	// transition; the guest then sees a single consistent MAC across
	// netdev / IB port / HCA. Best-effort: any failure here is logged
	// and the fallback (agent-side MAC reconciliation, see
	// rpc.rs::update_interface) still keeps L2/L3 working.
	if endpoint.HardAddr != "" && endpoint.BDF != "" {
		if err := setVfAdminMAC(endpoint.BDF, endpoint.HardAddr); err != nil {
			networkLogger().WithFields(logrus.Fields{
				"bdf":     endpoint.BDF,
				"netdev":  endpoint.IfaceName,
				"hwAddr":  endpoint.HardAddr,
			}).WithError(err).Warn("setVfAdminMAC: skipped, falling back to in-guest MAC reconciliation")
		}
	}

	// Unbind physical interface from host driver and bind to vfio
	// so that it can be passed to qemu.
	vfioPath, err := bindNICToVFIO(endpoint)
	if err != nil {
		return err
	}

	c, err := resCtrl.DeviceToCgroupDeviceRule(vfioPath)
	if err != nil {
		return err
	}

	d := config.DeviceInfo{
		ContainerPath: vfioPath,
		DevType:       string(c.Type),
		Major:         c.Major,
		Minor:         c.Minor,
		ColdPlug:      true,
		Port:          s.config.HypervisorConfig.ColdPlugVFIO,
	}

	_, err = s.AddDevice(ctx, d)
	return err
}

// Detach for physical endpoint unbinds the physical network interface from vfio-pci
// and binds it back to the saved host driver.
func (endpoint *PhysicalEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	span, _ := physicalTrace(ctx, "Detach", endpoint)
	defer span.End()

	// Bind back the physical network interface to host.
	// We need to do this even if a new network namespace has not
	// been created by virtcontainers.

	// We do not need to enter the network namespace to bind back the
	// physical interface to host driver.
	return bindNICToHost(endpoint)
}

// HotAttach for physical endpoint not supported yet
func (endpoint *PhysicalEndpoint) HotAttach(ctx context.Context, s *Sandbox) error {
	span, ctx := physicalTrace(ctx, "HotAttach", endpoint)
	defer span.End()

	// Unbind physical interface from host driver and bind to vfio
	// so that it can be passed to the hypervisor.
	vfioPath, err := bindNICToVFIO(endpoint)
	if err != nil {
		return err
	}

	c, err := resCtrl.DeviceToCgroupDeviceRule(vfioPath)
	if err != nil {
		return err
	}

	d := config.DeviceInfo{
		ContainerPath: vfioPath,
		DevType:       string(c.Type),
		Major:         c.Major,
		Minor:         c.Minor,
		ColdPlug:      false,
	}

	_, err = s.AddDevice(ctx, d)
	return err
}

// HotDetach for physical endpoint not supported yet
func (endpoint *PhysicalEndpoint) HotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string) error {
	span, _ := physicalTrace(ctx, "HotDetach", endpoint)
	defer span.End()

	var vfioPath string
	var err error

	if vfioPath, err = drivers.GetVFIODevPath(endpoint.BDF); err != nil {
		return err
	}

	c, err := resCtrl.DeviceToCgroupDeviceRule(vfioPath)
	if err != nil {
		return err
	}

	d := config.DeviceInfo{
		ContainerPath: vfioPath,
		DevType:       string(c.Type),
		Major:         c.Major,
		Minor:         c.Minor,
		ColdPlug:      false,
	}

	device := s.devManager.FindDevice(&d)
	s.devManager.RemoveDevice(device.DeviceID())

	// We do not need to enter the network namespace to bind back the
	// physical interface to host driver.
	return bindNICToHost(endpoint)
}

// isPhysicalIface checks if an interface is a physical device.
// We use ethtool here to not rely on device sysfs inside the network namespace.
func isPhysicalIface(ifaceName string) (bool, error) {
	if ifaceName == "lo" {
		return false, nil
	}

	ethHandle, err := ethtool.NewEthtool()
	if err != nil {
		return false, err
	}
	defer ethHandle.Close()

	bus, err := ethHandle.BusInfo(ifaceName)
	if err != nil {
		return false, nil
	}

	// Check for a pci bus format
	tokens := strings.Split(bus, ":")
	if len(tokens) != 3 {
		return false, nil
	}

	return true, nil
}

var sysPCIDevicesPath = "/sys/bus/pci/devices"

func createPhysicalEndpoint(netInfo NetworkInfo) (*PhysicalEndpoint, error) {
	// Get ethtool handle to derive driver and bus
	ethHandle, err := ethtool.NewEthtool()
	if err != nil {
		return nil, err
	}
	defer ethHandle.Close()

	// Get BDF
	bdf, err := ethHandle.BusInfo(netInfo.Iface.Name)
	if err != nil {
		return nil, err
	}

	// Get driver by following symlink /sys/bus/pci/devices/$bdf/driver
	driverPath := filepath.Join(sysPCIDevicesPath, bdf, "driver")
	link, err := os.Readlink(driverPath)
	if err != nil {
		return nil, err
	}

	driver := filepath.Base(link)

	// Get vendor and device id from pci space (sys/bus/pci/devices/$bdf)

	ifaceDevicePath := filepath.Join(sysPCIDevicesPath, bdf, "device")
	contents, err := os.ReadFile(ifaceDevicePath)
	if err != nil {
		return nil, err
	}

	deviceID := strings.TrimSpace(string(contents))

	// Vendor id
	ifaceVendorPath := filepath.Join(sysPCIDevicesPath, bdf, "vendor")
	contents, err = os.ReadFile(ifaceVendorPath)
	if err != nil {
		return nil, err
	}

	vendorID := strings.TrimSpace(string(contents))
	vendorDeviceID := fmt.Sprintf("%s %s", vendorID, deviceID)
	vendorDeviceID = strings.TrimSpace(vendorDeviceID)

	physicalEndpoint := &PhysicalEndpoint{
		IfaceName:      netInfo.Iface.Name,
		HardAddr:       netInfo.Iface.HardwareAddr.String(),
		VendorDeviceID: vendorDeviceID,
		EndpointType:   PhysicalEndpointType,
		Driver:         driver,
		BDF:            bdf,
	}

	return physicalEndpoint, nil
}

func bindNICToVFIO(endpoint *PhysicalEndpoint) (string, error) {
	return drivers.BindDevicetoVFIO(endpoint.BDF, endpoint.Driver)
}

// [coldplug-vf-roce] setVfAdminMAC pushes `mac` down to the VF
// identified by `vfBDF` as an admin MAC, via the parent PF using
// rtnetlink (the same plumbing as `ip link set <PF> vf <N> mac <MAC>`).
// The admin MAC is stored in the PF's per-VF context and applied by
// the VF firmware on its next reset/init — which is exactly the
// unbind-from-mlx5_core / bind-to-vfio-pci cycle we do right after
// this call. Best-effort: returns an error only when the caller
// should know about it (logged at warn). Bare bones, no retries.
func setVfAdminMAC(vfBDF, mac string) error {
	hwaddr, err := net.ParseMAC(mac)
	if err != nil {
		return fmt.Errorf("parse MAC %q: %w", mac, err)
	}

	pfBDF, vfIndex, err := resolveVfPfPath(vfBDF)
	if err != nil {
		return fmt.Errorf("resolve PF/vf-index for VF %s: %w", vfBDF, err)
	}

	pfNetdev, err := pfNetdevName(pfBDF)
	if err != nil {
		return fmt.Errorf("look up PF netdev for %s: %w", pfBDF, err)
	}

	link, err := netlink.LinkByName(pfNetdev)
	if err != nil {
		return fmt.Errorf("netlink LinkByName(%s): %w", pfNetdev, err)
	}

	if err := netlink.LinkSetVfHardwareAddr(link, vfIndex, hwaddr); err != nil {
		return fmt.Errorf("netlink LinkSetVfHardwareAddr(%s, vf=%d, mac=%s): %w",
			pfNetdev, vfIndex, mac, err)
	}

	networkLogger().WithFields(logrus.Fields{
		"vf-bdf":     vfBDF,
		"pf-bdf":     pfBDF,
		"pf-netdev":  pfNetdev,
		"vf-index":   vfIndex,
		"admin-mac":  mac,
	}).Info("setVfAdminMAC: stamped admin MAC on VF")
	return nil
}

// resolveVfPfPath walks `/sys/bus/pci/devices/<vfBDF>/physfn` to find
// the parent PF, then walks the PF's `virtfnN/` symlinks to find the
// VF index that points back at `vfBDF`. Returns (pfBDF, vfIndex).
func resolveVfPfPath(vfBDF string) (string, int, error) {
	physfn := filepath.Join("/sys/bus/pci/devices", vfBDF, "physfn")
	pfTarget, err := os.Readlink(physfn)
	if err != nil {
		return "", -1, fmt.Errorf("readlink(%s): %w (is %s actually a VF?)", physfn, err, vfBDF)
	}
	pfBDF := filepath.Base(pfTarget)

	pfDir := filepath.Join("/sys/bus/pci/devices", pfBDF)
	entries, err := os.ReadDir(pfDir)
	if err != nil {
		return "", -1, fmt.Errorf("read_dir(%s): %w", pfDir, err)
	}

	const prefix = "virtfn"
	for _, entry := range entries {
		name := entry.Name()
		if !strings.HasPrefix(name, prefix) {
			continue
		}
		idxStr := strings.TrimPrefix(name, prefix)
		idx, err := strconv.Atoi(idxStr)
		if err != nil {
			continue
		}

		target, err := os.Readlink(filepath.Join(pfDir, name))
		if err != nil {
			continue
		}
		if filepath.Base(target) == vfBDF {
			return pfBDF, idx, nil
		}
	}
	return "", -1, fmt.Errorf("no virtfn under %s links to %s", pfDir, vfBDF)
}

// pfNetdevName returns the single netdev name registered under the
// PF's sysfs node, e.g. `enp6s0f0np0` for a BlueField-3 PF0. Returns
// an error if zero or more-than-one netdev is found, since with
// SR-IOV the PF has exactly one netdev per port and per-PF here.
func pfNetdevName(pfBDF string) (string, error) {
	netDir := filepath.Join("/sys/bus/pci/devices", pfBDF, "net")
	entries, err := os.ReadDir(netDir)
	if err != nil {
		return "", fmt.Errorf("read_dir(%s): %w", netDir, err)
	}
	var names []string
	for _, e := range entries {
		names = append(names, e.Name())
	}
	switch len(names) {
	case 0:
		return "", fmt.Errorf("no netdev under %s (PF not bound to a Linux driver?)", netDir)
	case 1:
		return names[0], nil
	default:
		// Should not happen for SR-IOV NICs; pick the first
		// deterministically and surface the ambiguity in the error
		// path so callers can decide.
		return names[0], fmt.Errorf("PF %s has %d netdevs (%s), picked %s",
			pfBDF, len(names), strings.Join(names, ","), names[0])
	}
}

func bindNICToHost(endpoint *PhysicalEndpoint) error {
	return drivers.BindDevicetoHost(endpoint.BDF, endpoint.Driver)
}

func (endpoint *PhysicalEndpoint) save() persistapi.NetworkEndpoint {
	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),

		Physical: &persistapi.PhysicalEndpoint{
			BDF:            endpoint.BDF,
			Driver:         endpoint.Driver,
			VendorDeviceID: endpoint.VendorDeviceID,
		},
	}
}

func (endpoint *PhysicalEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = PhysicalEndpointType

	if s.Physical != nil {
		endpoint.BDF = s.Physical.BDF
		endpoint.Driver = s.Physical.Driver
		endpoint.VendorDeviceID = s.Physical.VendorDeviceID
	}
}

// unsupported
func (endpoint *PhysicalEndpoint) GetRxRateLimiter() bool {
	return false
}

func (endpoint *PhysicalEndpoint) SetRxRateLimiter() error {
	return fmt.Errorf("rx rate limiter is unsupported for physical endpoint")
}

// unsupported
func (endpoint *PhysicalEndpoint) GetTxRateLimiter() bool {
	return false
}

func (endpoint *PhysicalEndpoint) SetTxRateLimiter() error {
	return fmt.Errorf("tx rate limiter is unsupported for physical endpoint")
}
