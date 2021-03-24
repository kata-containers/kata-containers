// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/drivers"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/cgroups"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"
	"github.com/safchain/ethtool"
)

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
	// Unbind physical interface from host driver and bind to vfio
	// so that it can be passed to qemu.
	vfioPath, err := bindNICToVFIO(endpoint)
	if err != nil {
		return err
	}

	c, err := cgroups.DeviceToCgroupDevice(vfioPath)
	if err != nil {
		return err
	}

	d := config.DeviceInfo{
		ContainerPath: c.Path,
		DevType:       string(c.Type),
		Major:         c.Major,
		Minor:         c.Minor,
		ColdPlug:      true,
	}

	_, err = s.AddDevice(ctx, d)
	return err
}

// Detach for physical endpoint unbinds the physical network interface from vfio-pci
// and binds it back to the saved host driver.
func (endpoint *PhysicalEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	// Bind back the physical network interface to host.
	// We need to do this even if a new network namespace has not
	// been created by virtcontainers.

	// We do not need to enter the network namespace to bind back the
	// physical interface to host driver.
	return bindNICToHost(endpoint)
}

// HotAttach for physical endpoint not supported yet
func (endpoint *PhysicalEndpoint) HotAttach(ctx context.Context, h hypervisor) error {
	return fmt.Errorf("PhysicalEndpoint does not support Hot attach")
}

// HotDetach for physical endpoint not supported yet
func (endpoint *PhysicalEndpoint) HotDetach(ctx context.Context, h hypervisor, netNsCreated bool, netNsPath string) error {
	return fmt.Errorf("PhysicalEndpoint does not support Hot detach")
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
	contents, err := ioutil.ReadFile(ifaceDevicePath)
	if err != nil {
		return nil, err
	}

	deviceID := strings.TrimSpace(string(contents))

	// Vendor id
	ifaceVendorPath := filepath.Join(sysPCIDevicesPath, bdf, "vendor")
	contents, err = ioutil.ReadFile(ifaceVendorPath)
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
	return drivers.BindDevicetoVFIO(endpoint.BDF, endpoint.Driver, endpoint.VendorDeviceID)
}

func bindNICToHost(endpoint *PhysicalEndpoint) error {
	return drivers.BindDevicetoHost(endpoint.BDF, endpoint.Driver, endpoint.VendorDeviceID)
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
