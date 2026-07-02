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

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/drivers"
	resCtrl "github.com/kata-containers/kata-containers/src/runtime/pkg/resourcecontrol"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/safchain/ethtool"
	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
)

var physicalTrace = getNetworkTrace(PhysicalEndpointType)

// PhysicalEndpoint gathers a physical network interface and its properties
type PhysicalEndpoint struct {
	IfaceName          string
	IsVFIO             bool
	HardAddr           string
	EndpointProperties NetworkInfo
	EndpointType       EndpointType
	BDF                string
	Driver             string
	VendorDeviceID     string
	PCIPath            vcTypes.PciPath
	CCWDevice          *vcTypes.CcwDevice
	NetPair            NetworkInterfacePair
	BusType            string
	RxRateLimiter      bool
	TxRateLimiter      bool
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
	return &endpoint.NetPair
}

// Attach for physical endpoint binds the physical network interface to
// vfio-pci and adds device to the hypervisor with vfio-passthrough.
func (endpoint *PhysicalEndpoint) Attach(ctx context.Context, s *Sandbox) error {
	span, ctx := physicalTrace(ctx, "Attach", endpoint)
	defer span.End()
	if endpoint.IsVFIO {
		// Push the desired netdev MAC down to the VF as an "admin MAC" via the
		// PF before we rebind to vfio-pci. Without this the guest's mlx5_core
		// inherits whatever firmware-default MAC the VF was created with, the
		// guest netdev MAC ends up different from the IB port's HCA MAC, and
		// `mlx5_ib`'s GID cache refuses to populate
		// `/sys/class/infiniband/mlx5_*/ports/N/gids/*`. RoCE then looks like
		// it works (port = ACTIVE, link_layer = Ethernet) but every actual
		// verb that needs a GID — RoCEv2 packets, address handles, librdmacm
		// bind — fails. The bind-to-vfio-pci step is the VF's reset, so the
		// firmware applies the admin MAC during that transition; the guest
		// then sees a single consistent MAC across netdev / IB port / HCA.
		// Best-effort: any failure here is logged and the fallback (agent-side
		// MAC reconciliation, see rpc.rs::update_interface) still keeps L2/L3
		// working.
		if endpoint.HardAddr != "" && endpoint.BDF != "" {
			if err := setVfAdminMAC(endpoint.BDF, endpoint.HardAddr); err != nil {
				networkLogger().WithFields(logrus.Fields{
					"bdf":    endpoint.BDF,
					"netdev": endpoint.IfaceName,
					"hwAddr": endpoint.HardAddr,
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
	} else {
		h := s.hypervisor
		if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
			return err
		}
		return h.AddDevice(ctx, endpoint, NetDev)
	}
}

// Detach for physical endpoint unbinds the physical network interface from vfio-pci
// and binds it back to the saved host driver.
func (endpoint *PhysicalEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	span, _ := physicalTrace(ctx, "Detach", endpoint)
	defer span.End()
	if endpoint.IsVFIO {
		// Bind back the physical network interface to host.
		// We need to do this even if a new network namespace has not
		// been created by virtcontainers.
		// We do not need to enter the network namespace to bind back the
		// physical interface to host driver.
		return bindNICToHost(endpoint)
	} else {
		// The network namespace would have been deleted at this point
		// if it has not been created by virtcontainers.
		if !netNsCreated {
			return nil
		}
		return doNetNS(netNsPath, func(_ ns.NetNS) error {
			return xDisconnectVMNetwork(ctx, endpoint)
		})
	}
}

// HotAttach for physical endpoint not supported yet
func (endpoint *PhysicalEndpoint) HotAttach(ctx context.Context, s *Sandbox) error {
	span, ctx := physicalTrace(ctx, "HotAttach", endpoint)
	defer span.End()
	if endpoint.IsVFIO {
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
	} else {
		h := s.hypervisor
		if err := xConnectVMNetwork(ctx, endpoint, h); err != nil {
			return err
		}
		if _, err := h.HotplugAddDevice(ctx, endpoint, NetDev); err != nil {
			return err
		}
		return nil
	}
}

// HotDetach for physical endpoint not supported yet
func (endpoint *PhysicalEndpoint) HotDetach(ctx context.Context, s *Sandbox, netNsCreated bool, netNsPath string) error {
	span, ctx := physicalTrace(ctx, "HotDetach", endpoint)
	defer span.End()
	var vfioPath string
	var err error
	if endpoint.IsVFIO {
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
		if device == nil {
			return fmt.Errorf("failed to find VFIO device for %s during hot detach", endpoint.BDF)
		}
		s.devManager.RemoveDevice(device.DeviceID())
		// We do not need to enter the network namespace to bind back the
		// physical interface to host driver.
		return bindNICToHost(endpoint)
	} else {
		if !netNsCreated {
			return nil
		}
		if err := doNetNS(netNsPath, func(_ ns.NetNS) error {
			return xDisconnectVMNetwork(ctx, endpoint)
		}); err != nil {
			networkLogger().WithError(err).Warn("Error un-bridging virtual ep")
		}
		h := s.hypervisor
		if _, err := h.HotplugRemoveDevice(ctx, endpoint, NetDev); err != nil {
			return err
		}
		return nil
	}
}

// isPhysicalIface checks if an interface is a physical device by inspecting
// the link's ParentDevBus attribute. Returns true when the bus is "pci" or
// "vmbus". ParentDevBus is populated by the kernel via netlink
// and does not require sysfs access inside the network namespace.
func isPhysicalIface(link netlink.Link) bool {
	isParent := (link.Attrs().ParentDevBus == "pci" || link.Attrs().ParentDevBus == "vmbus")
	return isParent
}

var sysBusPath = "/sys/bus/"

// Get vendor and device id from pci space (sys/bus/pci/devices, or sys/bus/vmbus/devices, ...)
func getDevicesPath(link netlink.Link) string {
	return filepath.Join(sysBusPath, link.Attrs().ParentDevBus, "devices")
}

// Get vendor and device id from pci space (sys/bus/pci/devices/$BusDeviceInfo)
func getIfaceDevicePath(link netlink.Link, deviceInterfaceName string) (string, string, error) {
	if link.Attrs().ParentDevBus == "pci" {
		// Get ethtool handle to derive driver and bus
		ethHandle, err := ethtool.NewEthtool()
		if err != nil {
			return "", "", err
		}
		defer ethHandle.Close()
		// Get Bus info
		bdf, err := ethHandle.BusInfo(deviceInterfaceName)
		if err != nil {
			return "", "", err
		}
		// Get device by following symlink /sys/bus/pci/devices/$bdf
		return filepath.Join(getDevicesPath(link), bdf), bdf, nil
	} else if link.Attrs().ParentDevBus == "vmbus" {
		parentDev := link.Attrs().ParentDev
		if parentDev == "" {
			return "", "", fmt.Errorf("vmbus interface %q has empty ParentDev; cannot resolve sysfs device path", deviceInterfaceName)
		}
		return filepath.Join(getDevicesPath(link), parentDev), parentDev, nil
	} else {
		return "", "", fmt.Errorf("unsupported ParentDevBus: %s", link.Attrs().ParentDevBus)
	}
}
func createPhysicalEndpoint(idx int, netInfo NetworkInfo, isVFIODisabled bool, interworkingModel NetInterworkingModel) (*PhysicalEndpoint, error) {
	sysIfaceDevicePath, bdf, err := getIfaceDevicePath(netInfo.Link, netInfo.Iface.Name)
	if err != nil {
		return nil, err
	}
	// Get driver by following symlink /sys/bus/pci/devices/$bdf/driver or /sys/bus/vmbus/devices/$guid/driver
	driverPath := filepath.Join(sysIfaceDevicePath, "driver")
	link, err := os.Readlink(driverPath)
	if err != nil {
		return nil, err
	}
	driver := filepath.Base(link)
	// Get device by following symlink /sys/bus/pci/devices/$bdf/device or /sys/bus/vmbus/devices/$guid/device
	ifaceDevicePath := filepath.Join(sysIfaceDevicePath, "device")
	contents, err := os.ReadFile(ifaceDevicePath)
	if err != nil {
		return nil, err
	}
	deviceID := strings.TrimSpace(string(contents))
	// Vendor id (/sys/bus/pci/devices/$bdf/vendor or /sys/bus/vmbus/devices/$guid/vendor)
	ifaceVendorPath := filepath.Join(sysIfaceDevicePath, "vendor")
	contents, err = os.ReadFile(ifaceVendorPath)
	if err != nil {
		return nil, err
	}
	// Determine whether to use VFIO passthrough based on bus type:
	// PCI devices are passed through via VFIO.
	// VMBus devices use a network pair (tap/bridge).
	isVFIO := (netInfo.Link.Attrs().ParentDevBus == "pci")
	netPair := NetworkInterfacePair{}
	if isVFIO {
		if isVFIODisabled {
			// When `cold_plug_vfio` is set to "no-port", the PhysicalEndpoint's VFIO device cannot be attached to the guest VM.
			// Fail early to prevent the interface from being unbound and rebound to the VFIO driver.
			return nil, fmt.Errorf("unable to add physical endpoint %s: cold_plug_vfio is disabled", netInfo.Iface.Name)
		}
	} else {
		if idx < 0 {
			return nil, fmt.Errorf("invalid network endpoint index: %d", idx)
		}
		netPair, err = createNetworkInterfacePair(idx, netInfo.Iface.Name, interworkingModel)
		if err != nil {
			return nil, err
		}
		if netInfo.Iface.Name != "" {
			netPair.VirtIface.Name = netInfo.Iface.Name
		}
	}
	vendorID := strings.TrimSpace(string(contents))
	vendorDeviceID := fmt.Sprintf("%s %s", vendorID, deviceID)
	vendorDeviceID = strings.TrimSpace(vendorDeviceID)
	physicalEndpoint := &PhysicalEndpoint{
		IfaceName:      netInfo.Iface.Name,
		IsVFIO:         isVFIO,
		HardAddr:       netInfo.Iface.HardwareAddr.String(),
		VendorDeviceID: vendorDeviceID,
		EndpointType:   PhysicalEndpointType,
		Driver:         driver,
		BDF:            bdf,
		NetPair:        netPair,
		BusType:        netInfo.Link.Attrs().ParentDevBus,
	}
	return physicalEndpoint, nil
}
func bindNICToVFIO(endpoint *PhysicalEndpoint) (string, error) {
	return drivers.BindDevicetoVFIO(endpoint.BDF, endpoint.Driver)
}
func bindNICToHost(endpoint *PhysicalEndpoint) error {
	return drivers.BindDevicetoHost(endpoint.BDF, endpoint.Driver)
}

// setVfAdminMAC pushes `mac` down to the VF identified by `vfBDF` as an admin
// MAC, via the parent PF using rtnetlink (the same plumbing as `ip link set
// <PF> vf <N> mac <MAC>`). The admin MAC is stored in the PF's per-VF context
// and applied by the VF firmware on its next reset/init — which is exactly the
// unbind-from-mlx5_core / bind-to-vfio-pci cycle we do right after this call.
// Best-effort: returns an error only when the caller should know about it
// (logged at warn). Bare bones, no retries.
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

	// At this point we may be running inside the pod's network namespace
	// (network_linux.go::addSingleEndpoint is wrapped in doNetNS), but the
	// PF netdev lives in the host's init netns. Anchor the netlink handle
	// to PID 1's netns so the PF lookup and the VF MAC RTM_SETLINK
	// actually hit the right kernel state.
	hostNs, err := netns.GetFromPid(1)
	if err != nil {
		return fmt.Errorf("open host netns (/proc/1/ns/net): %w", err)
	}
	defer hostNs.Close()

	handle, err := netlink.NewHandleAt(hostNs)
	if err != nil {
		return fmt.Errorf("netlink NewHandleAt(host-ns): %w", err)
	}
	defer handle.Close()

	pfLink, err := handle.LinkByName(pfNetdev)
	if err != nil {
		return fmt.Errorf("link %s not found in host netns: %w", pfNetdev, err)
	}

	return handle.LinkSetVfHardwareAddr(pfLink, vfIndex, hwaddr)
}

// resolveVfPfPath returns the PF BDF and VF index for a given VF BDF by
// inspecting sysfs virtfn symlinks under the PF directory.
func resolveVfPfPath(vfBDF string) (string, int, error) {
	pfDir := filepath.Join("/sys/bus/pci/devices", vfBDF, "physfn")
	pfTarget, err := os.Readlink(pfDir)
	if err != nil {
		return "", -1, fmt.Errorf("readlink %s: %w", pfDir, err)
	}
	pfBDF := filepath.Base(pfTarget)

	virtfnDir := filepath.Join("/sys/bus/pci/devices", pfBDF)
	entries, err := os.ReadDir(virtfnDir)
	if err != nil {
		return "", -1, fmt.Errorf("readdir %s: %w", virtfnDir, err)
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
		target, err := os.Readlink(filepath.Join(virtfnDir, name))
		if err != nil {
			continue
		}
		if filepath.Base(target) == vfBDF {
			return pfBDF, idx, nil
		}
	}
	return "", -1, fmt.Errorf("no virtfn under %s links to %s", virtfnDir, vfBDF)
}

// pfNetdevName returns the single netdev name registered under the
// PF's sysfs node. Returns an error if zero netdevs are found.
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
		networkLogger().WithFields(logrus.Fields{
			"pf-bdf":      pfBDF,
			"netdev-list": strings.Join(names, ","),
			"picked":      names[0],
		}).Warn("pfNetdevName: PF exposes multiple netdevs; picking first entry from sysdev")
		return names[0], nil
	}
}

func (endpoint *PhysicalEndpoint) save() persistapi.NetworkEndpoint {
	// saveNetIfPair returns a non-nil pair when given a non-nil input. For VFIO
	// physical endpoints the pair is intentionally empty; persist it as-is
	// without warning.
	savedPair := *saveNetIfPair(&endpoint.NetPair)
	return persistapi.NetworkEndpoint{
		Type: string(endpoint.Type()),
		Physical: &persistapi.PhysicalEndpoint{
			BDF:            endpoint.BDF,
			Driver:         endpoint.Driver,
			VendorDeviceID: endpoint.VendorDeviceID,
			NetPair:        savedPair,
			BusType:        endpoint.BusType,
			IsVFIO:         endpoint.IsVFIO,
		},
	}
}
func (endpoint *PhysicalEndpoint) load(s persistapi.NetworkEndpoint) {
	endpoint.EndpointType = PhysicalEndpointType
	if s.Physical != nil {
		if netpair := loadNetIfPair(&s.Physical.NetPair); netpair != nil {
			endpoint.NetPair = *netpair
		}
		endpoint.BDF = s.Physical.BDF
		endpoint.Driver = s.Physical.Driver
		endpoint.VendorDeviceID = s.Physical.VendorDeviceID
		endpoint.BusType = s.Physical.BusType
		endpoint.IsVFIO = s.Physical.IsVFIO
	}
}

func (endpoint *PhysicalEndpoint) GetRxRateLimiter() bool {
	return endpoint.RxRateLimiter
}
func (endpoint *PhysicalEndpoint) SetRxRateLimiter() error {
	if endpoint.IsVFIO {
		// VFIO endpoints use VFIO passthrough; the runtime has no dataplane in
		// which to enforce rate limiting. Leave the flag unset so callers can
		// observe the actual runtime behavior via GetRxRateLimiter().
		networkLogger().WithField("endpoint", endpoint.Name()).
			Debug("ignoring SetRxRateLimiter on VFIO physical endpoint")
		return nil
	}
	endpoint.RxRateLimiter = true
	return nil
}

func (endpoint *PhysicalEndpoint) GetTxRateLimiter() bool {
	return endpoint.TxRateLimiter
}
func (endpoint *PhysicalEndpoint) SetTxRateLimiter() error {
	if endpoint.IsVFIO {
		// VFIO endpoints use VFIO passthrough; the runtime has no dataplane in
		// which to enforce rate limiting. Leave the flag unset so callers can
		// observe the actual runtime behavior via GetTxRateLimiter().
		networkLogger().WithField("endpoint", endpoint.Name()).
			Debug("ignoring SetTxRateLimiter on VFIO physical endpoint")
		return nil
	}
	endpoint.TxRateLimiter = true
	return nil
}
