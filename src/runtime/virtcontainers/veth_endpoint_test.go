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

func TestCreateVethNetworkEndpoint(t *testing.T) {
	assert := assert.New(t)
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	expected := &VethEndpoint{
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
		EndpointType: VethEndpointType,
	}

	result, err := createVethNetworkEndpoint(4, "", DefaultNetInterworkingModel)
	assert.NoError(err)

	// the resulting ID  will be random - so let's overwrite to test the rest of the flow
	result.NetPair.ID = "uniqueTestID-4"

	// the resulting mac address will be random - so lets overwrite it
	result.NetPair.VirtIface.HardAddr = macAddr.String()

	assert.Exactly(result, expected)
}

func TestCreateVethNetworkEndpointChooseIfaceName(t *testing.T) {
	assert := assert.New(t)
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	expected := &VethEndpoint{
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
		EndpointType: VethEndpointType,
	}

	result, err := createVethNetworkEndpoint(4, "eth1", DefaultNetInterworkingModel)
	assert.NoError(err)

	// the resulting ID will be random - so let's overwrite to test the rest of the flow
	result.NetPair.ID = "uniqueTestID-4"

	// the resulting mac address will be random - so lets overwrite it
	result.NetPair.VirtIface.HardAddr = macAddr.String()

	assert.Exactly(result, expected)
}

func TestCreateVethNetworkEndpointInvalidArgs(t *testing.T) {
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
		_, err := createVethNetworkEndpoint(d.idx, d.ifName, DefaultNetInterworkingModel)
		assert.Error(err)
	}
}
