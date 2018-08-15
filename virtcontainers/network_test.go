// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"io/ioutil"
	"net"
	"os"
	"path/filepath"
	"reflect"
	"syscall"
	"testing"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/kata-containers/agent/protocols/grpc"
	"github.com/stretchr/testify/assert"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
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

func testEndpointTypeSet(t *testing.T, value string, expected EndpointType) {
	//var netModel NetworkModel
	var endpointType EndpointType

	err := endpointType.Set(value)
	if err != nil {
		t.Fatal(err)
	}

	if endpointType != expected {
		t.Fatal()
	}
}

func TestPhysicalEndpointTypeSet(t *testing.T) {
	testEndpointTypeSet(t, "physical", PhysicalEndpointType)
}

func TestVirtualEndpointTypeSet(t *testing.T) {
	testEndpointTypeSet(t, "virtual", VirtualEndpointType)
}

func TestEndpointTypeSetFailure(t *testing.T) {
	var endpointType EndpointType

	err := endpointType.Set("wrong-value")
	if err == nil {
		t.Fatal(err)
	}
}

func testEndpointTypeString(t *testing.T, endpointType *EndpointType, expected string) {
	result := endpointType.String()

	if result != expected {
		t.Fatal()
	}
}

func TestPhysicalEndpointTypeString(t *testing.T) {
	endpointType := PhysicalEndpointType
	testEndpointTypeString(t, &endpointType, string(PhysicalEndpointType))
}

func TestVirtualEndpointTypeString(t *testing.T) {
	endpointType := VirtualEndpointType
	testEndpointTypeString(t, &endpointType, string(VirtualEndpointType))
}

func TestIncorrectEndpointTypeString(t *testing.T) {
	var endpointType EndpointType
	testEndpointTypeString(t, &endpointType, "")
}

func TestCreateVhostUserEndpoint(t *testing.T) {
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x48}
	ifcName := "vhost-deadbeef"
	socket := "/tmp/vhu_192.168.0.1"

	netinfo := NetworkInfo{
		Iface: NetlinkIface{
			LinkAttrs: netlink.LinkAttrs{
				HardwareAddr: macAddr,
				Name:         ifcName,
			},
		},
	}

	expected := &VhostUserEndpoint{
		SocketPath:   socket,
		HardAddr:     macAddr.String(),
		IfaceName:    ifcName,
		EndpointType: VhostUserEndpointType,
	}

	result, err := createVhostUserEndpoint(netinfo, socket)
	if err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(result, expected) == false {
		t.Fatalf("\n\tGot %v\n\tExpecting %v", result, expected)
	}
}

func TestCreateVirtualNetworkEndpoint(t *testing.T) {
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	expected := &VirtualEndpoint{
		NetPair: NetworkInterfacePair{
			ID:   "uniqueTestID-4",
			Name: "br4",
			VirtIface: NetworkInterface{
				Name:     "eth4",
				HardAddr: macAddr.String(),
			},
			TAPIface: NetworkInterface{
				Name: "tap4",
			},
			NetInterworkingModel: DefaultNetInterworkingModel,
		},
		EndpointType: VirtualEndpointType,
	}

	result, err := createVirtualNetworkEndpoint(4, "", DefaultNetInterworkingModel)
	if err != nil {
		t.Fatal(err)
	}

	// the resulting ID  will be random - so let's overwrite to test the rest of the flow
	result.NetPair.ID = "uniqueTestID-4"

	if reflect.DeepEqual(result, expected) == false {
		t.Fatal()
	}
}

func TestCreateVirtualNetworkEndpointChooseIfaceName(t *testing.T) {
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	expected := &VirtualEndpoint{
		NetPair: NetworkInterfacePair{
			ID:   "uniqueTestID-4",
			Name: "br4",
			VirtIface: NetworkInterface{
				Name:     "eth1",
				HardAddr: macAddr.String(),
			},
			TAPIface: NetworkInterface{
				Name: "tap4",
			},
			NetInterworkingModel: DefaultNetInterworkingModel,
		},
		EndpointType: VirtualEndpointType,
	}

	result, err := createVirtualNetworkEndpoint(4, "eth1", DefaultNetInterworkingModel)
	if err != nil {
		t.Fatal(err)
	}

	// the resulting ID will be random - so let's overwrite to test the rest of the flow
	result.NetPair.ID = "uniqueTestID-4"

	if reflect.DeepEqual(result, expected) == false {
		t.Fatal()
	}
}

func TestCreateVirtualNetworkEndpointInvalidArgs(t *testing.T) {
	type endpointValues struct {
		idx    int
		ifName string
	}

	// all elements are expected to result in failure
	failingValues := []endpointValues{
		{-1, "bar"},
		{-1, ""},
	}

	for _, d := range failingValues {
		result, err := createVirtualNetworkEndpoint(d.idx, d.ifName, DefaultNetInterworkingModel)
		if err == nil {
			t.Fatalf("expected invalid endpoint for %v, got %v", d, result)
		}
	}
}

func TestIsPhysicalIface(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	testNetIface := "testIface0"
	testMTU := 1500
	testMACAddr := "00:00:00:00:00:01"

	hwAddr, err := net.ParseMAC(testMACAddr)
	if err != nil {
		t.Fatal(err)
	}

	link := &netlink.Bridge{
		LinkAttrs: netlink.LinkAttrs{
			Name:         testNetIface,
			MTU:          testMTU,
			HardwareAddr: hwAddr,
			TxQLen:       -1,
		},
	}

	n, err := ns.NewNS()
	if err != nil {
		t.Fatal(err)
	}
	defer n.Close()

	netnsHandle, err := netns.GetFromPath(n.Path())
	if err != nil {
		t.Fatal(err)
	}
	defer netnsHandle.Close()

	netlinkHandle, err := netlink.NewHandleAt(netnsHandle)
	if err != nil {
		t.Fatal(err)
	}
	defer netlinkHandle.Delete()

	if err := netlinkHandle.LinkAdd(link); err != nil {
		t.Fatal(err)
	}

	var isPhysical bool
	err = doNetNS(n.Path(), func(_ ns.NetNS) error {
		var err error
		isPhysical, err = isPhysicalIface(testNetIface)
		return err
	})

	if err != nil {
		t.Fatal(err)
	}

	if isPhysical == true {
		t.Fatalf("Got %+v\nExpecting %+v", isPhysical, false)
	}
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
		{"default Model", "default", false},
		{"bridged Model", "bridged", false},
		{"macvtap Model", "macvtap", false},
		{"enlightened Model", "enlightened", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if err := n.SetModel(tt.modelName); (err != nil) != tt.wantErr {
				t.Errorf("NetInterworkingModel.SetModel() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestVhostUserSocketPath(t *testing.T) {

	// First test case: search for existing:
	addresses := []netlink.Addr{
		{
			IPNet: &net.IPNet{
				IP:   net.IPv4(192, 168, 0, 2),
				Mask: net.IPv4Mask(192, 168, 0, 2),
			},
		},
		{
			IPNet: &net.IPNet{
				IP:   net.IPv4(192, 168, 0, 1),
				Mask: net.IPv4Mask(192, 168, 0, 1),
			},
		},
	}

	expectedPath := "/tmp/vhostuser_192.168.0.1"
	expectedFileName := "vhu.sock"
	expectedResult := fmt.Sprintf("%s/%s", expectedPath, expectedFileName)

	err := os.Mkdir(expectedPath, 0777)
	if err != nil {
		t.Fatal(err)
	}

	_, err = os.Create(expectedResult)
	if err != nil {
		t.Fatal(err)
	}
	netinfo := NetworkInfo{
		Addrs: addresses,
	}

	path, _ := vhostUserSocketPath(netinfo)

	if path != expectedResult {
		t.Fatalf("Got %+v\nExpecting %+v", path, expectedResult)
	}

	// Second test case: search doesn't include matching vsock:
	addressesFalse := []netlink.Addr{
		{
			IPNet: &net.IPNet{
				IP:   net.IPv4(192, 168, 0, 4),
				Mask: net.IPv4Mask(192, 168, 0, 4),
			},
		},
	}
	netinfoFail := NetworkInfo{
		Addrs: addressesFalse,
	}

	path, _ = vhostUserSocketPath(netinfoFail)
	if path != "" {
		t.Fatalf("Got %+v\nExpecting %+v", path, "")
	}

	err = os.Remove(expectedResult)
	if err != nil {
		t.Fatal(err)
	}

	err = os.Remove(expectedPath)
	if err != nil {
		t.Fatal(err)
	}

}

func TestVhostUserEndpointAttach(t *testing.T) {
	v := &VhostUserEndpoint{
		SocketPath:   "/tmp/sock",
		HardAddr:     "mac-addr",
		EndpointType: VhostUserEndpointType,
	}

	h := &mockHypervisor{}

	err := v.Attach(h)
	if err != nil {
		t.Fatal(err)
	}
}

func TestVhostUserEndpoint_HotAttach(t *testing.T) {
	assert := assert.New(t)
	v := &VhostUserEndpoint{
		SocketPath:   "/tmp/sock",
		HardAddr:     "mac-addr",
		EndpointType: VhostUserEndpointType,
	}

	h := &mockHypervisor{}

	err := v.HotAttach(h)
	assert.Error(err)
}

func TestVhostUserEndpoint_HotDetach(t *testing.T) {
	assert := assert.New(t)
	v := &VhostUserEndpoint{
		SocketPath:   "/tmp/sock",
		HardAddr:     "mac-addr",
		EndpointType: VhostUserEndpointType,
	}

	h := &mockHypervisor{}

	err := v.HotDetach(h, true, "")
	assert.Error(err)
}

func TestPhysicalEndpoint_HotAttach(t *testing.T) {
	assert := assert.New(t)
	v := &PhysicalEndpoint{
		IfaceName: "eth0",
		HardAddr:  net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
	}

	h := &mockHypervisor{}

	err := v.HotAttach(h)
	assert.Error(err)
}

func TestPhysicalEndpoint_HotDetach(t *testing.T) {
	assert := assert.New(t)
	v := &PhysicalEndpoint{
		IfaceName: "eth0",
		HardAddr:  net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
	}

	h := &mockHypervisor{}

	err := v.HotDetach(h, true, "")
	assert.Error(err)
}

func TestGetNetNsFromBindMount(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	mountFile := filepath.Join(tmpdir, "mountInfo")
	nsPath := filepath.Join(tmpdir, "ns123")

	// Non-existent namespace path
	_, err = getNetNsFromBindMount(nsPath, mountFile)
	assert.NotNil(err)

	tmpNSPath := filepath.Join(tmpdir, "testNetNs")
	f, err := os.Create(tmpNSPath)
	assert.NoError(err)
	defer f.Close()

	type testData struct {
		contents       string
		expectedResult string
	}

	data := []testData{
		{fmt.Sprintf("711 26 0:3 net:[4026532008] %s rw shared:535 - nsfs nsfs rw", tmpNSPath), "net:[4026532008]"},
		{"711 26 0:3 net:[4026532008] /run/netns/ns123 rw shared:535 - tmpfs tmpfs rw", ""},
		{"a a a a a a a - b c d", ""},
		{"", ""},
	}

	for i, d := range data {
		err := ioutil.WriteFile(mountFile, []byte(d.contents), 0640)
		assert.NoError(err)

		path, err := getNetNsFromBindMount(tmpNSPath, mountFile)
		assert.NoError(err, fmt.Sprintf("got %q, test data: %+v", path, d))

		assert.Equal(d.expectedResult, path, "Test %d, expected %s, got %s", i, d.expectedResult, path)
	}
}

func TestHostNetworkingRequested(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	assert := assert.New(t)

	// Network namespace same as the host
	selfNsPath := "/proc/self/ns/net"
	isHostNs, err := hostNetworkingRequested(selfNsPath)
	assert.NoError(err)
	assert.True(isHostNs)

	// Non-existent netns path
	nsPath := "/proc/123/ns/net"
	_, err = hostNetworkingRequested(nsPath)
	assert.Error(err)

	// Bind-mounted Netns
	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	// Create a bind mount to the current network namespace.
	tmpFile := filepath.Join(tmpdir, "testNetNs")
	f, err := os.Create(tmpFile)
	assert.NoError(err)
	defer f.Close()

	err = syscall.Mount(selfNsPath, tmpFile, "bind", syscall.MS_BIND, "")
	assert.Nil(err)

	isHostNs, err = hostNetworkingRequested(tmpFile)
	assert.NoError(err)
	assert.True(isHostNs)

	syscall.Unmount(tmpFile, 0)
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
	expectedAddresses := []*grpc.IPAddress{
		{Family: 0, Address: "172.17.0.2", Mask: "16"},
		{Family: 0, Address: "182.17.0.2", Mask: "16"},
	}

	expectedInterfaces := []*grpc.Interface{
		{Device: "eth0", Name: "eth0", IPAddresses: expectedAddresses, Mtu: 1500, HwAddr: "02:00:ca:fe:00:04"},
	}

	expectedRoutes := []*grpc.Route{
		{Dest: "", Gateway: "172.17.0.1", Device: "eth0", Source: "", Scope: uint32(254)},
		{Dest: "172.17.0.0/16", Gateway: "172.17.0.1", Device: "eth0", Source: "172.17.0.2"},
	}

	assert.Nil(t, err, "unexpected failure when calling generateKataInterfacesAndRoutes")
	assert.True(t, reflect.DeepEqual(resInterfaces, expectedInterfaces),
		"Interfaces returned didn't match: got %+v, expecting %+v", resInterfaces, expectedInterfaces)
	assert.True(t, reflect.DeepEqual(resRoutes, expectedRoutes),
		"Routes returned didn't match: got %+v, expecting %+v", resRoutes, expectedRoutes)

}
