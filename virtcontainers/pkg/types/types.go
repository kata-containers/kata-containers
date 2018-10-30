// Copyright 2018 Intel Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package types

// IPAddress describes an IP address.
type IPAddress struct {
	Family  int
	Address string
	Mask    string
}

// Interface describes a network interface.
type Interface struct {
	Device      string
	Name        string
	IPAddresses []*IPAddress
	Mtu         uint64
	HwAddr      string
	// pciAddr is the PCI address in the format  "bridgeAddr/deviceAddr".
	// Here, bridgeAddr is the address at which the bridge is attached on the root bus,
	// while deviceAddr is the address at which the network device is attached on the bridge.
	PciAddr string
	// LinkType defines the type of interface described by this structure.
	// The expected values are the one that are defined by the netlink
	// library, regarding each type of link. Here is a non exhaustive
	// list: "veth", "macvtap", "vlan", "macvlan", "tap", ...
	LinkType string
}

// Route describes a network route.
type Route struct {
	Dest    string
	Gateway string
	Device  string
	Source  string
	Scope   uint32
}
