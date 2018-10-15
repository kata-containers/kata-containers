// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"net"
	"os"
	"reflect"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/vishvananda/netlink"
)

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
