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

func TestCreateIPVlanEndpoint(t *testing.T) {
	assert := assert.New(t)
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	expected := &IPVlanEndpoint{
		NetPair: NetworkInterfacePair{
			TapInterface: TapInterface{
				ID:   "uniqueTestID-5",
				Name: "br5_kata",
				TAPIface: NetworkInterface{
					Name: "tap5_kata",
				},
			},
			VirtIface: NetworkInterface{
				Name:     "eth5",
				HardAddr: macAddr.String(),
			},

			NetInterworkingModel: NetXConnectTCFilterModel,
		},
		EndpointType: IPVlanEndpointType,
	}

	result, err := createIPVlanNetworkEndpoint(5, "")
	assert.NoError(err)

	// the resulting ID  will be random - so let's overwrite to test the rest of the flow
	result.NetPair.ID = "uniqueTestID-5"

	// the resulting mac address will be random - so lets overwrite it
	result.NetPair.VirtIface.HardAddr = macAddr.String()

	assert.Exactly(result, expected)
}
