//go:build linux

// Copyright (c) 2025 Datadog, Inc
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"net"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/vishvananda/netlink"

	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

func TestCreateNetkitNetworkEndpoint(t *testing.T) {
	assert := assert.New(t)
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	expected := &NetkitEndpoint{
		NetPair: NetworkInterfacePair{
			TapInterface: TapInterface{
				ID:   "uniqueTestID-4",
				Name: "br4_kata",
				TAPIface: NetworkInterface{
					Name: "tap4_kata",
				},
			},
			VirtIface: NetworkInterface{
				Name:     "eth4",
				HardAddr: macAddr.String(),
			},
			NetInterworkingModel: DefaultNetInterworkingModel,
		},
		EndpointType: NetkitEndpointType,
	}

	result, err := createNetkitNetworkEndpoint(4, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	// the resulting ID  will be random - so let's overwrite to test the rest of the flow
	result.NetPair.ID = "uniqueTestID-4"

	// the resulting mac address will be random - so lets overwrite it
	result.NetPair.VirtIface.HardAddr = macAddr.String()

	assert.Exactly(result, expected)
}

func TestCreateNetkitNetworkEndpointChooseIfaceName(t *testing.T) {
	assert := assert.New(t)
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	expected := &NetkitEndpoint{
		NetPair: NetworkInterfacePair{
			TapInterface: TapInterface{
				ID:   "uniqueTestID-4",
				Name: "br4_kata",
				TAPIface: NetworkInterface{
					Name: "tap4_kata",
				},
			},
			VirtIface: NetworkInterface{
				Name:     "eth1",
				HardAddr: macAddr.String(),
			},
			NetInterworkingModel: DefaultNetInterworkingModel,
		},
		EndpointType: NetkitEndpointType,
	}

	result, err := createNetkitNetworkEndpoint(4, "eth1", DefaultNetInterworkingModel)
	assert.NoError(err)

	// the resulting ID will be random - so let's overwrite to test the rest of the flow
	result.NetPair.ID = "uniqueTestID-4"

	// the resulting mac address will be random - so lets overwrite it
	result.NetPair.VirtIface.HardAddr = macAddr.String()

	assert.Exactly(result, expected)
}

func TestCreateNetkitNetworkEndpointInvalidArgs(t *testing.T) {
	// nolint: govet
	type endpointValues struct {
		idx    int
		ifName string
	}

	assert := assert.New(t)

	// all elements are expected to result in failure
	failingValues := []endpointValues{
		{-1, "bar"},
		{-1, ""},
	}

	for _, d := range failingValues {
		_, err := createNetkitNetworkEndpoint(d.idx, d.ifName, DefaultNetInterworkingModel)
		assert.Error(err)
	}
}

func TestNetkitEndpointProperties(t *testing.T) {
	assert := assert.New(t)

	endpoint, err := createNetkitNetworkEndpoint(0, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	// Test Properties
	properties := NetworkInfo{
		Iface: NetlinkIface{
			LinkAttrs: netlink.LinkAttrs{
				Name: "test-netkit",
			},
			Type: "netkit",
		},
	}
	endpoint.SetProperties(properties)
	assert.Equal(properties, endpoint.Properties())

	// Test Name
	assert.Equal(endpoint.NetPair.VirtIface.Name, endpoint.Name())

	// Test HardwareAddr
	assert.Equal(endpoint.NetPair.TAPIface.HardAddr, endpoint.HardwareAddr())

	// Test Type
	assert.Equal(NetkitEndpointType, endpoint.Type())

	// Test NetworkPair
	netPair := endpoint.NetworkPair()
	assert.NotNil(netPair)
	assert.Equal(&endpoint.NetPair, netPair)
}

func TestNetkitEndpointPciPath(t *testing.T) {
	assert := assert.New(t)

	endpoint, err := createNetkitNetworkEndpoint(0, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	// Test PciPath get/set
	testPciPath := vcTypes.PciPath{Bus: "0x01", Device: "0x02", Function: "0x03"}
	endpoint.SetPciPath(testPciPath)
	assert.Equal(testPciPath, endpoint.PciPath())
}

func TestNetkitEndpointCcwDevice(t *testing.T) {
	assert := assert.New(t)

	endpoint, err := createNetkitNetworkEndpoint(0, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	// Test CcwDevice get/set
	testCcwDev := vcTypes.CcwDevice{DevNo: "fe.0.0001"}
	endpoint.SetCcwDevice(testCcwDev)
	ccwDevice := endpoint.CcwDevice()
	assert.NotNil(ccwDevice)
	assert.Equal(testCcwDev, *ccwDevice)
}

func TestNetkitEndpointRateLimiters(t *testing.T) {
	assert := assert.New(t)

	endpoint, err := createNetkitNetworkEndpoint(0, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	// Test RxRateLimiter
	assert.False(endpoint.GetRxRateLimiter())
	err = endpoint.SetRxRateLimiter()
	assert.NoError(err)
	assert.True(endpoint.GetRxRateLimiter())

	// Test TxRateLimiter
	assert.False(endpoint.GetTxRateLimiter())
	err = endpoint.SetTxRateLimiter()
	assert.NoError(err)
	assert.True(endpoint.GetTxRateLimiter())
}

func TestNetkitEndpointSaveLoad(t *testing.T) {
	assert := assert.New(t)
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x05}

	endpoint := &NetkitEndpoint{
		NetPair: NetworkInterfacePair{
			TapInterface: TapInterface{
				ID:   "test-netkit-id",
				Name: "br5_kata",
				TAPIface: NetworkInterface{
					Name:     "tap5_kata",
					HardAddr: macAddr.String(),
				},
			},
			VirtIface: NetworkInterface{
				Name:     "eth5",
				HardAddr: macAddr.String(),
			},
			NetInterworkingModel: NetXConnectTCFilterModel,
		},
		EndpointType: NetkitEndpointType,
	}

	// Save the endpoint
	saved := endpoint.save()
	assert.Equal(string(NetkitEndpointType), saved.Type)
	assert.NotNil(saved.Netkit)
	assert.Equal("test-netkit-id", saved.Netkit.NetPair.TapInterface.ID)
	assert.Equal("br5_kata", saved.Netkit.NetPair.TapInterface.Name)
	assert.Equal("eth5", saved.Netkit.NetPair.VirtIface.Name)

	// Load into a new endpoint
	newEndpoint := &NetkitEndpoint{}
	newEndpoint.load(saved)

	assert.Equal(NetkitEndpointType, newEndpoint.EndpointType)
	assert.Equal(endpoint.NetPair.ID, newEndpoint.NetPair.ID)
	assert.Equal(endpoint.NetPair.Name, newEndpoint.NetPair.Name)
	assert.Equal(endpoint.NetPair.TAPIface.Name, newEndpoint.NetPair.TAPIface.Name)
	assert.Equal(endpoint.NetPair.VirtIface.Name, newEndpoint.NetPair.VirtIface.Name)
	assert.Equal(endpoint.NetPair.NetInterworkingModel, newEndpoint.NetPair.NetInterworkingModel)
}
