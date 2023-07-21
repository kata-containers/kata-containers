// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package manager

import (
	"context"
	"os"
	"path/filepath"
	"strconv"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/drivers"
	"github.com/stretchr/testify/assert"
)

const fileMode0640 = os.FileMode(0640)

// dirMode is the permission bits used for creating a directory
const dirMode = os.FileMode(0750) | os.ModeDir

func TestNewDevice(t *testing.T) {
	dm := &deviceManager{
		blockDriver: config.VirtioBlock,
		devices:     make(map[string]api.Device),
	}
	savedSysDevPrefix := config.SysDevPrefix

	major := int64(252)
	minor := int64(3)

	config.SysDevPrefix = t.TempDir()
	defer func() {
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

	_, err := dm.NewDevice(deviceInfo)
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
	err = os.WriteFile(ueventPath, content, fileMode0640)
	assert.Nil(t, err)

	_, err = dm.NewDevice(deviceInfo)
	assert.NotNil(t, err)

	content = []byte("MAJOR=252\nMINOR=3\nDEVNAME=vfio/2")
	err = os.WriteFile(ueventPath, content, fileMode0640)
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

func TestAttachVFIOAPDevice(t *testing.T) {

	var err error
	var ok bool

	dm := &deviceManager{
		devices: make(map[string]api.Device),
	}

	tmpDir := t.TempDir()
	// sys/devices/vfio_ap/matrix/f94290f8-78ac-45fb-bb22-e55e519fa64f
	testSysfsAP := "/sys/devices/vfio_ap/"
	testDeviceAP := "f94290f8-78ac-45fb-bb22-e55e519fa64f"
	testVFIOGroup := "42"

	matrixDir := filepath.Join(tmpDir, testSysfsAP, "matrix")
	err = os.MkdirAll(matrixDir, dirMode)
	assert.Nil(t, err)

	deviceAPFile := filepath.Join(matrixDir, testDeviceAP)
	err = os.MkdirAll(deviceAPFile, dirMode)
	assert.Nil(t, err)

	matrixDeviceAPFile := filepath.Join(deviceAPFile, "matrix")
	_, err = os.Create(matrixDeviceAPFile)
	assert.Nil(t, err)
	// create AP devices in the matrix file
	APDevices := []byte("05.001f\n")
	err = os.WriteFile(matrixDeviceAPFile, APDevices, 0644)
	assert.Nil(t, err)

	devicesVFIOGroupDir := filepath.Join(tmpDir, testVFIOGroup, "devices")
	err = os.MkdirAll(devicesVFIOGroupDir, dirMode)
	assert.Nil(t, err)

	deviceAPSymlink := filepath.Join(devicesVFIOGroupDir, testDeviceAP)
	err = os.Symlink(deviceAPFile, deviceAPSymlink)
	assert.Nil(t, err)

	savedIOMMUPath := config.SysIOMMUGroupPath
	config.SysIOMMUGroupPath = tmpDir

	savedSysBusPciDevicesPath := config.SysBusPciDevicesPath
	config.SysBusPciDevicesPath = devicesVFIOGroupDir

	defer func() {
		config.SysIOMMUGroupPath = savedIOMMUPath
		config.SysBusPciDevicesPath = savedSysBusPciDevicesPath
	}()

	path := filepath.Join(vfioPath, testVFIOGroup)
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "c",
		ColdPlug:      false,
		Port:          config.RootPort,
	}

	device, err := dm.NewDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok = device.(*drivers.VFIODevice)
	assert.True(t, ok)

	devReceiver := &api.MockDeviceReceiver{}
	err = device.Attach(context.Background(), devReceiver)
	assert.Nil(t, err)

	err = device.Detach(context.Background(), devReceiver)
	assert.Nil(t, err)

	// If we omit the port setting we should fail
	failDm := &deviceManager{
		devices: make(map[string]api.Device),
	}

	failDeviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "c",
		ColdPlug:      false,
	}

	failDevice, err := failDm.NewDevice(failDeviceInfo)
	assert.Nil(t, err)
	_, ok = failDevice.(*drivers.VFIODevice)
	assert.True(t, ok)

	failDevReceiver := &api.MockDeviceReceiver{}
	err = failDevice.Attach(context.Background(), failDevReceiver)
	assert.Error(t, err)

}

func TestAttachVFIODevice(t *testing.T) {
	dm := &deviceManager{
		blockDriver: config.VirtioBlock,
		devices:     make(map[string]api.Device),
	}
	tmpDir := t.TempDir()

	testFDIOGroup := "2"
	testDeviceBDFPath := "0000:00:1c.0"

	devicesDir := filepath.Join(tmpDir, testFDIOGroup, "devices")
	err := os.MkdirAll(devicesDir, dirMode)
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

	savedIOMMUPath := config.SysIOMMUGroupPath
	config.SysIOMMUGroupPath = tmpDir

	savedSysBusPciDevicesPath := config.SysBusPciDevicesPath
	config.SysBusPciDevicesPath = devicesDir

	defer func() {
		config.SysIOMMUGroupPath = savedIOMMUPath
		config.SysBusPciDevicesPath = savedSysBusPciDevicesPath
	}()

	path := filepath.Join(vfioPath, testFDIOGroup)
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "c",
		ColdPlug:      false,
		Port:          config.RootPort,
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
		blockDriver: config.VirtioBlock,
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
		blockDriver: config.VirtioBlock,
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
	dm.blockDriver = config.VirtioSCSI
	device, err = dm.NewDevice(deviceInfo)
	assert.Nil(t, err)
	err = device.Attach(context.Background(), devReceiver)
	assert.Nil(t, err)

	err = device.Detach(context.Background(), devReceiver)
	assert.Nil(t, err)
}

func TestAttachDetachDevice(t *testing.T) {
	dm := NewDeviceManager(config.VirtioSCSI, false, "", 0, nil)

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
