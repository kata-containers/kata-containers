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
	"path/filepath"
	"testing"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/containernetworking/plugins/pkg/testutils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/manager"
	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
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

// Fail after the hypervisor starts, so startup rollback has to stop it.
type startupFailingAgent struct {
	mockAgent
	err error
}

func (a *startupFailingAgent) startSandbox(context.Context, *Sandbox) error {
	return a.err
}

func TestSandboxRollbackPreservesPassthroughDevice(t *testing.T) {
	for _, startupFailure := range []bool{true, false} {
		t.Run(fmt.Sprintf("startupFailure=%t", startupFailure), func(t *testing.T) {
			endpoint := &detachRecordingEndpoint{PhysicalEndpoint: PhysicalEndpoint{
				IfaceName:    "eth0",
				EndpointType: PhysicalEndpointType,
			}}
			stopErr := fmt.Errorf("%w: QEMU still running", errVMMExitUnconfirmed)
			startErr := errors.New("agent startup failed")
			stops := 0
			s := &Sandbox{
				id:      testSandboxID,
				ctx:     context.Background(),
				config:  &SandboxConfig{NetworkConfig: NetworkConfig{DisableNewNetwork: true}},
				agent:   &startupFailingAgent{err: startErr},
				network: &LinuxNetwork{eps: []Endpoint{endpoint}},
				hypervisor: &mockHypervisor{stopVMFunc: func(context.Context, bool) error {
					stops++
					return stopErr
				}},
			}
			if startupFailure {
				require.ErrorIs(t, s.startVM(context.Background(), nil), startErr)
			} else {
				// createSandboxFromConfig uses stopVM on later failures.
				require.ErrorIs(t, s.stopVM(context.Background()), stopErr)
			}
			require.Equal(t, 1, stops)
			require.NoError(t, s.removeNetwork(context.Background()))
			assert.Zero(t, endpoint.detached, "rollback restored a device QEMU may still own")

			// A later confirmed exit allows the retained device to be restored.
			s.hypervisor.(*mockHypervisor).stopVMFunc = nil
			require.NoError(t, s.stopVM(context.Background()))
			require.NoError(t, s.removeNetwork(context.Background()))
			assert.Equal(t, 1, endpoint.detached)
		})
	}
}

func TestSandboxStartupFailureRestoresNeverLaunchedDevice(t *testing.T) {
	for _, stage := range []string{"QMP setup", "virtiofsd setup", "QEMU launch", "stopped before launch"} {
		t.Run(stage, func(t *testing.T) {
			hypervisor, err := NewHypervisor(QemuHypervisor)
			require.NoError(t, err)
			q := hypervisor.(*qemu)
			dir := t.TempDir()
			q.id = "testSandbox"
			q.ctx = context.Background()
			q.config = HypervisorConfig{VMStorePath: dir, RunStorePath: dir, DisableSeLinux: true}
			q.arch, err = newQemuArch(newQemuConfig())
			require.NoError(t, err)
			q.qmpMonitorCh.ctx = context.Background()
			q.qmpMonitorCh.path = filepath.Join(dir, q.id, "qmp.sock")
			q.qemuConfig = govmmQemu.Config{
				Path:       filepath.Join(dir, "missing-qemu"),
				PidFile:    filepath.Join(dir, q.id, "pid"),
				QMPSockets: []govmmQemu.QMPSocket{{Type: govmmQemu.Unix}},
				SMP:        govmmQemu.SMP{CPUs: 1, MaxCPUs: 1},
			}
			switch stage {
			case "QMP setup":
				// Listening on the existing directory fails before launch.
				q.qmpMonitorCh.path = dir
			case "virtiofsd setup":
				q.config.SharedFS = config.VirtioFS
				q.virtiofsDaemon = &virtiofsd{path: filepath.Join(dir, "missing-virtiofsd")}
			}
			endpoint := &detachRecordingEndpoint{PhysicalEndpoint: PhysicalEndpoint{
				IfaceName: "eth0", EndpointType: PhysicalEndpointType,
			}}
			s := &Sandbox{
				id:         q.id,
				ctx:        context.Background(),
				config:     &SandboxConfig{NetworkConfig: NetworkConfig{DisableNewNetwork: true}},
				agent:      NewMockAgent(),
				hypervisor: q,
				network:    &LinuxNetwork{eps: []Endpoint{endpoint}},
			}

			if stage == "stopped before launch" {
				require.NoError(t, q.StopVM(context.Background(), false))
			}
			err = s.startVM(context.Background(), nil)
			require.Error(t, err)
			if stage == "QEMU launch" {
				assert.Contains(t, err.Error(), "failed to launch qemu")
			}
			if stage == "stopped before launch" {
				assert.Contains(t, err.Error(), "QEMU was stopped before launch")
				assert.NoDirExists(t, filepath.Join(dir, q.id))
			}
			require.Nil(t, q.exitCh, "QEMU must not have launched")
			require.NoError(t, s.removeNetwork(context.Background()))
			assert.Equal(t, 1, endpoint.detached, "startup rollback must restore a VF QEMU never owned")
		})
	}
}
