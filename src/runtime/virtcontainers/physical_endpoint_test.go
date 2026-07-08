//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"net"
	"os"
	"path/filepath"
	"testing"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/containernetworking/plugins/pkg/testutils"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/stretchr/testify/assert"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
)

func TestPhysicalEndpoint_HotAttach_VFIO(t *testing.T) {
	assert := assert.New(t)
	v := &PhysicalEndpoint{
		IfaceName: "eth0",
		HardAddr:  net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
		BDF:       "0000:00:1f.0",
		Driver:    "fake-driver",
		IsVFIO:    true,
	}

	s := &Sandbox{
		hypervisor: &mockHypervisor{},
	}

	// VFIO path tries to write driver_override in sysfs which fails without real hardware
	err := v.HotAttach(context.Background(), s)
	assert.Error(err)
	assert.Contains(err.Error(), "0000:00:1f.0")
}

func TestPhysicalEndpoint_HotAttach_NonVFIO(t *testing.T) {
	assert := assert.New(t)
	v := &PhysicalEndpoint{
		IfaceName: "eth0",
		HardAddr:  net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
		IsVFIO:    false,
	}

	s := &Sandbox{
		hypervisor: &mockHypervisor{},
	}

	// Non-VFIO path tries xConnectVMNetwork which fails without tap/bridge setup
	err := v.HotAttach(context.Background(), s)
	assert.Error(err)
}

func TestPhysicalEndpoint_HotDetach_VFIO(t *testing.T) {
	assert := assert.New(t)
	v := &PhysicalEndpoint{
		IfaceName: "eth0",
		HardAddr:  net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
		BDF:       "0000:00:1f.0",
		Driver:    "fake-driver",
		IsVFIO:    true,
	}

	s := &Sandbox{
		hypervisor: &mockHypervisor{},
	}

	// VFIO path tries to read iommu_group in sysfs which fails without real hardware
	err := v.HotDetach(context.Background(), s, true, "")
	assert.Error(err)
	assert.Contains(err.Error(), "0000:00:1f.0")
}

func TestPhysicalEndpoint_HotDetach_NonVFIO_NetNsNotCreated(t *testing.T) {
	assert := assert.New(t)
	v := &PhysicalEndpoint{
		IfaceName: "eth0",
		HardAddr:  net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
		IsVFIO:    false,
	}

	s := &Sandbox{
		hypervisor: &mockHypervisor{},
	}

	// Non-VFIO with netNsCreated=false should return nil immediately
	err := v.HotDetach(context.Background(), s, false, "")
	assert.NoError(err)
}

func TestPhysicalEndpoint_HotDetach_NonVFIO_NetNsCreated(t *testing.T) {
	assert := assert.New(t)
	v := &PhysicalEndpoint{
		IfaceName: "eth0",
		HardAddr:  net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
		IsVFIO:    false,
	}

	s := &Sandbox{
		hypervisor: &mockHypervisor{},
	}

	// Non-VFIO with netNsCreated=true but empty path calls doNetNS("", ...),
	// xDisconnectVMNetwork error is only logged as a warning (not returned).
	// mockHypervisor.HotplugRemoveDevice succeeds, so overall returns nil.
	err := v.HotDetach(context.Background(), s, true, "")
	assert.NoError(err)
}

func TestPhysicalEndpoint_NetworkPair(t *testing.T) {
	assert := assert.New(t)

	netPair := NetworkInterfacePair{
		VirtIface: NetworkInterface{
			Name: "eth0",
		},
	}

	v := &PhysicalEndpoint{
		IfaceName: "eth0",
		NetPair:   netPair,
	}

	result := v.NetworkPair()
	assert.NotNil(result)
	assert.Equal("eth0", result.VirtIface.Name)
}

func TestPhysicalEndpoint_Detach_NonVFIO_NetNsNotCreated(t *testing.T) {
	assert := assert.New(t)
	v := &PhysicalEndpoint{
		IfaceName: "eth0",
		HardAddr:  net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
		IsVFIO:    false,
	}

	// Non-VFIO with netNsCreated=false should return nil immediately
	err := v.Detach(context.Background(), false, "")
	assert.NoError(err)
}

func TestIsPhysicalIface(t *testing.T) {
	assert := assert.New(t)

	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	testNetIface := "testIface0"
	testMTU := 1500
	testMACAddr := "00:00:00:00:00:01"

	hwAddr, err := net.ParseMAC(testMACAddr)
	assert.NoError(err)

	link := &netlink.Bridge{
		LinkAttrs: netlink.LinkAttrs{
			Name:         testNetIface,
			MTU:          testMTU,
			HardwareAddr: hwAddr,
			TxQLen:       -1,
		},
	}

	n, err := testutils.NewNS()
	assert.NoError(err)
	defer n.Close()

	netnsHandle, err := netns.GetFromPath(n.Path())
	assert.NoError(err)
	defer netnsHandle.Close()

	netlinkHandle, err := netlink.NewHandleAt(netnsHandle)
	assert.NoError(err)
	defer netlinkHandle.Close()

	err = netlinkHandle.LinkAdd(link)
	assert.NoError(err)

	// Fetch the link back from the kernel so its attributes (e.g. ParentDevBus)
	// reflect reality rather than whatever was set on the local struct.
	kernelLink, err := netlinkHandle.LinkByName(testNetIface)
	assert.NoError(err)

	var isPhysical bool
	err = doNetNS(n.Path(), func(_ ns.NetNS) error {
		isPhysical = isPhysicalIface(kernelLink)
		return nil
	})
	assert.NoError(err)
	assert.False(isPhysical)
}

func TestIsPhysicalIface_PCI(t *testing.T) {
	assert := assert.New(t)
	link := &netlink.Dummy{
		LinkAttrs: netlink.LinkAttrs{
			Name:         "eth0",
			ParentDevBus: "pci",
		},
	}
	assert.True(isPhysicalIface(link))
}

func TestIsPhysicalIface_VMBus(t *testing.T) {
	assert := assert.New(t)
	link := &netlink.Dummy{
		LinkAttrs: netlink.LinkAttrs{
			Name:         "eth0",
			ParentDevBus: "vmbus",
		},
	}
	assert.True(isPhysicalIface(link))
}

func TestIsPhysicalIface_NoBus(t *testing.T) {
	assert := assert.New(t)
	link := &netlink.Dummy{
		LinkAttrs: netlink.LinkAttrs{
			Name:         "veth0",
			ParentDevBus: "",
		},
	}
	assert.False(isPhysicalIface(link))
}

func TestGetDevicesPath(t *testing.T) {
	assert := assert.New(t)

	pciLink := &netlink.Dummy{
		LinkAttrs: netlink.LinkAttrs{
			ParentDevBus: "pci",
		},
	}
	assert.Equal("/sys/bus/pci/devices", getDevicesPath(pciLink))

	vmbusLink := &netlink.Dummy{
		LinkAttrs: netlink.LinkAttrs{
			ParentDevBus: "vmbus",
		},
	}
	assert.Equal("/sys/bus/vmbus/devices", getDevicesPath(vmbusLink))
}

func TestGetIfaceDevicePath_UnsupportedBus(t *testing.T) {
	assert := assert.New(t)

	link := &netlink.Dummy{
		LinkAttrs: netlink.LinkAttrs{
			ParentDevBus: "usb",
		},
	}
	_, _, err := getIfaceDevicePath(link, "eth0")
	assert.Error(err)
	assert.Contains(err.Error(), "unsupported ParentDevBus")
}

func TestGetIfaceDevicePath_VMBus(t *testing.T) {
	assert := assert.New(t)

	guid := "00000000-0000-0000-0000-000000000001"
	link := &netlink.Dummy{
		LinkAttrs: netlink.LinkAttrs{
			ParentDevBus: "vmbus",
			ParentDev:    guid,
		},
	}
	path, bdf, err := getIfaceDevicePath(link, "eth0")
	assert.NoError(err)
	assert.Equal(guid, bdf)
	assert.Equal(filepath.Join("/sys/bus/vmbus/devices", guid), path)
}

func TestCreatePhysicalEndpoint_NegativeIdx(t *testing.T) {
	assert := assert.New(t)

	// Create a temp directory to mock sysfs
	tmpDir := t.TempDir()
	origSysBusPath := sysBusPath
	sysBusPath = tmpDir
	defer func() { sysBusPath = origSysBusPath }()

	guid := "00000000-0000-0000-0000-000000000001"
	vmbusDevDir := filepath.Join(tmpDir, "vmbus", "devices", guid)
	assert.NoError(os.MkdirAll(vmbusDevDir, 0755))

	// Create driver symlink
	driverTarget := filepath.Join(tmpDir, "drivers", "hv_netvsc")
	assert.NoError(os.MkdirAll(driverTarget, 0755))
	assert.NoError(os.Symlink(driverTarget, filepath.Join(vmbusDevDir, "driver")))

	// Create device and vendor files
	assert.NoError(os.WriteFile(filepath.Join(vmbusDevDir, "device"), []byte("0x1572\n"), 0644))
	assert.NoError(os.WriteFile(filepath.Join(vmbusDevDir, "vendor"), []byte("0x8086\n"), 0644))

	// VMBus device → not VFIO → negative idx should fail
	hwAddr, _ := net.ParseMAC("aa:bb:cc:dd:ee:ff")
	netInfo := NetworkInfo{
		Iface: NetlinkIface{
			LinkAttrs: netlink.LinkAttrs{
				Name:         "eth0",
				ParentDevBus: "vmbus",
				ParentDev:    guid,
				HardwareAddr: hwAddr,
			},
			Type: "",
		},
		Link: &netlink.Dummy{
			LinkAttrs: netlink.LinkAttrs{
				Name:         "eth0",
				ParentDevBus: "vmbus",
				ParentDev:    guid,
				HardwareAddr: hwAddr,
			},
		},
	}

	_, err := createPhysicalEndpoint(-1, netInfo, false, DefaultNetInterworkingModel)
	assert.Error(err)
	assert.Contains(err.Error(), "invalid network endpoint index")
}

func TestPhysicalEndpoint_SaveLoad_NonVFIO(t *testing.T) {
	assert := assert.New(t)

	netPair := NetworkInterfacePair{
		TapInterface: TapInterface{
			ID:   "tap-id-1",
			Name: "br-tap1",
			TAPIface: NetworkInterface{
				Name:     "tap1",
				HardAddr: "aa:bb:cc:dd:ee:ff",
			},
		},
		VirtIface: NetworkInterface{
			Name:     "eth0",
			HardAddr: "aa:bb:cc:dd:ee:ff",
		},
		NetInterworkingModel: DefaultNetInterworkingModel,
	}

	endpoint := &PhysicalEndpoint{
		IfaceName:      "eth0",
		HardAddr:       "aa:bb:cc:dd:ee:ff",
		EndpointType:   PhysicalEndpointType,
		BDF:            "0000:01:00.0",
		Driver:         "mlx5_core",
		VendorDeviceID: "0x8086 0x1572",
		IsVFIO:         false,
		NetPair:        netPair,
		BusType:        "pci",
	}

	saved := endpoint.save()
	assert.NotNil(saved.Physical)
	assert.Equal("0000:01:00.0", saved.Physical.BDF)
	assert.Equal("mlx5_core", saved.Physical.Driver)
	assert.Equal("pci", saved.Physical.BusType)
	assert.Equal("tap1", saved.Physical.NetPair.TAPIface.Name)
	assert.Equal("eth0", saved.Physical.NetPair.VirtIface.Name)
	assert.False(saved.Physical.IsVFIO)

	loaded := &PhysicalEndpoint{}
	loaded.load(saved)
	assert.Equal(PhysicalEndpointType, loaded.EndpointType)
	assert.Equal("0000:01:00.0", loaded.BDF)
	assert.Equal("mlx5_core", loaded.Driver)
	assert.Equal("0x8086 0x1572", loaded.VendorDeviceID)
	assert.Equal("pci", loaded.BusType)
	assert.Equal("tap1", loaded.NetPair.TapInterface.TAPIface.Name)
	assert.Equal("eth0", loaded.NetPair.VirtIface.Name)
	assert.Equal(DefaultNetInterworkingModel, loaded.NetPair.NetInterworkingModel)
	assert.False(loaded.IsVFIO)
}

func TestCreatePhysicalEndpoint_NonVFIO_HappyPath(t *testing.T) {
	assert := assert.New(t)

	tmpDir := t.TempDir()
	origSysBusPath := sysBusPath
	sysBusPath = tmpDir
	defer func() { sysBusPath = origSysBusPath }()

	guid := "00000000-0000-0000-0000-000000000002"
	vmbusDevDir := filepath.Join(tmpDir, "vmbus", "devices", guid)
	assert.NoError(os.MkdirAll(vmbusDevDir, 0755))

	// Create driver symlink
	driverTarget := filepath.Join(tmpDir, "drivers", "hv_netvsc")
	assert.NoError(os.MkdirAll(driverTarget, 0755))
	assert.NoError(os.Symlink(driverTarget, filepath.Join(vmbusDevDir, "driver")))

	// Create device and vendor files
	assert.NoError(os.WriteFile(filepath.Join(vmbusDevDir, "device"), []byte("0x1572\n"), 0644))
	assert.NoError(os.WriteFile(filepath.Join(vmbusDevDir, "vendor"), []byte("0x8086\n"), 0644))

	// VMBus device → not VFIO
	hwAddr, _ := net.ParseMAC("aa:bb:cc:dd:ee:ff")
	netInfo := NetworkInfo{
		Iface: NetlinkIface{
			LinkAttrs: netlink.LinkAttrs{
				Name:         "eth1",
				ParentDevBus: "vmbus",
				ParentDev:    guid,
				HardwareAddr: hwAddr,
			},
			Type: "",
		},
		Link: &netlink.Dummy{
			LinkAttrs: netlink.LinkAttrs{
				Name:         "eth1",
				ParentDevBus: "vmbus",
				ParentDev:    guid,
				HardwareAddr: hwAddr,
			},
		},
	}

	ep, err := createPhysicalEndpoint(0, netInfo, false, DefaultNetInterworkingModel)
	assert.NoError(err)
	assert.NotNil(ep)
	assert.False(ep.IsVFIO)
	assert.Equal("eth1", ep.IfaceName)
	assert.Equal("hv_netvsc", ep.Driver)
	assert.Equal(guid, ep.BDF)
	assert.Equal("vmbus", ep.BusType)
	assert.Equal("eth1", ep.NetPair.VirtIface.Name)
}
