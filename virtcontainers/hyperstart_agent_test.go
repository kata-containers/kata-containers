// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"net"
	"reflect"
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/pkg/hyperstart"
	"github.com/vishvananda/netlink"
)

var testRouteDest = "192.168.10.1/32"
var testRouteGateway = "192.168.0.0"
var testRouteDeviceName = "test_eth0"
var testRouteDestIPv6 = "2001:db8::/32"

func TestHyperstartGenerateSocketsSuccessful(t *testing.T) {
	config := HyperConfig{
		SockCtlName: "ctlSock",
		SockTtyName: "ttySock",
	}

	sandbox := &Sandbox{
		id: testSandboxID,
	}

	h := &hyper{}

	h.generateSockets(sandbox, config)

	expectedSockets := []Socket{
		{
			DeviceID: fmt.Sprintf(defaultDeviceIDTemplate, 0),
			ID:       fmt.Sprintf(defaultIDTemplate, 0),
			HostPath: config.SockCtlName,
			Name:     fmt.Sprintf(defaultChannelTemplate, 0),
		},
		{
			DeviceID: fmt.Sprintf(defaultDeviceIDTemplate, 1),
			ID:       fmt.Sprintf(defaultIDTemplate, 1),
			HostPath: config.SockTtyName,
			Name:     fmt.Sprintf(defaultChannelTemplate, 1),
		},
	}

	if !reflect.DeepEqual(expectedSockets, h.sockets) {
		t.Fatalf("Expecting %+v, Got %+v", expectedSockets, h.sockets)
	}
}

func TestHyperstartGenerateSocketsSuccessfulNoPathProvided(t *testing.T) {
	config := HyperConfig{}

	sandbox := &Sandbox{
		id: testSandboxID,
	}

	h := &hyper{}

	h.generateSockets(sandbox, config)

	expectedSockets := []Socket{
		{
			DeviceID: fmt.Sprintf(defaultDeviceIDTemplate, 0),
			ID:       fmt.Sprintf(defaultIDTemplate, 0),
			HostPath: fmt.Sprintf(defaultSockPathTemplates[0], runStoragePath, sandbox.id),
			Name:     fmt.Sprintf(defaultChannelTemplate, 0),
		},
		{
			DeviceID: fmt.Sprintf(defaultDeviceIDTemplate, 1),
			ID:       fmt.Sprintf(defaultIDTemplate, 1),
			HostPath: fmt.Sprintf(defaultSockPathTemplates[1], runStoragePath, sandbox.id),
			Name:     fmt.Sprintf(defaultChannelTemplate, 1),
		},
	}

	if !reflect.DeepEqual(expectedSockets, h.sockets) {
		t.Fatalf("Expecting %+v, Got %+v", expectedSockets, h.sockets)
	}
}

func testProcessHyperRoute(t *testing.T, route netlink.Route, deviceName string, expected *hyperstart.Route) {
	h := &hyper{}
	hyperRoute := h.processHyperRoute(route, deviceName)

	if expected == nil {
		if hyperRoute != nil {
			t.Fatalf("Expecting route to be nil, Got %+v", hyperRoute)
		} else {
			return
		}
	}

	// At this point, we know that "expected" != nil.
	if !reflect.DeepEqual(*expected, *hyperRoute) {
		t.Fatalf("Expecting %+v, Got %+v", *expected, *hyperRoute)
	}
}

func TestProcessHyperRouteEmptyGWSuccessful(t *testing.T) {
	expected := &hyperstart.Route{
		Dest:    testRouteDest,
		Gateway: "",
		Device:  testRouteDeviceName,
	}

	_, dest, err := net.ParseCIDR(testRouteDest)
	if err != nil {
		t.Fatal(err)
	}

	route := netlink.Route{
		Dst: dest,
		Gw:  net.IP{},
	}

	testProcessHyperRoute(t, route, testRouteDeviceName, expected)
}

func TestProcessHyperRouteEmptyDestSuccessful(t *testing.T) {
	expected := &hyperstart.Route{
		Dest:    defaultRouteLabel,
		Gateway: testRouteGateway,
		Device:  testRouteDeviceName,
	}

	_, dest, err := net.ParseCIDR(defaultRouteDest)
	if err != nil {
		t.Fatal(err)
	}

	route := netlink.Route{
		Dst: dest,
		Gw:  net.ParseIP(testRouteGateway),
	}

	testProcessHyperRoute(t, route, testRouteDeviceName, expected)
}

func TestProcessHyperRouteDestIPv6Failure(t *testing.T) {
	_, dest, err := net.ParseCIDR(testRouteDestIPv6)
	if err != nil {
		t.Fatal(err)
	}

	route := netlink.Route{
		Dst: dest,
		Gw:  net.IP{},
	}

	testProcessHyperRoute(t, route, testRouteDeviceName, nil)
}
