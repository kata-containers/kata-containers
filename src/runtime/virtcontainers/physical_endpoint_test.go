//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"
	"fmt"
	"net"
	"testing"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/containernetworking/plugins/pkg/testutils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/manager"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
)

// detachRecordingEndpoint is a physical endpoint that records its detach
// instead of touching the host's PCI devices.
type detachRecordingEndpoint struct {
	PhysicalEndpoint
	detached int
}

func (endpoint *detachRecordingEndpoint) Detach(ctx context.Context, netNsCreated bool, netNsPath string) error {
	endpoint.detached++
	return nil
}

// TestSandboxStopRestoresPassthroughDeviceOnlyOnConfirmedVMMExit covers the
// teardown's side of an unconfirmed VMM exit.
func TestSandboxStopRestoresPassthroughDeviceOnlyOnConfirmedVMMExit(t *testing.T) {
	for _, tc := range []struct {
		name         string
		stopVMErr    error
		wantDetaches int
	}{
		{
			name:         "a confirmed exit hands the device back",
			stopVMErr:    nil,
			wantDetaches: 1,
		},
		{
			name:         "an unconfirmed exit leaves the device alone",
			stopVMErr:    fmt.Errorf("%w: QEMU pid 1 still running", errVMMExitUnconfirmed),
			wantDetaches: 0,
		},
		{
			name:         "an unrelated stop failure still hands the device back",
			stopVMErr:    errors.New("failed to stop the virtiofs daemon"),
			wantDetaches: 1,
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			assert := assert.New(t)

			endpoint := &detachRecordingEndpoint{
				PhysicalEndpoint: PhysicalEndpoint{
					IfaceName:    "eth0",
					HardAddr:     net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
					EndpointType: PhysicalEndpointType,
					BDF:          "0000:b5:09.7",
					Driver:       "mlx5_core",
				},
			}

			store, err := persist.GetDriver()
			assert.NoError(err)

			s := &Sandbox{
				id:     testSandboxID,
				ctx:    context.Background(),
				config: &SandboxConfig{},
				store:  store,
				state: types.SandboxState{
					State:         types.StateReady,
					BlockIndexMap: make(map[int]struct{}),
				},
				containers: make(map[string]*Container),
				agent:      NewMockAgent(),
				devManager: manager.NewDeviceManager(config.VirtioSCSI, false, "", 0, nil),
				network:    &LinuxNetwork{eps: []Endpoint{endpoint}},
				hypervisor: &mockHypervisor{
					stopVMFunc: func(ctx context.Context, waitOnly bool) error {
						return tc.stopVMErr
					},
				},
			}

			// Only a force stop carries on past the hypervisor failure,
			// which is the case that matters here.
			s.Stop(context.Background(), true) //nolint:errcheck

			assert.Equal(tc.wantDetaches, endpoint.detached)
			assert.Equal(types.StateStopped, s.state.State)
		})
	}
}

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
