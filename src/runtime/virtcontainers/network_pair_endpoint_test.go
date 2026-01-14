//go:build linux

// Copyright (c) 2018 Intel Corporation
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

// endpointCreator is a function type for creating network pair endpoints
type endpointCreator func(idx int, ifName string, model NetInterworkingModel) (Endpoint, error)

// testCreateNetworkPairEndpoint tests creation of network pair endpoints
func testCreateNetworkPairEndpoint(t *testing.T, createFunc endpointCreator, expectedType EndpointType) {
	assert := assert.New(t)
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	result, err := createFunc(4, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	// the resulting ID will be random - so let's overwrite to test the rest of the flow
	netPair := result.NetworkPair()
	netPair.ID = "uniqueTestID-4"

	// the resulting mac address will be random - so lets overwrite it
	netPair.VirtIface.HardAddr = macAddr.String()

	assert.Equal(expectedType, result.Type())
	assert.Equal("uniqueTestID-4", netPair.ID)
	assert.Equal("br4_kata", netPair.Name)
	assert.Equal("tap4_kata", netPair.TAPIface.Name)
	assert.Equal("eth4", netPair.VirtIface.Name)
	assert.Equal(macAddr.String(), netPair.VirtIface.HardAddr)
	assert.Equal(DefaultNetInterworkingModel, netPair.NetInterworkingModel)
}

// testCreateNetworkPairEndpointChooseIfaceName tests creation with custom interface name
func testCreateNetworkPairEndpointChooseIfaceName(t *testing.T, createFunc endpointCreator, expectedType EndpointType) {
	assert := assert.New(t)
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	result, err := createFunc(4, "eth1", DefaultNetInterworkingModel)
	assert.NoError(err)

	// the resulting ID will be random - so let's overwrite to test the rest of the flow
	netPair := result.NetworkPair()
	netPair.ID = "uniqueTestID-4"

	// the resulting mac address will be random - so lets overwrite it
	netPair.VirtIface.HardAddr = macAddr.String()

	assert.Equal(expectedType, result.Type())
	assert.Equal("uniqueTestID-4", netPair.ID)
	assert.Equal("br4_kata", netPair.Name)
	assert.Equal("tap4_kata", netPair.TAPIface.Name)
	assert.Equal("eth1", netPair.VirtIface.Name) // Custom name
	assert.Equal(macAddr.String(), netPair.VirtIface.HardAddr)
	assert.Equal(DefaultNetInterworkingModel, netPair.NetInterworkingModel)
}

// testCreateNetworkPairEndpointInvalidArgs tests creation with invalid arguments
func testCreateNetworkPairEndpointInvalidArgs(t *testing.T, createFunc endpointCreator) {
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
		_, err := createFunc(d.idx, d.ifName, DefaultNetInterworkingModel)
		assert.Error(err)
	}
}

// testNetworkPairEndpointProperties tests basic property getters/setters
func testNetworkPairEndpointProperties(t *testing.T, endpoint Endpoint, expectedType EndpointType, typeName string) {
	assert := assert.New(t)

	// Test Properties
	properties := NetworkInfo{
		Iface: NetlinkIface{
			LinkAttrs: netlink.LinkAttrs{
				Name: "test-" + typeName,
			},
			Type: typeName,
		},
	}
	endpoint.SetProperties(properties)
	assert.Equal(properties, endpoint.Properties())

	// Test Name
	netPair := endpoint.NetworkPair()
	assert.Equal(netPair.VirtIface.Name, endpoint.Name())

	// Test HardwareAddr
	assert.Equal(netPair.TAPIface.HardAddr, endpoint.HardwareAddr())

	// Test Type
	assert.Equal(expectedType, endpoint.Type())

	// Test NetworkPair
	assert.NotNil(netPair)
}

// testNetworkPairEndpointPciPath tests PCI path getters/setters
func testNetworkPairEndpointPciPath(t *testing.T, endpoint Endpoint) {
	assert := assert.New(t)

	// Test PciPath get/set
	testPciPath, err := vcTypes.PciPathFromString("01/02")
	assert.NoError(err)
	endpoint.SetPciPath(testPciPath)
	assert.Equal(testPciPath, endpoint.PciPath())
}

// testNetworkPairEndpointCcwDevice tests CCW device getters/setters
func testNetworkPairEndpointCcwDevice(t *testing.T, endpoint Endpoint) {
	assert := assert.New(t)

	// Test CcwDevice get/set
	testCcwDev, err := vcTypes.CcwDeviceFrom(0, "0001")
	assert.NoError(err)
	endpoint.SetCcwDevice(testCcwDev)
	ccwDevice := endpoint.CcwDevice()
	assert.NotNil(ccwDevice)
	assert.Equal(testCcwDev, *ccwDevice)
}

// testNetworkPairEndpointRateLimiters tests rate limiter getters/setters
func testNetworkPairEndpointRateLimiters(t *testing.T, endpoint Endpoint) {
	assert := assert.New(t)

	// Test RxRateLimiter
	assert.False(endpoint.GetRxRateLimiter())
	err := endpoint.SetRxRateLimiter()
	assert.NoError(err)
	assert.True(endpoint.GetRxRateLimiter())

	// Test TxRateLimiter
	assert.False(endpoint.GetTxRateLimiter())
	err = endpoint.SetTxRateLimiter()
	assert.NoError(err)
	assert.True(endpoint.GetTxRateLimiter())
}
