// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/drivers"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/manager"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"

	"github.com/stretchr/testify/assert"
	"golang.org/x/sys/unix"
)

func TestSandboxAttachDevicesVhostUserBlk(t *testing.T) {
	rootEnabled := true
	tc := ktu.NewTestConstraint(false)
	if tc.NotValid(ktu.NeedRoot()) {
		rootEnabled = false
	}

	tmpDir := t.TempDir()
	os.RemoveAll(tmpDir)
	dm := manager.NewDeviceManager(config.VirtioSCSI, true, tmpDir, 0, nil)

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

	device, err := dm.NewDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok := device.(*drivers.VhostUserBlkDevice)
	assert.True(t, ok)

	c := &Container{
		id: "100",
		devices: []ContainerDevice{
			{
				ID:            device.DeviceID(),
				ContainerPath: path,
			},
		},
	}

	containers := map[string]*Container{}
	containers[c.id] = c

	sandbox := Sandbox{
		id:         "100",
		containers: containers,
		hypervisor: &mockHypervisor{},
		devManager: dm,
		ctx:        context.Background(),
		config:     &SandboxConfig{},
	}

	containers[c.id].sandbox = &sandbox

	err = containers[c.id].attachDevices(context.Background())
	assert.Nil(t, err, "Error while attaching vhost-user-blk devices %s", err)

	err = containers[c.id].detachDevices(context.Background())
	assert.Nil(t, err, "Error while detaching vhost-user-blk devices %s", err)
}
