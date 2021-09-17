// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"runtime"
	"testing"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"

	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
	"golang.org/x/sys/unix"
)

const (
	testSandboxID          = "123456789"
	testRuntimePath        = "/foo/bar/test-runtime"
	testLogLevel           = "info"
	testStorageParentPath  = "/tmp/netmon"
	testSharedFile         = "foo-shared.json"
	testWrongNetlinkFamily = -1
	testIfaceName          = "test_eth0"
	testMTU                = 12345
	testHwAddr             = "02:00:ca:fe:00:48"
	testIPAddress          = "192.168.0.15"
	testIPAddressWithMask  = "192.168.0.15/32"
	testIP6Address         = "2001:db8:1::242:ac11:2"
	testIP6AddressWithMask = "2001:db8:1::/64"
	testScope              = 1
	testTxQLen             = -1
	testIfaceIndex         = 5
)

func skipUnlessRoot(t *testing.T) {
	tc := ktu.NewTestConstraint(false)

	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip("Test disabled as requires root user")
	}
}

func TestNewNetmon(t *testing.T) {
	skipUnlessRoot(t)

	// Override storageParentPath
	savedStorageParentPath := storageParentPath
	storageParentPath = testStorageParentPath
	defer func() {
		storageParentPath = savedStorageParentPath
	}()

	params := netmonParams{
		sandboxID:   testSandboxID,
		runtimePath: testRuntimePath,
		debug:       true,
		logLevel:    testLogLevel,
	}

	expected := &netmon{
		netmonParams: params,
		storagePath:  filepath.Join(storageParentPath, params.sandboxID),
		sharedFile:   filepath.Join(storageParentPath, params.sandboxID, sharedFile),
	}

	os.RemoveAll(expected.storagePath)

	got, err := newNetmon(params)
	assert.Nil(t, err)
	assert.True(t, reflect.DeepEqual(expected.netmonParams, got.netmonParams),
		"Got %+v\nExpected %+v", got.netmonParams, expected.netmonParams)
	assert.True(t, reflect.DeepEqual(expected.storagePath, got.storagePath),
		"Got %+v\nExpected %+v", got.storagePath, expected.storagePath)
	assert.True(t, reflect.DeepEqual(expected.sharedFile, got.sharedFile),
		"Got %+v\nExpected %+v", got.sharedFile, expected.sharedFile)

	_, err = os.Stat(got.storagePath)
	assert.Nil(t, err)

	os.RemoveAll(got.storagePath)
}

func TestNewNetmonErrorWrongFamilyType(t *testing.T) {
	// Override netlinkFamily
	savedNetlinkFamily := netlinkFamily
	netlinkFamily = testWrongNetlinkFamily
	defer func() {
		netlinkFamily = savedNetlinkFamily
	}()

	n, err := newNetmon(netmonParams{})
	assert.NotNil(t, err)
	assert.Nil(t, n)
}

func TestCleanup(t *testing.T) {
	skipUnlessRoot(t)

	// Override storageParentPath
	savedStorageParentPath := storageParentPath
	storageParentPath = testStorageParentPath
	defer func() {
		storageParentPath = savedStorageParentPath
	}()

	handler, err := netlink.NewHandle(netlinkFamily)
	assert.Nil(t, err)

	n := &netmon{
		storagePath: filepath.Join(storageParentPath, testSandboxID),
		linkDoneCh:  make(chan struct{}),
		rtDoneCh:    make(chan struct{}),
		netHandler:  handler,
	}

	err = os.MkdirAll(n.storagePath, storageDirPerm)
	assert.Nil(t, err)
	_, err = os.Stat(n.storagePath)
	assert.Nil(t, err)

	n.cleanup()

	_, err = os.Stat(n.storagePath)
	assert.NotNil(t, err)
	_, ok := (<-n.linkDoneCh)
	assert.False(t, ok)
	_, ok = (<-n.rtDoneCh)
	assert.False(t, ok)
}

func TestLogger(t *testing.T) {
	fields := logrus.Fields{
		"name":    netmonName,
		"pid":     os.Getpid(),
		"source":  "netmon",
		"sandbox": testSandboxID,
	}

	expected := netmonLog.WithFields(fields)

	n := &netmon{
		netmonParams: netmonParams{
			sandboxID: testSandboxID,
		},
	}

	got := n.logger()
	assert.True(t, reflect.DeepEqual(*expected, *got),
		"Got %+v\nExpected %+v", *got, *expected)
}

func TestConvertInterface(t *testing.T) {
	hwAddr, err := net.ParseMAC(testHwAddr)
	assert.Nil(t, err)

	addrs := []netlink.Addr{
		{
			IPNet: &net.IPNet{
				IP: net.ParseIP(testIPAddress),
			},
		},
		{
			IPNet: &net.IPNet{
				IP: net.ParseIP(testIP6Address),
			},
		},
	}

	linkAttrs := &netlink.LinkAttrs{
		Name:         testIfaceName,
		MTU:          testMTU,
		HardwareAddr: hwAddr,
	}

	linkType := "link_type_test"

	expected := pbTypes.Interface{
		Device: testIfaceName,
		Name:   testIfaceName,
		Mtu:    uint64(testMTU),
		HwAddr: testHwAddr,
		IPAddresses: []*pbTypes.IPAddress{
			{
				Family:  utils.ConvertNetlinkFamily(netlink.FAMILY_V4),
				Address: testIPAddress,
				Mask:    "0",
			},
			{
				Family:  utils.ConvertNetlinkFamily(netlink.FAMILY_V6),
				Address: testIP6Address,
				Mask:    "0",
			},
		},
		Type: linkType,
	}

	got := convertInterface(linkAttrs, linkType, addrs)

	assert.True(t, reflect.DeepEqual(expected, got),
		"Got %+v\nExpected %+v", got, expected)
}

func TestConvertRoutes(t *testing.T) {
	ip, ipNet, err := net.ParseCIDR(testIPAddressWithMask)
	assert.Nil(t, err)
	assert.NotNil(t, ipNet)

	_, ip6Net, err := net.ParseCIDR(testIP6AddressWithMask)
	assert.Nil(t, err)
	assert.NotNil(t, ipNet)

	routes := []netlink.Route{
		{
			Dst:       ipNet,
			Src:       ip,
			Gw:        ip,
			LinkIndex: -1,
			Scope:     testScope,
		},
		{
			Dst:       ip6Net,
			Src:       nil,
			Gw:        nil,
			LinkIndex: -1,
			Scope:     testScope,
		},
	}

	expected := []pbTypes.Route{
		{
			Dest:    testIPAddressWithMask,
			Gateway: testIPAddress,
			Source:  testIPAddress,
			Scope:   uint32(testScope),
		},
		{
			Dest:    testIP6AddressWithMask,
			Gateway: "",
			Source:  "",
			Scope:   uint32(testScope),
		},
	}

	got := convertRoutes(routes)
	assert.True(t, reflect.DeepEqual(expected, got),
		"Got %+v\nExpected %+v", got, expected)
}

type testTeardownNetwork func()

func testSetupNetwork(t *testing.T) testTeardownNetwork {
	skipUnlessRoot(t)

	// new temporary namespace so we don't pollute the host
	// lock thread since the namespace is thread local
	runtime.LockOSThread()
	var err error
	ns, err := netns.New()
	if err != nil {
		t.Fatal("Failed to create newns", ns)
	}

	return func() {
		ns.Close()
		runtime.UnlockOSThread()
	}
}

func testCreateDummyNetwork(t *testing.T, handler *netlink.Handle) (int, pbTypes.Interface) {
	hwAddr, err := net.ParseMAC(testHwAddr)
	assert.Nil(t, err)

	link := &netlink.Dummy{
		LinkAttrs: netlink.LinkAttrs{
			MTU:          testMTU,
			TxQLen:       testTxQLen,
			Name:         testIfaceName,
			HardwareAddr: hwAddr,
		},
	}

	err = handler.LinkAdd(link)
	assert.Nil(t, err)
	err = handler.LinkSetUp(link)
	assert.Nil(t, err)

	attrs := link.Attrs()
	assert.NotNil(t, attrs)

	addrs, err := handler.AddrList(link, netlinkFamily)
	assert.Nil(t, err)

	var ipAddrs []*pbTypes.IPAddress

	// Scan addresses for ipv6 link local address which is automatically assigned
	for _, addr := range addrs {
		if addr.IPNet == nil {
			continue
		}

		netMask, _ := addr.Mask.Size()

		ipAddr := &pbTypes.IPAddress{
			Address: addr.IP.String(),
			Mask:    fmt.Sprintf("%d", netMask),
		}

		if addr.IP.To4() != nil {
			ipAddr.Family = utils.ConvertNetlinkFamily(netlink.FAMILY_V4)
		} else {
			ipAddr.Family = utils.ConvertNetlinkFamily(netlink.FAMILY_V6)
		}

		ipAddrs = append(ipAddrs, ipAddr)
	}

	iface := pbTypes.Interface{
		Device:      testIfaceName,
		Name:        testIfaceName,
		Mtu:         uint64(testMTU),
		HwAddr:      testHwAddr,
		Type:        link.Type(),
		IPAddresses: ipAddrs,
	}

	return attrs.Index, iface
}

func TestScanNetwork(t *testing.T) {
	tearDownNetworkCb := testSetupNetwork(t)
	defer tearDownNetworkCb()

	handler, err := netlink.NewHandle(netlinkFamily)
	assert.Nil(t, err)
	assert.NotNil(t, handler)
	defer handler.Delete()

	idx, expected := testCreateDummyNetwork(t, handler)

	n := &netmon{
		netIfaces:  make(map[int]pbTypes.Interface),
		netHandler: handler,
	}

	err = n.scanNetwork()
	assert.Nil(t, err)
	assert.True(t, reflect.DeepEqual(expected, n.netIfaces[idx]),
		"Got %+v\nExpected %+v", n.netIfaces[idx], expected)
}

func TestStoreDataToSend(t *testing.T) {
	var got pbTypes.Interface

	expected := pbTypes.Interface{
		Device: testIfaceName,
		Name:   testIfaceName,
		Mtu:    uint64(testMTU),
		HwAddr: testHwAddr,
	}

	n := &netmon{
		sharedFile: filepath.Join(testStorageParentPath, testSharedFile),
	}

	err := os.MkdirAll(testStorageParentPath, storageDirPerm)
	defer os.RemoveAll(testStorageParentPath)
	assert.Nil(t, err)

	err = n.storeDataToSend(expected)
	assert.Nil(t, err)

	// Check the file has been created, check the content, and delete it.
	_, err = os.Stat(n.sharedFile)
	assert.Nil(t, err)
	byteArray, err := ioutil.ReadFile(n.sharedFile)
	assert.Nil(t, err)
	err = json.Unmarshal(byteArray, &got)
	assert.Nil(t, err)
	assert.True(t, reflect.DeepEqual(expected, got),
		"Got %+v\nExpected %+v", got, expected)
}

func TestExecKataCmdSuccess(t *testing.T) {
	trueBinPath, err := exec.LookPath("true")
	assert.Nil(t, err)
	assert.NotEmpty(t, trueBinPath)

	params := netmonParams{
		runtimePath: trueBinPath,
	}

	n := &netmon{
		netmonParams: params,
		sharedFile:   filepath.Join(testStorageParentPath, testSharedFile),
	}

	err = os.MkdirAll(testStorageParentPath, storageDirPerm)
	assert.Nil(t, err)
	defer os.RemoveAll(testStorageParentPath)

	file, err := os.Create(n.sharedFile)
	assert.Nil(t, err)
	assert.NotNil(t, file)
	file.Close()

	_, err = os.Stat(n.sharedFile)
	assert.Nil(t, err)

	err = n.execKataCmd("")
	assert.Nil(t, err)
	_, err = os.Stat(n.sharedFile)
	assert.NotNil(t, err)
}

func TestExecKataCmdFailure(t *testing.T) {
	falseBinPath, err := exec.LookPath("false")
	assert.Nil(t, err)
	assert.NotEmpty(t, falseBinPath)

	params := netmonParams{
		runtimePath: falseBinPath,
	}

	n := &netmon{
		netmonParams: params,
	}

	err = n.execKataCmd("")
	assert.NotNil(t, err)
}

func TestActionsCLI(t *testing.T) {
	trueBinPath, err := exec.LookPath("true")
	assert.Nil(t, err)
	assert.NotEmpty(t, trueBinPath)

	params := netmonParams{
		runtimePath: trueBinPath,
	}

	n := &netmon{
		netmonParams: params,
		sharedFile:   filepath.Join(testStorageParentPath, testSharedFile),
	}

	err = os.MkdirAll(testStorageParentPath, storageDirPerm)
	assert.Nil(t, err)
	defer os.RemoveAll(testStorageParentPath)

	// Test addInterfaceCLI
	err = n.addInterfaceCLI(pbTypes.Interface{})
	assert.Nil(t, err)

	// Test delInterfaceCLI
	err = n.delInterfaceCLI(pbTypes.Interface{})
	assert.Nil(t, err)

	// Test updateRoutesCLI
	err = n.updateRoutesCLI([]pbTypes.Route{})
	assert.Nil(t, err)

	tearDownNetworkCb := testSetupNetwork(t)
	defer tearDownNetworkCb()

	handler, err := netlink.NewHandle(netlinkFamily)
	assert.Nil(t, err)
	assert.NotNil(t, handler)
	defer handler.Delete()

	n.netHandler = handler

	// Test updateRoutes
	err = n.updateRoutes()
	assert.Nil(t, err)

	// Test handleRTMDelRoute
	err = n.handleRTMDelRoute(netlink.RouteUpdate{})
	assert.Nil(t, err)
}

func TestHandleRTMNewAddr(t *testing.T) {
	n := &netmon{}

	err := n.handleRTMNewAddr(netlink.LinkUpdate{})
	assert.Nil(t, err)
}

func TestHandleRTMDelAddr(t *testing.T) {
	n := &netmon{}

	err := n.handleRTMDelAddr(netlink.LinkUpdate{})
	assert.Nil(t, err)
}

func TestHandleRTMNewLink(t *testing.T) {
	n := &netmon{}
	ev := netlink.LinkUpdate{
		Link: &netlink.Dummy{},
	}

	// LinkAttrs is nil
	err := n.handleRTMNewLink(ev)
	assert.Nil(t, err)

	// Link name contains "kata" suffix
	ev = netlink.LinkUpdate{
		Link: &netlink.Dummy{
			LinkAttrs: netlink.LinkAttrs{
				Name: "foo_kata",
			},
		},
	}
	err = n.handleRTMNewLink(ev)
	assert.Nil(t, err)

	// Interface already exist in list
	n.netIfaces = make(map[int]pbTypes.Interface)
	n.netIfaces[testIfaceIndex] = pbTypes.Interface{}
	ev = netlink.LinkUpdate{
		Link: &netlink.Dummy{
			LinkAttrs: netlink.LinkAttrs{
				Name: "foo0",
			},
		},
	}
	ev.Index = testIfaceIndex
	err = n.handleRTMNewLink(ev)
	assert.Nil(t, err)

	// Flags are not up and running
	n.netIfaces = make(map[int]pbTypes.Interface)
	ev = netlink.LinkUpdate{
		Link: &netlink.Dummy{
			LinkAttrs: netlink.LinkAttrs{
				Name: "foo0",
			},
		},
	}
	ev.Index = testIfaceIndex
	err = n.handleRTMNewLink(ev)
	assert.Nil(t, err)

	// Invalid link
	n.netIfaces = make(map[int]pbTypes.Interface)
	ev = netlink.LinkUpdate{
		Link: &netlink.Dummy{
			LinkAttrs: netlink.LinkAttrs{
				Name: "foo0",
			},
		},
	}
	ev.Index = testIfaceIndex
	ev.Flags = unix.IFF_UP | unix.IFF_RUNNING
	handler, err := netlink.NewHandle(netlinkFamily)
	assert.Nil(t, err)
	assert.NotNil(t, handler)
	defer handler.Delete()
	n.netHandler = handler
	err = n.handleRTMNewLink(ev)
	assert.NotNil(t, err)
}

func TestHandleRTMDelLink(t *testing.T) {
	n := &netmon{}
	ev := netlink.LinkUpdate{
		Link: &netlink.Dummy{},
	}

	// LinkAttrs is nil
	err := n.handleRTMDelLink(ev)
	assert.Nil(t, err)

	// Link name contains "kata" suffix
	ev = netlink.LinkUpdate{
		Link: &netlink.Dummy{
			LinkAttrs: netlink.LinkAttrs{
				Name: "foo_kata",
			},
		},
	}
	err = n.handleRTMDelLink(ev)
	assert.Nil(t, err)

	// Interface does not exist in list
	n.netIfaces = make(map[int]pbTypes.Interface)
	ev = netlink.LinkUpdate{
		Link: &netlink.Dummy{
			LinkAttrs: netlink.LinkAttrs{
				Name: "foo0",
			},
		},
	}
	ev.Index = testIfaceIndex
	err = n.handleRTMDelLink(ev)
	assert.Nil(t, err)
}

func TestHandleRTMNewRouteIfaceNotFound(t *testing.T) {
	n := &netmon{
		netIfaces: make(map[int]pbTypes.Interface),
	}

	err := n.handleRTMNewRoute(netlink.RouteUpdate{})
	assert.Nil(t, err)
}

func TestHandleLinkEvent(t *testing.T) {
	n := &netmon{}
	ev := netlink.LinkUpdate{}

	// Unknown event
	err := n.handleLinkEvent(ev)
	assert.Nil(t, err)

	// DONE event
	ev.Header.Type = unix.NLMSG_DONE
	err = n.handleLinkEvent(ev)
	assert.Nil(t, err)

	// ERROR event
	ev.Header.Type = unix.NLMSG_ERROR
	err = n.handleLinkEvent(ev)
	assert.NotNil(t, err)

	// NEWADDR event
	ev.Header.Type = unix.RTM_NEWADDR
	err = n.handleLinkEvent(ev)
	assert.Nil(t, err)

	// DELADDR event
	ev.Header.Type = unix.RTM_DELADDR
	err = n.handleLinkEvent(ev)
	assert.Nil(t, err)

	// NEWLINK event
	ev.Header.Type = unix.RTM_NEWLINK
	ev.Link = &netlink.Dummy{}
	err = n.handleLinkEvent(ev)
	assert.Nil(t, err)

	// DELLINK event
	ev.Header.Type = unix.RTM_DELLINK
	ev.Link = &netlink.Dummy{}
	err = n.handleLinkEvent(ev)
	assert.Nil(t, err)
}

func TestHandleRouteEvent(t *testing.T) {
	n := &netmon{}
	ev := netlink.RouteUpdate{}

	// Unknown event
	err := n.handleRouteEvent(ev)
	assert.Nil(t, err)

	// RTM_NEWROUTE event
	ev.Type = unix.RTM_NEWROUTE
	err = n.handleRouteEvent(ev)
	assert.Nil(t, err)

	trueBinPath, err := exec.LookPath("true")
	assert.Nil(t, err)
	assert.NotEmpty(t, trueBinPath)

	n.runtimePath = trueBinPath
	n.sharedFile = filepath.Join(testStorageParentPath, testSharedFile)

	err = os.MkdirAll(testStorageParentPath, storageDirPerm)
	assert.Nil(t, err)
	defer os.RemoveAll(testStorageParentPath)

	tearDownNetworkCb := testSetupNetwork(t)
	defer tearDownNetworkCb()

	handler, err := netlink.NewHandle(netlinkFamily)
	assert.Nil(t, err)
	assert.NotNil(t, handler)
	defer handler.Delete()

	n.netHandler = handler

	// RTM_DELROUTE event
	ev.Type = unix.RTM_DELROUTE
	err = n.handleRouteEvent(ev)
	assert.Nil(t, err)
}
