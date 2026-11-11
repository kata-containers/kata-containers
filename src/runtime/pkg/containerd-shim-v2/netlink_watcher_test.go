//go:build linux

// Copyright (c) 2026 Naval Group
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"net"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/vishvananda/netlink"

	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
)

func TestKnownIfaceSet(t *testing.T) {
	assert := assert.New(t)

	k := newKnownIfaceSet()

	// Empty set
	assert.False(k.hasMACOrName("aa:bb:cc:dd:ee:ff", "eth0"))
	assert.Equal("", k.lookupMAC("aa:bb:cc:dd:ee:ff", "eth0"))
	assert.Equal("", k.getEndpointMAC("eth0"))

	// Add an interface
	k.add("aa:bb:cc:dd:ee:ff", "eth0")
	assert.True(k.hasMACOrName("aa:bb:cc:dd:ee:ff", ""))
	assert.True(k.hasMACOrName("", "eth0"))
	assert.True(k.hasMACOrName("aa:bb:cc:dd:ee:ff", "eth0"))
	assert.False(k.hasMACOrName("11:22:33:44:55:66", "net1"))

	// lookupMAC by MAC
	assert.Equal("aa:bb:cc:dd:ee:ff", k.lookupMAC("aa:bb:cc:dd:ee:ff", ""))
	// lookupMAC by name (fallback when MAC is empty, e.g. DELLINK events)
	assert.Equal("aa:bb:cc:dd:ee:ff", k.lookupMAC("", "eth0"))
	// lookupMAC with unknown
	assert.Equal("", k.lookupMAC("", "unknown"))

	// Endpoint MAC tracking
	k.setEndpointMAC("eth0", "11:22:33:44:55:66")
	assert.Equal("11:22:33:44:55:66", k.getEndpointMAC("eth0"))
	assert.Equal("", k.getEndpointMAC("net1"))

	// Remove
	k.remove("aa:bb:cc:dd:ee:ff", "eth0")
	assert.False(k.hasMACOrName("aa:bb:cc:dd:ee:ff", "eth0"))
}

func TestKnownIfaceSetMultiple(t *testing.T) {
	assert := assert.New(t)

	k := newKnownIfaceSet()
	k.add("aa:bb:cc:dd:ee:ff", "eth0")
	k.add("11:22:33:44:55:66", "net1")

	assert.True(k.hasMACOrName("aa:bb:cc:dd:ee:ff", ""))
	assert.True(k.hasMACOrName("11:22:33:44:55:66", ""))
	assert.True(k.hasMACOrName("", "eth0"))
	assert.True(k.hasMACOrName("", "net1"))

	// Remove one, other stays
	k.remove("aa:bb:cc:dd:ee:ff", "eth0")
	assert.False(k.hasMACOrName("aa:bb:cc:dd:ee:ff", "eth0"))
	assert.True(k.hasMACOrName("11:22:33:44:55:66", "net1"))
}

func TestIsInfraInterface(t *testing.T) {
	tests := []struct {
		name     string
		linkName string
		linkType string
		want     bool
	}{
		{"kata TAP device", "tap0_kata", "tuntap", true},
		{"kata TAP by suffix", "tap1_kata", "device", true},
		{"kata bridge", "br0_kata", "bridge", true},
		{"regular bridge", "br-test", "bridge", true},
		{"tun device", "tun0", "tun", true},
		{"tuntap device", "tap99", "tuntap", true},
		{"veth device", "net1", "veth", false},
		{"regular eth", "eth0", "device", false},
		{"macvlan", "macvlan0", "macvlan", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := isInfraInterface(&mockLink{name: tt.linkName, linkType: tt.linkType})
			assert.Equal(t, tt.want, got)
		})
	}
}

// mockLink implements netlink.Link for testing isInfraInterface.
type mockLink struct {
	netlink.LinkAttrs
	name     string
	linkType string
}

func (m *mockLink) Attrs() *netlink.LinkAttrs {
	m.LinkAttrs.Name = m.name
	return &m.LinkAttrs
}

func (m *mockLink) Type() string {
	return m.linkType
}

func TestLinkToInterface(t *testing.T) {
	assert := assert.New(t)

	mac, _ := net.ParseMAC("aa:bb:cc:dd:ee:ff")
	link := &mockLink{
		name:     "net1",
		linkType: "veth",
	}
	link.LinkAttrs.Name = "net1"
	link.LinkAttrs.HardwareAddr = mac
	link.LinkAttrs.MTU = 1500

	addr1 := netlink.Addr{
		IPNet: &net.IPNet{
			IP:   net.IPv4(10, 88, 0, 1),
			Mask: net.CIDRMask(24, 32),
		},
	}
	addr2 := netlink.Addr{
		IPNet: &net.IPNet{
			IP:   net.ParseIP("fe80::1"),
			Mask: net.CIDRMask(64, 128),
		},
	}

	inf := linkToInterface(link, []netlink.Addr{addr1, addr2})

	assert.Equal("net1", inf.Device)
	assert.Equal("net1", inf.Name)
	assert.Equal("aa:bb:cc:dd:ee:ff", inf.HwAddr)
	assert.Equal(uint64(1500), inf.Mtu)
	assert.Equal("veth", inf.Type)
	assert.Len(inf.IPAddresses, 2)

	// IPv4 address
	assert.Equal(pbTypes.IPFamily_v4, inf.IPAddresses[0].Family)
	assert.Equal("10.88.0.1", inf.IPAddresses[0].Address)
	assert.Equal("24", inf.IPAddresses[0].Mask)

	// IPv6 address
	assert.Equal(pbTypes.IPFamily_v6, inf.IPAddresses[1].Family)
	assert.Equal("fe80::1", inf.IPAddresses[1].Address)
	assert.Equal("64", inf.IPAddresses[1].Mask)
}

func TestAddrMask(t *testing.T) {
	tests := []struct {
		name string
		mask net.IPMask
		want string
	}{
		{"/24", net.CIDRMask(24, 32), "24"},
		{"/32", net.CIDRMask(32, 32), "32"},
		{"/64", net.CIDRMask(64, 128), "64"},
		{"/16", net.CIDRMask(16, 32), "16"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			addr := netlink.Addr{
				IPNet: &net.IPNet{
					IP:   net.IPv4(10, 0, 0, 1),
					Mask: tt.mask,
				},
			}
			assert.Equal(t, tt.want, addrMask(addr))
		})
	}
}

func TestNlMsgTypeName(t *testing.T) {
	assert := assert.New(t)
	assert.Equal("RTM_NEWLINK", nlMsgTypeName(16))
	assert.Equal("RTM_DELLINK", nlMsgTypeName(17))
	assert.Contains(nlMsgTypeName(99), "unknown")
}

func TestKnownIfaceSetEndpointMAC(t *testing.T) {
	assert := assert.New(t)

	k := newKnownIfaceSet()
	k.add("aa:bb:cc:dd:ee:ff", "net1")

	// No endpoint MAC set yet
	assert.Equal("", k.getEndpointMAC("net1"))

	// Set and retrieve endpoint MAC
	k.setEndpointMAC("net1", "11:22:33:44:55:66")
	assert.Equal("11:22:33:44:55:66", k.getEndpointMAC("net1"))

	// Unknown name
	assert.Equal("", k.getEndpointMAC("net2"))

	// Remove clears all tracking including endpoint MAC mapping
	k.remove("aa:bb:cc:dd:ee:ff", "net1")
	assert.False(k.hasMACOrName("aa:bb:cc:dd:ee:ff", "net1"))
	assert.Equal("", k.getEndpointMAC("net1"))
}

func TestKnownIfaceSetLookupMACByName(t *testing.T) {
	assert := assert.New(t)

	k := newKnownIfaceSet()
	k.add("aa:bb:cc:dd:ee:ff", "net1")

	// Lookup by MAC directly
	assert.Equal("aa:bb:cc:dd:ee:ff", k.lookupMAC("aa:bb:cc:dd:ee:ff", ""))

	// Lookup by name when MAC is empty (DELLINK case)
	assert.Equal("aa:bb:cc:dd:ee:ff", k.lookupMAC("", "net1"))

	// Lookup with wrong MAC falls back to name
	assert.Equal("aa:bb:cc:dd:ee:ff", k.lookupMAC("wrong:mac", "net1"))

	// Unknown both
	assert.Equal("", k.lookupMAC("wrong:mac", "unknown"))
}

func TestIsInfraInterfaceEdgeCases(t *testing.T) {
	tests := []struct {
		name     string
		linkName string
		linkType string
		want     bool
	}{
		{"kata suffix with numbers", "tap123_kata", "device", true},
		{"br suffix kata", "br0_kata", "device", true},
		{"partial kata suffix", "tap_kat", "device", false},
		{"kata in middle", "kata_tap", "device", false},
		{"empty name tuntap type", "", "tuntap", true},
		{"ipvlan device", "ipvlan0", "ipvlan", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := isInfraInterface(&mockLink{name: tt.linkName, linkType: tt.linkType})
			assert.Equal(t, tt.want, got)
		})
	}
}

func TestLinkToInterfaceNoAddresses(t *testing.T) {
	assert := assert.New(t)

	mac, _ := net.ParseMAC("aa:bb:cc:dd:ee:ff")
	link := &mockLink{name: "net1", linkType: "veth"}
	link.LinkAttrs.Name = "net1"
	link.LinkAttrs.HardwareAddr = mac
	link.LinkAttrs.MTU = 9000

	inf := linkToInterface(link, []netlink.Addr{})

	assert.Equal("net1", inf.Name)
	assert.Equal("aa:bb:cc:dd:ee:ff", inf.HwAddr)
	assert.Equal(uint64(9000), inf.Mtu)
	assert.Len(inf.IPAddresses, 0)
}
