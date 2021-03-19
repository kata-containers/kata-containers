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
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/vishvananda/netlink"
)

func TestVhostUserSocketPath(t *testing.T) {
	assert := assert.New(t)

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
	assert.NoError(err)

	_, err = os.Create(expectedResult)
	assert.NoError(err)
	netinfo := NetworkInfo{
		Addrs: addresses,
	}

	path, _ := vhostUserSocketPath(netinfo)
	assert.Equal(path, expectedResult)

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
	assert.Empty(path)

	assert.NoError(os.Remove(expectedResult))
	assert.NoError(os.Remove(expectedPath))
}

func TestVhostUserEndpointAttach(t *testing.T) {
	assert := assert.New(t)
	v := &VhostUserEndpoint{
		SocketPath:   "/tmp/sock",
		HardAddr:     "mac-addr",
		EndpointType: VhostUserEndpointType,
	}

	s := &Sandbox{
		hypervisor: &mockHypervisor{},
	}

	err := v.Attach(context.Background(), s)
	assert.NoError(err)
}

func TestVhostUserEndpoint_HotAttach(t *testing.T) {
	assert := assert.New(t)
	v := &VhostUserEndpoint{
		SocketPath:   "/tmp/sock",
		HardAddr:     "mac-addr",
		EndpointType: VhostUserEndpointType,
	}

	h := &mockHypervisor{}

	err := v.HotAttach(context.Background(), h)
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

	err := v.HotDetach(context.Background(), h, true, "")
	assert.Error(err)
}

func TestCreateVhostUserEndpoint(t *testing.T) {
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x48}
	ifcName := "vhost-deadbeef"
	socket := "/tmp/vhu_192.168.0.1"
	assert := assert.New(t)

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
	assert.NoError(err)
	assert.Exactly(result, expected)
}
