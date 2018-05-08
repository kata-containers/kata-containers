// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package manager

import (
	"io/ioutil"
	"os"
	"path/filepath"
	"strconv"
	"testing"

	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
)

const fileMode0640 = os.FileMode(0640)

// dirMode is the permission bits used for creating a directory
const dirMode = os.FileMode(0750) | os.ModeDir

func TestNewDevices(t *testing.T) {
	dm := &deviceManager{
		blockDriver: VirtioBlock,
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

	_, err = dm.NewDevices([]config.DeviceInfo{deviceInfo})
	assert.NotNil(t, err)

	format := strconv.FormatInt(major, 10) + ":" + strconv.FormatInt(minor, 10)
	ueventPathPrefix := filepath.Join(config.SysDevPrefix, "char", format)
	ueventPath := filepath.Join(ueventPathPrefix, "uevent")

	// Return true for non-existent /sys/dev path.
	deviceInfo.ContainerPath = path
	_, err = dm.NewDevices([]config.DeviceInfo{deviceInfo})
	assert.Nil(t, err)

	err = os.MkdirAll(ueventPathPrefix, dirMode)
	assert.Nil(t, err)

	// Should return error for bad data in uevent file
	content := []byte("nonkeyvaluedata")
	err = ioutil.WriteFile(ueventPath, content, fileMode0640)
	assert.Nil(t, err)

	_, err = dm.NewDevices([]config.DeviceInfo{deviceInfo})
	assert.NotNil(t, err)

	content = []byte("MAJOR=252\nMINOR=3\nDEVNAME=vfio/2")
	err = ioutil.WriteFile(ueventPath, content, fileMode0640)
	assert.Nil(t, err)

	devices, err := dm.NewDevices([]config.DeviceInfo{deviceInfo})
	assert.Nil(t, err)

	assert.Equal(t, len(devices), 1)
	vfioDev, ok := devices[0].(*drivers.VFIODevice)
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
	}
	tmpDir, err := ioutil.TempDir("", "")
	assert.Nil(t, err)
	os.RemoveAll(tmpDir)

	testFDIOGroup := "2"
	testDeviceBDFPath := "0000:00:1c.0"

	devicesDir := filepath.Join(tmpDir, testFDIOGroup, "devices")
	err = os.MkdirAll(devicesDir, dirMode)
	assert.Nil(t, err)

	deviceFile := filepath.Join(devicesDir, testDeviceBDFPath)
	_, err = os.Create(deviceFile)
	assert.Nil(t, err)

	savedIOMMUPath := config.SysIOMMUPath
	config.SysIOMMUPath = tmpDir

	defer func() {
		config.SysIOMMUPath = savedIOMMUPath
	}()

	path := filepath.Join(vfioPath, testFDIOGroup)
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "c",
	}

	device, err := dm.createDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok := device.(*drivers.VFIODevice)
	assert.True(t, ok)

	devReceiver := &api.MockDeviceReceiver{}
	err = device.Attach(devReceiver)
	assert.Nil(t, err)

	err = device.Detach(devReceiver)
	assert.Nil(t, err)
}

func TestAttachGenericDevice(t *testing.T) {
	dm := &deviceManager{
		blockDriver: VirtioBlock,
	}
	path := "/dev/tty2"
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "c",
	}

	device, err := dm.createDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok := device.(*drivers.GenericDevice)
	assert.True(t, ok)

	devReceiver := &api.MockDeviceReceiver{}
	err = device.Attach(devReceiver)
	assert.Nil(t, err)

	err = device.Detach(devReceiver)
	assert.Nil(t, err)
}

func TestAttachBlockDevice(t *testing.T) {
	dm := &deviceManager{
		blockDriver: VirtioBlock,
	}
	path := "/dev/hda"
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "b",
	}

	devReceiver := &api.MockDeviceReceiver{}
	device, err := dm.createDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok := device.(*drivers.BlockDevice)
	assert.True(t, ok)

	err = device.Attach(devReceiver)
	assert.Nil(t, err)

	err = device.Detach(devReceiver)
	assert.Nil(t, err)

	// test virtio SCSI driver
	dm.blockDriver = VirtioSCSI
	device, err = dm.createDevice(deviceInfo)
	assert.Nil(t, err)
	err = device.Attach(devReceiver)
	assert.Nil(t, err)

	err = device.Detach(devReceiver)
	assert.Nil(t, err)
}
