// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
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
