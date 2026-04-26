//go:build linux

// Copyright (c) 2026
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/stretchr/testify/assert"
)

type testVFIODevice struct {
	info []*config.VFIODev
}

func (d testVFIODevice) Attach(context.Context, api.DeviceReceiver) error { return nil }
func (d testVFIODevice) Detach(context.Context, api.DeviceReceiver) error { return nil }

func (d testVFIODevice) DeviceID() string              { return "vfio-test" }
func (d testVFIODevice) DeviceType() config.DeviceType { return config.DeviceVFIO }
func (d testVFIODevice) GetMajorMinor() (int64, int64) { return 0, 0 }
func (d testVFIODevice) GetHostPath() string           { return "" }
func (d testVFIODevice) GetDeviceInfo() interface{}    { return d.info }
func (d testVFIODevice) GetAttachCount() uint          { return 0 }
func (d testVFIODevice) Reference() uint               { return 0 }
func (d testVFIODevice) Dereference() uint             { return 0 }
func (d testVFIODevice) Save() config.DeviceState      { return config.DeviceState{} }
func (d testVFIODevice) Load(config.DeviceState)       {}

type recordingHypervisor struct {
	mockHypervisor
	added []config.VFIODev
}

func (h *recordingHypervisor) AddDevice(_ context.Context, devInfo interface{}, devType DeviceType) error {
	if devType == VfioDev {
		h.added = append(h.added, devInfo.(config.VFIODev))
	}
	return nil
}

func TestSandboxAppendDeviceAddsAllVFIOFunctions(t *testing.T) {
	hypervisor := &recordingHypervisor{}
	s := &Sandbox{hypervisor: hypervisor}

	device := testVFIODevice{
		info: []*config.VFIODev{
			{BDF: "0000:01:00.0"},
			{BDF: "0000:01:00.1"},
		},
	}

	err := s.AppendDevice(context.Background(), device)
	assert.NoError(t, err)
	assert.Len(t, hypervisor.added, 2)
	assert.Equal(t, "0000:01:00.0", hypervisor.added[0].BDF)
	assert.Equal(t, "0000:01:00.1", hypervisor.added[1].BDF)
}
