//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestCreateMacvtapEndpoint(t *testing.T) {
	netInfo := NetworkInfo{
		Iface: NetlinkIface{
			Type: "macvtap",
		},
	}
	expected := &MacvtapEndpoint{
		EndpointType:       MacvtapEndpointType,
		EndpointProperties: netInfo,
	}

	result, err := createMacvtapNetworkEndpoint(netInfo)
	assert.NoError(t, err)
	assert.Exactly(t, result, expected)
}
