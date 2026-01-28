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
)

func vethEndpointCreator(idx int, ifName string, model NetInterworkingModel) (Endpoint, error) {
	return createVethNetworkEndpoint(idx, ifName, model)
}

func TestCreateVethNetworkEndpoint(t *testing.T) {
	testCreateNetworkPairEndpoint(t, vethEndpointCreator, VethEndpointType)
}

func TestCreateVethNetworkEndpointChooseIfaceName(t *testing.T) {
	testCreateNetworkPairEndpointChooseIfaceName(t, vethEndpointCreator, VethEndpointType)
}

func TestCreateVethNetworkEndpointInvalidArgs(t *testing.T) {
	testCreateNetworkPairEndpointInvalidArgs(t, vethEndpointCreator)
}

func TestVethEndpointProperties(t *testing.T) {
	assert := assert.New(t)

	endpoint, err := createVethNetworkEndpoint(0, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	testNetworkPairEndpointProperties(t, endpoint, VethEndpointType, "virtual")
}

func TestVethEndpointPciPath(t *testing.T) {
	assert := assert.New(t)

	endpoint, err := createVethNetworkEndpoint(0, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	testNetworkPairEndpointPciPath(t, endpoint)
}

func TestVethEndpointCcwDevice(t *testing.T) {
	assert := assert.New(t)

	endpoint, err := createVethNetworkEndpoint(0, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	testNetworkPairEndpointCcwDevice(t, endpoint)
}

func TestVethEndpointRateLimiters(t *testing.T) {
	assert := assert.New(t)

	endpoint, err := createVethNetworkEndpoint(0, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	testNetworkPairEndpointRateLimiters(t, endpoint)
}

func TestVethEndpointSaveLoad(t *testing.T) {
	assert := assert.New(t)
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x05}

	endpoint := &VethEndpoint{
		NetworkPairEndpointBase: NetworkPairEndpointBase{
			NetPair: NetworkInterfacePair{
				TapInterface: TapInterface{
					ID:   "test-veth-id",
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
			EndpointType: VethEndpointType,
		},
	}

	// Save the endpoint
	saved := endpoint.save()
	assert.Equal(string(VethEndpointType), saved.Type)
	assert.NotNil(saved.Veth)
	assert.Equal("test-veth-id", saved.Veth.NetPair.TapInterface.ID)
	assert.Equal("br5_kata", saved.Veth.NetPair.TapInterface.Name)
	assert.Equal("eth5", saved.Veth.NetPair.VirtIface.Name)

	// Load into a new endpoint
	newEndpoint := &VethEndpoint{}
	newEndpoint.load(saved)

	assert.Equal(VethEndpointType, newEndpoint.EndpointType)
	assert.Equal(endpoint.NetPair.ID, newEndpoint.NetPair.ID)
	assert.Equal(endpoint.NetPair.Name, newEndpoint.NetPair.Name)
	assert.Equal(endpoint.NetPair.TAPIface.Name, newEndpoint.NetPair.TAPIface.Name)
	assert.Equal(endpoint.NetPair.VirtIface.Name, newEndpoint.NetPair.VirtIface.Name)
	assert.Equal(endpoint.NetPair.NetInterworkingModel, newEndpoint.NetPair.NetInterworkingModel)
}
