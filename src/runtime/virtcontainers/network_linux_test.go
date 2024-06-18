// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"encoding/json"
	"net"
	"os"
	"reflect"
	"testing"

	"golang.org/x/sys/unix"

	"github.com/containernetworking/plugins/pkg/ns"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	vctypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/stretchr/testify/assert"
	"github.com/vishvananda/netlink"
)

func TestGenerateInterfacesAndRoutes(t *testing.T) {
	//
	//Create a couple of addresses
	//
	address1 := &net.IPNet{IP: net.IPv4(172, 17, 0, 2), Mask: net.CIDRMask(16, 32)}
	address2 := &net.IPNet{IP: net.IPv4(182, 17, 0, 2), Mask: net.CIDRMask(16, 32)}
	address3 := &net.IPNet{IP: net.ParseIP("2001:db8:1::242:ac11:2"), Mask: net.CIDRMask(64, 128)}

	addrs := []netlink.Addr{
		{IPNet: address1, Label: "phyaddr1"},
		{IPNet: address2, Label: "phyaddr2"},
		{IPNet: address3, Label: "phyaddr3"},
	}

	// Create a couple of routes:
	dst2 := &net.IPNet{IP: net.IPv4(172, 17, 0, 0), Mask: net.CIDRMask(16, 32)}
	src2 := net.IPv4(172, 17, 0, 2)
	gw2 := net.IPv4(172, 17, 0, 1)

	dstV6 := &net.IPNet{IP: net.ParseIP("2001:db8:1::"), Mask: net.CIDRMask(64, 128)}
	gatewayV6 := net.ParseIP("2001:db8:1::1")

	routes := []netlink.Route{
		{LinkIndex: 329, Dst: nil, Src: nil, Gw: net.IPv4(172, 17, 0, 1), Scope: netlink.Scope(254)},
		{LinkIndex: 329, Dst: dst2, Src: src2, Gw: gw2},
		{LinkIndex: 329, Dst: dstV6, Src: nil, Gw: nil},
		{LinkIndex: 329, Dst: nil, Src: nil, Gw: gatewayV6},
	}

	arpMAC, _ := net.ParseMAC("6a:92:3a:59:70:aa")

	neighs := []netlink.Neigh{
		{LinkIndex: 329, IP: net.IPv4(192, 168, 0, 101), State: netlink.NUD_PERMANENT, HardwareAddr: arpMAC},
	}

	networkInfo := NetworkInfo{
		Iface: NetlinkIface{
			LinkAttrs: netlink.LinkAttrs{MTU: 1500},
			Type:      "",
		},
		Addrs:     addrs,
		Routes:    routes,
		Neighbors: neighs,
	}

	ep0 := &PhysicalEndpoint{
		IfaceName:          "eth0",
		HardAddr:           net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
		EndpointProperties: networkInfo,
	}

	endpoints := []Endpoint{ep0}

	nns, err := NewNetwork(&NetworkConfig{NetworkID: "foobar", NetworkCreated: true})
	assert.Nil(t, err)
	nns.SetEndpoints(endpoints)

	resInterfaces, resRoutes, resNeighs, err := generateVCNetworkStructures(context.Background(), nns.Endpoints())

	//
	// Build expected results:
	//
	expectedAddresses := []*pbTypes.IPAddress{
		{Family: utils.ConvertAddressFamily(netlink.FAMILY_V4), Address: "172.17.0.2", Mask: "16"},
		{Family: utils.ConvertAddressFamily(netlink.FAMILY_V4), Address: "182.17.0.2", Mask: "16"},
		{Family: utils.ConvertAddressFamily(netlink.FAMILY_V6), Address: "2001:db8:1::242:ac11:2", Mask: "64"},
	}

	expectedInterfaces := []*pbTypes.Interface{
		{Device: "eth0", Name: "eth0", IPAddresses: expectedAddresses, Mtu: 1500, HwAddr: "02:00:ca:fe:00:04"},
	}

	expectedRoutes := []*pbTypes.Route{
		{Dest: "", Gateway: "172.17.0.1", Device: "eth0", Source: "", Scope: uint32(254)},
		{Dest: "172.17.0.0/16", Gateway: "172.17.0.1", Device: "eth0", Source: "172.17.0.2"},
		{Dest: "2001:db8:1::/64", Gateway: "", Device: "eth0", Source: ""},
		{Dest: "", Gateway: "2001:db8:1::1", Device: "eth0", Source: ""},
	}

	expectedNeighs := []*pbTypes.ARPNeighbor{
		{
			Device:      "eth0",
			State:       netlink.NUD_PERMANENT,
			Lladdr:      "6a:92:3a:59:70:aa",
			ToIPAddress: &pbTypes.IPAddress{Address: "192.168.0.101", Family: utils.ConvertAddressFamily(netlink.FAMILY_V4)},
		},
	}

	assert.Nil(t, err, "unexpected failure when calling generateKataInterfacesAndRoutes")
	assert.True(t, reflect.DeepEqual(resInterfaces, expectedInterfaces),
		"Interfaces returned didn't match: got %+v, expecting %+v", resInterfaces, expectedInterfaces)
	assert.True(t, reflect.DeepEqual(resRoutes, expectedRoutes),
		"Routes returned didn't match: got %+v, expecting %+v", resRoutes, expectedRoutes)
	assert.True(t, reflect.DeepEqual(resNeighs, expectedNeighs),
		"ARP Neighbors returned didn't match: got %+v, expecting %+v", resNeighs, expectedNeighs)
}

func TestCreateGetTunTapLink(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	assert := assert.New(t)

	netHandle, err := netlink.NewHandle()
	assert.NoError(err)
	defer netHandle.Close()

	assert.NoError(err)

	tapName := "testtap0"
	tapLink, fds, err := createLink(netHandle, tapName, &netlink.Tuntap{}, 1)
	assert.NoError(err)
	assert.NotNil(tapLink)
	assert.NotZero(len(fds))

	tapLink, err = getLinkByName(netHandle, tapName, &netlink.Tuntap{})
	assert.NoError(err)

	err = netHandle.LinkDel(tapLink)
	assert.NoError(err)
}

func TestCreateMacVtap(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	assert := assert.New(t)

	netHandle, err := netlink.NewHandle()
	assert.NoError(err)
	defer netHandle.Close()

	assert.NoError(err)

	tapName := "testtap0"
	tapLink, _, err := createLink(netHandle, tapName, &netlink.Tuntap{}, 1)
	assert.NoError(err)

	attrs := tapLink.Attrs()

	mcLink := &netlink.Macvtap{
		Macvlan: netlink.Macvlan{
			LinkAttrs: netlink.LinkAttrs{
				TxQLen:      attrs.TxQLen,
				ParentIndex: attrs.Index,
			},
		},
	}

	macvtapName := "testmc0"
	_, err = createMacVtap(netHandle, macvtapName, mcLink, 1)
	assert.NoError(err)

	macvtapLink, err := getLinkByName(netHandle, macvtapName, &netlink.Macvtap{})
	assert.NoError(err)

	err = netHandle.LinkDel(macvtapLink)
	assert.NoError(err)

	tapLink, err = getLinkByName(netHandle, tapName, &netlink.Tuntap{})
	assert.NoError(err)

	err = netHandle.LinkDel(tapLink)
	assert.NoError(err)
}

func TestTcRedirectNetwork(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	assert := assert.New(t)

	netHandle, err := netlink.NewHandle()
	assert.NoError(err)
	defer netHandle.Close()

	// Create a test veth interface.
	vethName := "foo"
	veth := &netlink.Veth{LinkAttrs: netlink.LinkAttrs{Name: vethName, TxQLen: 200, MTU: 1400}, PeerName: "bar"}

	err = netlink.LinkAdd(veth)
	assert.NoError(err)

	endpoint, err := createVethNetworkEndpoint(1, vethName, NetXConnectTCFilterModel)
	assert.NoError(err)

	link, err := netlink.LinkByName(vethName)
	assert.NoError(err)

	err = netHandle.LinkSetUp(link)
	assert.NoError(err)

	err = setupTCFiltering(context.Background(), endpoint, 1, true)
	assert.NoError(err)

	err = removeTCFiltering(context.Background(), endpoint)
	assert.NoError(err)

	// Remove the veth created for testing.
	err = netHandle.LinkDel(link)
	assert.NoError(err)
}

func TestRxRateLimiter(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	assert := assert.New(t)

	netHandle, err := netlink.NewHandle()
	assert.NoError(err)
	defer netHandle.Close()

	// Create a test veth interface.
	vethName := "foo"
	veth := &netlink.Veth{LinkAttrs: netlink.LinkAttrs{Name: vethName, TxQLen: 200, MTU: 1400}, PeerName: "bar"}

	err = netlink.LinkAdd(veth)
	assert.NoError(err)

	endpoint, err := createVethNetworkEndpoint(1, vethName, NetXConnectTCFilterModel)
	assert.NoError(err)

	link, err := netlink.LinkByName(vethName)
	assert.NoError(err)

	err = netHandle.LinkSetUp(link)
	assert.NoError(err)

	err = setupTCFiltering(context.Background(), endpoint, 1, true)
	assert.NoError(err)

	// 10Mb
	maxRate := uint64(10000000)
	err = addRxRateLimiter(endpoint, maxRate)
	assert.NoError(err)

	currentNS, err := ns.GetCurrentNS()
	assert.NoError(err)

	err = removeRxRateLimiter(endpoint, currentNS.Path())
	assert.NoError(err)

	err = removeTCFiltering(context.Background(), endpoint)
	assert.NoError(err)

	// Remove the veth created for testing.
	err = netHandle.LinkDel(link)
	assert.NoError(err)
}

func TestTxRateLimiter(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	assert := assert.New(t)

	netHandle, err := netlink.NewHandle()
	assert.NoError(err)
	defer netHandle.Close()

	// Create a test veth interface.
	vethName := "foo"
	veth := &netlink.Veth{LinkAttrs: netlink.LinkAttrs{Name: vethName, TxQLen: 200, MTU: 1400}, PeerName: "bar"}

	err = netlink.LinkAdd(veth)
	assert.NoError(err)

	endpoint, err := createVethNetworkEndpoint(1, vethName, NetXConnectTCFilterModel)
	assert.NoError(err)

	link, err := netlink.LinkByName(vethName)
	assert.NoError(err)

	err = netHandle.LinkSetUp(link)
	assert.NoError(err)

	err = setupTCFiltering(context.Background(), endpoint, 1, true)
	assert.NoError(err)

	// 10Mb
	maxRate := uint64(10000000)
	err = addTxRateLimiter(endpoint, maxRate)
	assert.NoError(err)

	currentNS, err := ns.GetCurrentNS()
	assert.NoError(err)

	err = removeTxRateLimiter(endpoint, currentNS.Path())
	assert.NoError(err)

	err = removeTCFiltering(context.Background(), endpoint)
	assert.NoError(err)

	// Remove the veth created for testing.
	err = netHandle.LinkDel(link)
	assert.NoError(err)
}

func TestConvertDanDeviceToNetworkInfo(t *testing.T) {

	jsonData, err := os.ReadFile("testdata/dan-config.json")
	assert.NoError(t, err)
	var config vctypes.DanConfig
	err = json.Unmarshal([]byte(jsonData), &config)
	assert.NoError(t, err)

	ni, err := convertDanDeviceToNetworkInfo(&config.Devices[0])
	assert.NoError(t, err)
	assert.Equal(t, 1500, ni.Iface.MTU)

	assert.Len(t, ni.Addrs, 1)
	assert.Equal(t, "10.10.0.5/24", ni.Addrs[0].String())

	dest, _ := netlink.ParseIPNet("10.10.0.0/16")
	dest.IP = dest.IP.To4()
	routes := []netlink.Route{
		{Family: unix.AF_INET, Dst: nil, Gw: net.ParseIP("10.0.0.1"), Src: nil, Scope: 0},
		{Family: unix.AF_INET, Dst: dest, Gw: net.ParseIP("10.0.0.1"), Src: nil, Scope: 0},
	}
	assert.Equal(t, routes, ni.Routes)

	neighMac, _ := net.ParseMAC("0a:58:0a:0a:0a:0a")
	neigh := netlink.Neigh{
		HardwareAddr: neighMac,
		IP:           net.ParseIP("10.10.10.10"),
	}
	assert.Len(t, ni.Neighbors, 1)
	assert.Equal(t, neigh, ni.Neighbors[0])
}

func TestAddEndpoints_Dan(t *testing.T) {

	network := &LinuxNetwork{
		"net-123",
		[]Endpoint{},
		NetXConnectDefaultModel,
		true,
		"testdata/dan-config.json",
	}

	ctx := context.TODO()
	eps, err := network.AddEndpoints(ctx, nil, nil, true)
	assert.NoError(t, err)
	assert.Len(t, eps, 1)

	ep := eps[0]
	assert.Equal(t, ep.Name(), "eth0")
	assert.Equal(t, ep.HardwareAddr(), "0a:58:0a:0a:00:05")
	assert.Equal(t, ep.Type(), VfioEndpointType)
	assert.Equal(t, ep.PciPath().String(), "")
}
