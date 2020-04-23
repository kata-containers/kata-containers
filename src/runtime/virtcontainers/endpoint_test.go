// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"io/ioutil"
	"net"
	"os"
	"reflect"
	"testing"

	"github.com/stretchr/testify/assert"
)

func testEndpointTypeSet(t *testing.T, value string, expected EndpointType) {
	var endpointType EndpointType

	err := endpointType.Set(value)
	assert.NoError(t, err)
	assert.Equal(t, endpointType, expected)
}

func TestPhysicalEndpointTypeSet(t *testing.T) {
	testEndpointTypeSet(t, "physical", PhysicalEndpointType)
}

func TestVethEndpointTypeSet(t *testing.T) {
	testEndpointTypeSet(t, "virtual", VethEndpointType)
}

func TestVhostUserEndpointTypeSet(t *testing.T) {
	testEndpointTypeSet(t, "vhost-user", VhostUserEndpointType)
}

func TestBridgedMacvlanEndpointTypeSet(t *testing.T) {
	testEndpointTypeSet(t, "macvlan", BridgedMacvlanEndpointType)
}

func TestMacvtapEndpointTypeSet(t *testing.T) {
	testEndpointTypeSet(t, "macvtap", MacvtapEndpointType)
}

func TestEndpointTypeSetFailure(t *testing.T) {
	var endpointType EndpointType

	assert.Error(t, endpointType.Set("wrong-value"))
}

func testEndpointTypeString(t *testing.T, endpointType *EndpointType, expected string) {
	result := endpointType.String()
	assert.Equal(t, result, expected)
}

func TestPhysicalEndpointTypeString(t *testing.T) {
	endpointType := PhysicalEndpointType
	testEndpointTypeString(t, &endpointType, string(PhysicalEndpointType))
}

func TestVethEndpointTypeString(t *testing.T) {
	endpointType := VethEndpointType
	testEndpointTypeString(t, &endpointType, string(VethEndpointType))
}

func TestVhostUserEndpointTypeString(t *testing.T) {
	endpointType := VhostUserEndpointType
	testEndpointTypeString(t, &endpointType, string(VhostUserEndpointType))
}

func TestBridgedMacvlanEndpointTypeString(t *testing.T) {
	endpointType := BridgedMacvlanEndpointType
	testEndpointTypeString(t, &endpointType, string(BridgedMacvlanEndpointType))
}

func TestMacvtapEndpointTypeString(t *testing.T) {
	endpointType := MacvtapEndpointType
	testEndpointTypeString(t, &endpointType, string(MacvtapEndpointType))
}

func TestIncorrectEndpointTypeString(t *testing.T) {
	var endpointType EndpointType
	testEndpointTypeString(t, &endpointType, "")
}

func TestSaveLoadIfPair(t *testing.T) {
	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	tmpfile, err := ioutil.TempFile("", "vc-save-load-net-")
	assert.Nil(t, err)
	defer os.Remove(tmpfile.Name())

	netPair := &NetworkInterfacePair{
		TapInterface: TapInterface{
			ID:   "uniqueTestID-4",
			Name: "br4_kata",
			TAPIface: NetworkInterface{
				Name:     "tap4_kata",
				HardAddr: macAddr.String(),
			},
			VMFds:    []*os.File{tmpfile}, // won't be saved to disk
			VhostFds: []*os.File{tmpfile}, // won't be saved to disk
		},
		VirtIface: NetworkInterface{
			Name:     "eth4",
			HardAddr: macAddr.String(),
		},
		NetInterworkingModel: DefaultNetInterworkingModel,
	}

	// Save to disk then load it back.
	savedIfPair := saveNetIfPair(netPair)
	loadedIfPair := loadNetIfPair(savedIfPair)

	// Since VMFds and VhostFds are't saved, netPair and loadedIfPair are not equal.
	assert.False(t, reflect.DeepEqual(netPair, loadedIfPair))

	netPair.TapInterface.VMFds = nil
	netPair.TapInterface.VhostFds = nil
	// They are equal now.
	assert.True(t, reflect.DeepEqual(netPair, loadedIfPair))
}
