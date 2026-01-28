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
)

func netkitEndpointCreator(idx int, ifName string, model NetInterworkingModel) (Endpoint, error) {
	return createNetkitNetworkEndpoint(idx, ifName, model)
}

func TestCreateNetkitNetworkEndpoint(t *testing.T) {
	testCreateNetworkPairEndpoint(t, netkitEndpointCreator, NetkitEndpointType)
}

func TestCreateNetkitNetworkEndpointChooseIfaceName(t *testing.T) {
	testCreateNetworkPairEndpointChooseIfaceName(t, netkitEndpointCreator, NetkitEndpointType)
}

func TestCreateNetkitNetworkEndpointInvalidArgs(t *testing.T) {
	testCreateNetworkPairEndpointInvalidArgs(t, netkitEndpointCreator)
}

func TestNetkitEndpointProperties(t *testing.T) {
	assert := assert.New(t)

	endpoint, err := createNetkitNetworkEndpoint(0, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	testNetworkPairEndpointProperties(t, endpoint, NetkitEndpointType, "netkit")
}

func TestNetkitEndpointPciPath(t *testing.T) {
	assert := assert.New(t)

	endpoint, err := createNetkitNetworkEndpoint(0, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	testNetworkPairEndpointPciPath(t, endpoint)
}

func TestNetkitEndpointCcwDevice(t *testing.T) {
	assert := assert.New(t)

	endpoint, err := createNetkitNetworkEndpoint(0, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	testNetworkPairEndpointCcwDevice(t, endpoint)
}

func TestNetkitEndpointRateLimiters(t *testing.T) {
	assert := assert.New(t)

	endpoint, err := createNetkitNetworkEndpoint(0, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	testNetworkPairEndpointRateLimiters(t, endpoint)
}

func TestNetkitEndpointSaveLoad(t *testing.T) {
	assert := assert.New(t)
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x05}

	endpoint := &NetkitEndpoint{
		NetworkPairEndpointBase: NetworkPairEndpointBase{
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
		},
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
