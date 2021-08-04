// Copyright (c) 2021 Red Hat Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestGetAddressAndDialer(t *testing.T) {
	assert := assert.New(t)

	endpoint := "/no/protocol"
	addr, _, err := getAddressAndDialer(endpoint)
	assert.Nil(err, "endpoints with no protocol are deprecated but should be accepted")
	assert.Equal(endpoint, addr, "failed address parsing")

	endpoint = "tcp://hostname:1234"
	_, _, err = getAddressAndDialer(endpoint)
	assert.NotNil(err, "only unix endpoints should be accepted")
}

func TestParseEndpointWithFallbackProtocol(t *testing.T) {
	assert := assert.New(t)

	endpoint := "/no/protocol"
	proto, addr, err := parseEndpointWithFallbackProtocol(endpoint, unixProtocol)
	assert.Nil(err, "endpoints with no protocol are deprecated but should be accepted")
	assert.Equal(unixProtocol, proto, "error parsing the endpoint protocol")
	assert.Equal(addr, endpoint, "error parsing the endpoint address")

	endpoint = "wrong://protocol"
	_, _, err = parseEndpointWithFallbackProtocol(endpoint, unixProtocol)
	assert.NotNil(err, "unsupported protocols shouldn't be accepted")

	endpoint = "unix:///run/runtime/runtime.sock"
	proto, addr, err = parseEndpointWithFallbackProtocol(endpoint, unixProtocol)
	assert.Nil(err, "failed parsing unix endpoint")
	assert.Equal(proto, "unix", "failed protocol parsing")
	assert.Equal(addr, "/run/runtime/runtime.sock", "failed address parsing")

	endpoint = "tcp://hostname:1234"
	proto, addr, err = parseEndpointWithFallbackProtocol(endpoint, unixProtocol)
	assert.Nil(err, "failed parsing tcp endpoint")
	assert.Equal(proto, "tcp", "failed protocol parsing")
	assert.Equal(addr, "hostname:1234", "failed address parsing")
}
func TestParseEndpoint(t *testing.T) {
	assert := assert.New(t)

	endpoint := "unix:///run/runtime/runtime.sock"
	proto, addr, err := parseEndpoint(endpoint)
	assert.Nil(err, "unix endpoints should be accepted")
	assert.Equal("unix", proto, "failed protocol parsing")
	assert.Equal("/run/runtime/runtime.sock", addr, "failed address parsing")

	endpoint = "no.protocol"
	_, _, err = parseEndpoint(endpoint)
	assert.NotNil(err, "endpoints with no protocol shouldn't be accepted")

	endpoint = "wrong://protocol"
	_, _, err = parseEndpoint(endpoint)
	assert.NotNil(err, "unsupported protocols shouldn't be accepted")

	endpoint = "tcp://hostname:1234"
	proto, addr, err = parseEndpoint(endpoint)
	assert.Nil(err, "tcp endpoints should be accepted")
	assert.Equal("tcp", proto, "failed protocol parsing")
	assert.Equal("hostname:1234", addr, "failed address parsing")
}
