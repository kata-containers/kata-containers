// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/stretchr/testify/assert"
)

func TestGetVFIODetails(t *testing.T) {
	type testData struct {
		deviceStr   string
		expectedStr string
	}

	data := []testData{
		{"0000:02:10.0", "0000:02:10.0"},
		{"0000:0210.0", ""},
		{"f79944e4-5a3d-11e8-99ce-", ""},
		{"f79944e4-5a3d-11e8-99ce", ""},
		{"test", ""},
		{"", ""},
	}

	for _, d := range data {
		deviceBDF, deviceSysfsDev, vfioDeviceType, err := GetVFIODetails(d.deviceStr, "")

		switch vfioDeviceType {
		case config.VFIOPCIDeviceNormalType:
			assert.Equal(t, d.expectedStr, deviceBDF)
		case config.VFIOPCIDeviceMediatedType, config.VFIOAPDeviceMediatedType:
			assert.Equal(t, d.expectedStr, deviceSysfsDev)
		default:
			assert.NotNil(t, err)
		}

		if d.expectedStr == "" {
			assert.NotNil(t, err)
		} else {
			assert.Nil(t, err)
		}
	}

}

func TestAssignPCIeTopologyKeepsMultifunctionDevicesOnSameBus(t *testing.T) {
	saved := config.PCIeDevicesPerPort
	config.PCIeDevicesPerPort = map[config.PCIePort][]config.VFIODev{
		config.RootPort:   {},
		config.SwitchPort: {},
		config.BridgePort: {},
	}
	defer func() {
		config.PCIeDevicesPerPort = saved
	}()

	vfioDevs := []*config.VFIODev{
		{
			BDF:    "0000:01:00.0",
			IsPCIe: true,
			Port:   config.RootPort,
		},
		{
			BDF:    "0000:01:00.1",
			IsPCIe: true,
			Port:   config.RootPort,
		},
	}

	assignPCIeTopology(vfioDevs)

	assert.Equal(t, "rp0", vfioDevs[0].Bus)
	assert.Equal(t, "rp0", vfioDevs[1].Bus)
	assert.Equal(t, "00.0", vfioDevs[0].Addr)
	assert.Equal(t, "00.1", vfioDevs[1].Addr)
	assert.True(t, vfioDevs[0].MultiFunction)
	assert.False(t, vfioDevs[1].MultiFunction)
	assert.Len(t, config.PCIeDevicesPerPort[config.RootPort], 2)
}
