// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"net"
	"os"
	"reflect"
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/pkg/types"
	"github.com/stretchr/testify/assert"
	"github.com/vishvananda/netlink"
)

func testNetworkModelSet(t *testing.T, value string, expected NetworkModel) {
	var netModel NetworkModel

	err := netModel.Set(value)
	if err != nil {
		t.Fatal(err)
	}

	if netModel != expected {
		t.Fatal()
	}
}

func TestNoopNetworkModelSet(t *testing.T) {
	testNetworkModelSet(t, "noop", NoopNetworkModel)
}

func TestDefaultNetworkModelSet(t *testing.T) {
	testNetworkModelSet(t, "default", DefaultNetworkModel)
}

func TestNetworkModelSetFailure(t *testing.T) {
	var netModel NetworkModel

	err := netModel.Set("wrong-value")
	if err == nil {
		t.Fatal(err)
	}
}

func testNetworkModelString(t *testing.T, netModel *NetworkModel, expected string) {
	result := netModel.String()

	if result != expected {
		t.Fatal()
	}
}

func TestNoopNetworkModelString(t *testing.T) {
	netModel := NoopNetworkModel
	testNetworkModelString(t, &netModel, string(NoopNetworkModel))
}

func TestDefaultNetworkModelString(t *testing.T) {
	netModel := DefaultNetworkModel
	testNetworkModelString(t, &netModel, string(DefaultNetworkModel))
}

func TestWrongNetworkModelString(t *testing.T) {
	var netModel NetworkModel
	testNetworkModelString(t, &netModel, "")
}

func testNewNetworkFromNetworkModel(t *testing.T, netModel NetworkModel, expected interface{}) {
	result := newNetwork(netModel)

	if reflect.DeepEqual(result, expected) == false {
		t.Fatal()
	}
}

func TestNewNoopNetworkFromNetworkModel(t *testing.T) {
	testNewNetworkFromNetworkModel(t, NoopNetworkModel, &noopNetwork{})
}

func TestNewDefaultNetworkFromNetworkModel(t *testing.T) {
	testNewNetworkFromNetworkModel(t, DefaultNetworkModel, &defNetwork{})
}

func TestNewUnknownNetworkFromNetworkModel(t *testing.T) {
	var netModel NetworkModel
	testNewNetworkFromNetworkModel(t, netModel, &noopNetwork{})
}

func TestCreateDeleteNetNS(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	netNSPath, err := createNetNS()
	if err != nil {
		t.Fatal(err)
	}

	if netNSPath == "" {
		t.Fatal()
	}

	_, err = os.Stat(netNSPath)
	if err != nil {
		t.Fatal(err)
	}

	err = deleteNetNS(netNSPath)
	if err != nil {
		t.Fatal(err)
	}
}

func TestGenerateInterfacesAndRoutes(t *testing.T) {
	//
	//Create a couple of addresses
	//
	address1 := &net.IPNet{IP: net.IPv4(172, 17, 0, 2), Mask: net.CIDRMask(16, 32)}
	address2 := &net.IPNet{IP: net.IPv4(182, 17, 0, 2), Mask: net.CIDRMask(16, 32)}

	addrs := []netlink.Addr{
		{IPNet: address1, Label: "phyaddr1"},
		{IPNet: address2, Label: "phyaddr2"},
	}

	// Create a couple of routes:
	dst2 := &net.IPNet{IP: net.IPv4(172, 17, 0, 0), Mask: net.CIDRMask(16, 32)}
	src2 := net.IPv4(172, 17, 0, 2)
	gw2 := net.IPv4(172, 17, 0, 1)

	routes := []netlink.Route{
		{LinkIndex: 329, Dst: nil, Src: nil, Gw: net.IPv4(172, 17, 0, 1), Scope: netlink.Scope(254)},
		{LinkIndex: 329, Dst: dst2, Src: src2, Gw: gw2},
	}

	networkInfo := NetworkInfo{
		Iface: NetlinkIface{
			LinkAttrs: netlink.LinkAttrs{MTU: 1500},
			Type:      "",
		},
		Addrs:  addrs,
		Routes: routes,
	}

	ep0 := &PhysicalEndpoint{
		IfaceName:          "eth0",
		HardAddr:           net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
		EndpointProperties: networkInfo,
	}

	endpoints := []Endpoint{ep0}

	nns := NetworkNamespace{NetNsPath: "foobar", NetNsCreated: true, Endpoints: endpoints}

	resInterfaces, resRoutes, err := generateInterfacesAndRoutes(nns)

	//
	// Build expected results:
	//
	expectedAddresses := []*types.IPAddress{
		{Family: netlink.FAMILY_V4, Address: "172.17.0.2", Mask: "16"},
		{Family: netlink.FAMILY_V4, Address: "182.17.0.2", Mask: "16"},
	}

	expectedInterfaces := []*types.Interface{
		{Device: "eth0", Name: "eth0", IPAddresses: expectedAddresses, Mtu: 1500, HwAddr: "02:00:ca:fe:00:04"},
	}

	expectedRoutes := []*types.Route{
		{Dest: "", Gateway: "172.17.0.1", Device: "eth0", Source: "", Scope: uint32(254)},
		{Dest: "172.17.0.0/16", Gateway: "172.17.0.1", Device: "eth0", Source: "172.17.0.2"},
	}

	assert.Nil(t, err, "unexpected failure when calling generateKataInterfacesAndRoutes")
	assert.True(t, reflect.DeepEqual(resInterfaces, expectedInterfaces),
		"Interfaces returned didn't match: got %+v, expecting %+v", resInterfaces, expectedInterfaces)
	assert.True(t, reflect.DeepEqual(resRoutes, expectedRoutes),
		"Routes returned didn't match: got %+v, expecting %+v", resRoutes, expectedRoutes)

}

func TestNetInterworkingModelIsValid(t *testing.T) {
	tests := []struct {
		name string
		n    NetInterworkingModel
		want bool
	}{
		{"Invalid Model", NetXConnectInvalidModel, false},
		{"Default Model", NetXConnectDefaultModel, true},
		{"Bridged Model", NetXConnectBridgedModel, true},
		{"TC Filter Model", NetXConnectTCFilterModel, true},
		{"Macvtap Model", NetXConnectMacVtapModel, true},
		{"Enlightened Model", NetXConnectEnlightenedModel, true},
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
		{"bridged Model", bridgedNetModelStr, false},
		{"macvtap Model", macvtapNetModelStr, false},
		{"enlightened Model", enlightenedNetModelStr, false},
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

func TestCreateGetBridgeLink(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	assert := assert.New(t)

	netHandle, err := netlink.NewHandle()
	defer netHandle.Delete()

	assert.NoError(err)

	brName := "testbr0"
	brLink, _, err := createLink(netHandle, brName, &netlink.Bridge{}, 1)
	assert.NoError(err)
	assert.NotNil(brLink)

	brLink, err = getLinkByName(netHandle, brName, &netlink.Bridge{})
	assert.NoError(err)

	err = netHandle.LinkDel(brLink)
	assert.NoError(err)
}

func TestCreateGetTunTapLink(t *testing.T) {
	if os.Geteuid() != 0 {
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
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	assert := assert.New(t)

	netHandle, err := netlink.NewHandle()
	defer netHandle.Delete()

	assert.NoError(err)

	brName := "testbr0"
	brLink, _, err := createLink(netHandle, brName, &netlink.Bridge{}, 1)
	assert.NoError(err)

	attrs := brLink.Attrs()

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

	brLink, err = getLinkByName(netHandle, brName, &netlink.Bridge{})
	assert.NoError(err)

	err = netHandle.LinkDel(brLink)
	assert.NoError(err)
}

func TestTcRedirectNetwork(t *testing.T) {
	if os.Geteuid() != 0 {
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
