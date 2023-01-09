// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package manager

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/drivers"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/stretchr/testify/assert"

	"golang.org/x/sys/unix"
)

func TestAttachVhostUserBlkDevice(t *testing.T) {
	rootEnabled := true
	tc := ktu.NewTestConstraint(false)
	if tc.NotValid(ktu.NeedRoot()) {
		rootEnabled = false
	}

	tmpDir := t.TempDir()
	dm := &deviceManager{
		blockDriver:           config.VirtioBlock,
		devices:               make(map[string]api.Device),
		vhostUserStoreEnabled: true,
		vhostUserStorePath:    tmpDir,
	}

	vhostUserDevNodePath := filepath.Join(tmpDir, "/block/devices/")
	vhostUserSockPath := filepath.Join(tmpDir, "/block/sockets/")
	deviceNodePath := filepath.Join(vhostUserDevNodePath, "vhostblk0")
	deviceSockPath := filepath.Join(vhostUserSockPath, "vhostblk0")

	err := os.MkdirAll(vhostUserDevNodePath, dirMode)
	assert.Nil(t, err)
	err = os.MkdirAll(vhostUserSockPath, dirMode)
	assert.Nil(t, err)
	_, err = os.Create(deviceSockPath)
	assert.Nil(t, err)

	// mknod requires root privilege, call mock function for non-root to
	// get VhostUserBlk device type.
	if rootEnabled == true {
		err = unix.Mknod(deviceNodePath, unix.S_IFBLK, int(unix.Mkdev(config.VhostUserBlkMajor, 0)))
		assert.Nil(t, err)
	} else {
		savedFunc := config.GetVhostUserNodeStatFunc

		_, err = os.Create(deviceNodePath)
		assert.Nil(t, err)

		config.GetVhostUserNodeStatFunc = func(devNodePath string,
			devNodeStat *unix.Stat_t) error {
			if deviceNodePath != devNodePath {
				return fmt.Errorf("mock GetVhostUserNodeStatFunc error")
			}

			devNodeStat.Rdev = unix.Mkdev(config.VhostUserBlkMajor, 0)
			return nil
		}

		defer func() {
			config.GetVhostUserNodeStatFunc = savedFunc
		}()
	}

	path := "/dev/vda"
	deviceInfo := config.DeviceInfo{
		HostPath:      deviceNodePath,
		ContainerPath: path,
		DevType:       "b",
		Major:         config.VhostUserBlkMajor,
		Minor:         0,
	}

	devReceiver := &api.MockDeviceReceiver{}
	device, err := dm.NewDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok := device.(*drivers.VhostUserBlkDevice)
	assert.True(t, ok)

	err = device.Attach(context.Background(), devReceiver)
	assert.Nil(t, err)

	err = device.Detach(context.Background(), devReceiver)
	assert.Nil(t, err)
}
