//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"net"
	"testing"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/containernetworking/plugins/pkg/testutils"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/stretchr/testify/assert"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
)

func TestPhysicalEndpoint_HotAttach(t *testing.T) {
	assert := assert.New(t)
	v := &PhysicalEndpoint{
		IfaceName: "eth0",
		HardAddr:  net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
	}

	s := &Sandbox{
		hypervisor: &mockHypervisor{},
	}

	err := v.HotAttach(context.Background(), s)
	assert.Error(err)
}

func TestPhysicalEndpoint_HotDetach(t *testing.T) {
	assert := assert.New(t)
	v := &PhysicalEndpoint{
		IfaceName: "eth0",
		HardAddr:  net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
	}

	s := &Sandbox{
		hypervisor: &mockHypervisor{},
	}

	err := v.HotDetach(context.Background(), s, true, "")
	assert.Error(err)
}

func TestIsPhysicalIface(t *testing.T) {
	assert := assert.New(t)

	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	testNetIface := "testIface0"
	testMTU := 1500
	testMACAddr := "00:00:00:00:00:01"

	hwAddr, err := net.ParseMAC(testMACAddr)
	assert.NoError(err)

	link := &netlink.Bridge{
		LinkAttrs: netlink.LinkAttrs{
			Name:         testNetIface,
			MTU:          testMTU,
			HardwareAddr: hwAddr,
			TxQLen:       -1,
		},
	}

	n, err := testutils.NewNS()
	assert.NoError(err)
	defer n.Close()

	netnsHandle, err := netns.GetFromPath(n.Path())
	assert.NoError(err)
	defer netnsHandle.Close()

	netlinkHandle, err := netlink.NewHandleAt(netnsHandle)
	assert.NoError(err)
	defer netlinkHandle.Close()

	err = netlinkHandle.LinkAdd(link)
	assert.NoError(err)

	var isPhysical bool
	err = doNetNS(n.Path(), func(_ ns.NetNS) error {
		var err error
		isPhysical, err = isPhysicalIface(testNetIface)
		return err
	})
	assert.NoError(err)
	assert.False(isPhysical)
}
