// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"net"
	"reflect"
	"testing"

	"github.com/containernetworking/plugins/pkg/ns"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
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

	nns := NetworkNamespace{NetNsPath: "foobar", NetNsCreated: true, Endpoints: endpoints}

	resInterfaces, resRoutes, resNeighs, err := generateVCNetworkStructures(nns)

	//
	// Build expected results:
	//
	expectedAddresses := []*pbTypes.IPAddress{
		{Family: utils.ConvertNetlinkFamily(netlink.FAMILY_V4), Address: "172.17.0.2", Mask: "16"},
		{Family: utils.ConvertNetlinkFamily(netlink.FAMILY_V4), Address: "182.17.0.2", Mask: "16"},
		{Family: utils.ConvertNetlinkFamily(netlink.FAMILY_V6), Address: "2001:db8:1::242:ac11:2", Mask: "64"},
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
			ToIPAddress: &pbTypes.IPAddress{Address: "192.168.0.101", Family: utils.ConvertNetlinkFamily(netlink.FAMILY_V4)},
		},
	}

	for _, r := range resRoutes {
		fmt.Printf("resRoute: %+v\n", r)
	}

	assert.Nil(t, err, "unexpected failure when calling generateKataInterfacesAndRoutes")
	assert.True(t, reflect.DeepEqual(resInterfaces, expectedInterfaces),
		"Interfaces returned didn't match: got %+v, expecting %+v", resInterfaces, expectedInterfaces)
	assert.True(t, reflect.DeepEqual(resRoutes, expectedRoutes),
		"Routes returned didn't match: got %+v, expecting %+v", resRoutes, expectedRoutes)
	assert.True(t, reflect.DeepEqual(resNeighs, expectedNeighs),
		"ARP Neighbors returned didn't match: got %+v, expecting %+v", resNeighs, expectedNeighs)
}

func TestNetInterworkingModelIsValid(t *testing.T) {
	tests := []struct {
		name string
		n    NetInterworkingModel
		want bool
	}{
		{"Invalid Model", NetXConnectInvalidModel, false},
		{"Default Model", NetXConnectDefaultModel, true},
		{"TC Filter Model", NetXConnectTCFilterModel, true},
		{"Macvtap Model", NetXConnectMacVtapModel, true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.n.IsValid(); got != tt.want {
				t.Errorf("NetInterworkingModel.IsValid() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestNetInterworkingModelSetModel(t *testing.T) {
	var n NetInterworkingModel
	tests := []struct {
		name      string
		modelName string
		wantErr   bool
	}{
		{"Invalid Model", "Invalid", true},
		{"default Model", defaultNetModelStr, false},
		{"macvtap Model", macvtapNetModelStr, false},
		{"tcfilter Model", tcFilterNetModelStr, false},
		{"none Model", noneNetModelStr, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if err := n.SetModel(tt.modelName); (err != nil) != tt.wantErr {
				t.Errorf("NetInterworkingModel.SetModel() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestGenerateRandomPrivateMacAdd(t *testing.T) {
	assert := assert.New(t)

	addr1, err := generateRandomPrivateMacAddr()
	assert.NoError(err)

	_, err = net.ParseMAC(addr1)
	assert.NoError(err)

	addr2, err := generateRandomPrivateMacAddr()
	assert.NoError(err)

	_, err = net.ParseMAC(addr2)
	assert.NoError(err)

	assert.NotEqual(addr1, addr2)
}

func TestCreateGetTunTapLink(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	assert := assert.New(t)

	netHandle, err := netlink.NewHandle()
	defer netHandle.Delete()

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
	defer netHandle.Delete()

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
	defer netHandle.Delete()

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

	err = setupTCFiltering(endpoint, 1, true)
	assert.NoError(err)

	err = removeTCFiltering(endpoint)
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
	defer netHandle.Delete()

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

	err = setupTCFiltering(endpoint, 1, true)
	assert.NoError(err)

	// 10Mb
	maxRate := uint64(10000000)
	err = addRxRateLimiter(endpoint, maxRate)
	assert.NoError(err)

	currentNS, err := ns.GetCurrentNS()
	assert.NoError(err)

	err = removeRxRateLimiter(endpoint, currentNS.Path())
	assert.NoError(err)

	err = removeTCFiltering(endpoint)
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
	defer netHandle.Delete()

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

	err = setupTCFiltering(endpoint, 1, true)
	assert.NoError(err)

	// 10Mb
	maxRate := uint64(10000000)
	err = addTxRateLimiter(endpoint, maxRate)
	assert.NoError(err)

	currentNS, err := ns.GetCurrentNS()
	assert.NoError(err)

	err = removeTxRateLimiter(endpoint, currentNS.Path())
	assert.NoError(err)

	err = removeTCFiltering(endpoint)
	assert.NoError(err)

	// Remove the veth created for testing.
	err = netHandle.LinkDel(link)
	assert.NoError(err)
}
