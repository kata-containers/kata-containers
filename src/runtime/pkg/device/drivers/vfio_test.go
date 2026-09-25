// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"os"
	"path/filepath"
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

func TestUnbindPCIDeviceIfBound(t *testing.T) {
	bdf := "0000:03:00.1"

	tests := []struct {
		name       string
		state      string
		wantUnbind bool
		wantErr    bool
	}{
		{
			name:       "bound device is unbound",
			state:      "bound",
			wantUnbind: true,
		},
		{
			name: "unbound device is accepted",
		},
		{
			name:    "other unbind errors are returned",
			state:   "invalid",
			wantErr: true,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			tempDir := t.TempDir()
			driverDir := filepath.Join(tempDir, "driver")
			unbindPath := filepath.Join(driverDir, "unbind")
			switch test.state {
			case "bound":
				assert.NoError(t, os.Mkdir(driverDir, 0o755))
				assert.NoError(t, os.WriteFile(unbindPath, nil, 0o600))
			case "invalid":
				assert.NoError(t, os.WriteFile(driverDir, nil, 0o600))
			}

			err := unbindPCIDeviceIfBound(unbindPath, bdf)
			if test.wantErr {
				assert.Error(t, err)
				return
			}
			assert.NoError(t, err)

			contents, err := os.ReadFile(unbindPath)
			if test.wantUnbind {
				assert.NoError(t, err)
				assert.Equal(t, bdf, string(contents))
			} else {
				assert.True(t, os.IsNotExist(err))
			}
		})
	}
}
