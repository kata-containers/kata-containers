// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"net"
	"reflect"
	"testing"
)

func TestCreateBridgedMacvlanEndpoint(t *testing.T) {
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	expected := &BridgedMacvlanEndpoint{
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
		EndpointType: BridgedMacvlanEndpointType,
	}

	result, err := createBridgedMacvlanNetworkEndpoint(4, "", DefaultNetInterworkingModel)
	if err != nil {
		t.Fatal(err)
	}

	// the resulting ID  will be random - so let's overwrite to test the rest of the flow
	result.NetPair.ID = "uniqueTestID-4"

	// the resulting mac address will be random - so lets overwrite it
	result.NetPair.VirtIface.HardAddr = macAddr.String()

	if reflect.DeepEqual(result, expected) == false {
		t.Fatalf("\nGot: %+v, \n\nExpected: %+v", result, expected)
	}
}
