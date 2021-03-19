// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package manager

import (
	"context"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strconv"
	"testing"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/drivers"
	"github.com/stretchr/testify/assert"

	"golang.org/x/sys/unix"
)

const fileMode0640 = os.FileMode(0640)

// dirMode is the permission bits used for creating a directory
const dirMode = os.FileMode(0750) | os.ModeDir

func TestNewDevice(t *testing.T) {
	dm := &deviceManager{
		blockDriver: VirtioBlock,
		devices:     make(map[string]api.Device),
	}
	savedSysDevPrefix := config.SysDevPrefix

	major := int64(252)
	minor := int64(3)

	tmpDir, err := ioutil.TempDir("", "")
	assert.Nil(t, err)

	config.SysDevPrefix = tmpDir
	defer func() {
		os.RemoveAll(tmpDir)
		config.SysDevPrefix = savedSysDevPrefix
	}()

	path := "/dev/vfio/2"
	deviceInfo := config.DeviceInfo{
		ContainerPath: "",
		Major:         major,
		Minor:         minor,
		UID:           2,
		GID:           2,
		DevType:       "c",
	}

	_, err = dm.NewDevice(deviceInfo)
	assert.NotNil(t, err)

	format := strconv.FormatInt(major, 10) + ":" + strconv.FormatInt(minor, 10)
	ueventPathPrefix := filepath.Join(config.SysDevPrefix, "char", format)
	ueventPath := filepath.Join(ueventPathPrefix, "uevent")

	// Return true for non-existent /sys/dev path.
	deviceInfo.ContainerPath = path
	_, err = dm.NewDevice(deviceInfo)
	assert.Nil(t, err)

	err = os.MkdirAll(ueventPathPrefix, dirMode)
	assert.Nil(t, err)

	// Should return error for bad data in uevent file
	content := []byte("nonkeyvaluedata")
	err = ioutil.WriteFile(ueventPath, content, fileMode0640)
	assert.Nil(t, err)

	_, err = dm.NewDevice(deviceInfo)
	assert.NotNil(t, err)

	content = []byte("MAJOR=252\nMINOR=3\nDEVNAME=vfio/2")
	err = ioutil.WriteFile(ueventPath, content, fileMode0640)
	assert.Nil(t, err)

	device, err := dm.NewDevice(deviceInfo)
	assert.Nil(t, err)

	vfioDev, ok := device.(*drivers.VFIODevice)
	assert.True(t, ok)
	assert.Equal(t, vfioDev.DeviceInfo.HostPath, path)
	assert.Equal(t, vfioDev.DeviceInfo.ContainerPath, path)
	assert.Equal(t, vfioDev.DeviceInfo.DevType, "c")
	assert.Equal(t, vfioDev.DeviceInfo.Major, major)
	assert.Equal(t, vfioDev.DeviceInfo.Minor, minor)
	assert.Equal(t, vfioDev.DeviceInfo.UID, uint32(2))
	assert.Equal(t, vfioDev.DeviceInfo.GID, uint32(2))
}

func TestAttachVFIODevice(t *testing.T) {
	dm := &deviceManager{
		blockDriver: VirtioBlock,
		devices:     make(map[string]api.Device),
	}
	tmpDir, err := ioutil.TempDir("", "")
	assert.Nil(t, err)
	defer os.RemoveAll(tmpDir)

	testFDIOGroup := "2"
	testDeviceBDFPath := "0000:00:1c.0"

	devicesDir := filepath.Join(tmpDir, testFDIOGroup, "devices")
	err = os.MkdirAll(devicesDir, dirMode)
	assert.Nil(t, err)

	deviceBDFDir := filepath.Join(devicesDir, testDeviceBDFPath)
	err = os.MkdirAll(deviceBDFDir, dirMode)
	assert.Nil(t, err)

	deviceClassFile := filepath.Join(deviceBDFDir, "class")
	_, err = os.Create(deviceClassFile)
	assert.Nil(t, err)

	deviceConfigFile := filepath.Join(deviceBDFDir, "config")
	_, err = os.Create(deviceConfigFile)
	assert.Nil(t, err)

	savedIOMMUPath := config.SysIOMMUPath
	config.SysIOMMUPath = tmpDir

	savedSysBusPciDevicesPath := config.SysBusPciDevicesPath
	config.SysBusPciDevicesPath = devicesDir

	defer func() {
		config.SysIOMMUPath = savedIOMMUPath
		config.SysBusPciDevicesPath = savedSysBusPciDevicesPath
	}()

	path := filepath.Join(vfioPath, testFDIOGroup)
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "c",
	}

	device, err := dm.NewDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok := device.(*drivers.VFIODevice)
	assert.True(t, ok)

	devReceiver := &api.MockDeviceReceiver{}
	err = device.Attach(context.Background(), devReceiver)
	assert.Nil(t, err)

	err = device.Detach(context.Background(), devReceiver)
	assert.Nil(t, err)
}

func TestAttachGenericDevice(t *testing.T) {
	dm := &deviceManager{
		blockDriver: VirtioBlock,
		devices:     make(map[string]api.Device),
	}
	path := "/dev/tty2"
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "c",
	}

	device, err := dm.NewDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok := device.(*drivers.GenericDevice)
	assert.True(t, ok)

	devReceiver := &api.MockDeviceReceiver{}
	err = device.Attach(context.Background(), devReceiver)
	assert.Nil(t, err)

	err = device.Detach(context.Background(), devReceiver)
	assert.Nil(t, err)
}

func TestAttachBlockDevice(t *testing.T) {
	dm := &deviceManager{
		blockDriver: VirtioBlock,
		devices:     make(map[string]api.Device),
	}
	path := "/dev/hda"
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "b",
	}

	devReceiver := &api.MockDeviceReceiver{}
	device, err := dm.NewDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok := device.(*drivers.BlockDevice)
	assert.True(t, ok)

	err = device.Attach(context.Background(), devReceiver)
	assert.Nil(t, err)

	err = device.Detach(context.Background(), devReceiver)
	assert.Nil(t, err)

	// test virtio SCSI driver
	dm.blockDriver = VirtioSCSI
	device, err = dm.NewDevice(deviceInfo)
	assert.Nil(t, err)
	err = device.Attach(context.Background(), devReceiver)
	assert.Nil(t, err)

	err = device.Detach(context.Background(), devReceiver)
	assert.Nil(t, err)
}

func TestAttachVhostUserBlkDevice(t *testing.T) {
	rootEnabled := true
	tc := ktu.NewTestConstraint(false)
	if tc.NotValid(ktu.NeedRoot()) {
		rootEnabled = false
	}

	tmpDir, err := ioutil.TempDir("", "")
	dm := &deviceManager{
		blockDriver:           VirtioBlock,
		devices:               make(map[string]api.Device),
		vhostUserStoreEnabled: true,
		vhostUserStorePath:    tmpDir,
	}
	assert.Nil(t, err)
	defer os.RemoveAll(tmpDir)

	vhostUserDevNodePath := filepath.Join(tmpDir, "/block/devices/")
	vhostUserSockPath := filepath.Join(tmpDir, "/block/sockets/")
	deviceNodePath := filepath.Join(vhostUserDevNodePath, "vhostblk0")
	deviceSockPath := filepath.Join(vhostUserSockPath, "vhostblk0")

	err = os.MkdirAll(vhostUserDevNodePath, dirMode)
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

func TestAttachDetachDevice(t *testing.T) {
	dm := NewDeviceManager(VirtioSCSI, false, "", nil)

	path := "/dev/hda"
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "b",
	}

	devReceiver := &api.MockDeviceReceiver{}
	device, err := dm.NewDevice(deviceInfo)
	assert.Nil(t, err)

	// attach non-exist device
	err = dm.AttachDevice(context.Background(), "non-exist", devReceiver)
	assert.NotNil(t, err)

	// attach device
	err = dm.AttachDevice(context.Background(), device.DeviceID(), devReceiver)
	assert.Nil(t, err)
	assert.Equal(t, device.GetAttachCount(), uint(1), "attach device count should be 1")
	// attach device again(twice)
	err = dm.AttachDevice(context.Background(), device.DeviceID(), devReceiver)
	assert.Nil(t, err)
	assert.Equal(t, device.GetAttachCount(), uint(2), "attach device count should be 2")

	attached := dm.IsDeviceAttached(device.DeviceID())
	assert.True(t, attached)

	// detach device
	err = dm.DetachDevice(context.Background(), device.DeviceID(), devReceiver)
	assert.Nil(t, err)
	assert.Equal(t, device.GetAttachCount(), uint(1), "attach device count should be 1")
	// detach device again(twice)
	err = dm.DetachDevice(context.Background(), device.DeviceID(), devReceiver)
	assert.Nil(t, err)
	assert.Equal(t, device.GetAttachCount(), uint(0), "attach device count should be 0")
	// detach device again should report error
	err = dm.DetachDevice(context.Background(), device.DeviceID(), devReceiver)
	assert.NotNil(t, err)
	assert.Equal(t, err, ErrDeviceNotAttached, "")
	assert.Equal(t, device.GetAttachCount(), uint(0), "attach device count should be 0")

	attached = dm.IsDeviceAttached(device.DeviceID())
	assert.False(t, attached)

	err = dm.RemoveDevice(device.DeviceID())
	assert.Nil(t, err)
}
