// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/json"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"syscall"
	"testing"

	"github.com/kata-containers/directvolume"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/stretchr/testify/assert"
)

func WriteJsonFile(obj interface{}, file string) error {
	maps := make(map[string]interface{})
	t := reflect.TypeOf(obj)
	v := reflect.ValueOf(obj)
	for i := 0; i < v.NumField(); i++ {
		if v.Field(i).String() != "" {
			maps[t.Field(i).Name] = v.Field(i).String()
		}
	}
	rankingsJSON, _ := json.Marshal(maps)
	if err := ioutil.WriteFile(file, rankingsJSON, 0644); err != nil {
		return err
	}
	return nil
}

func CreateAndMountLoopbackDevice(devicePath, mountPath string) (string, error) {
	if _, err := exec.Command("fallocate", "-l", "256K", devicePath).CombinedOutput(); err != nil {
		return "", err
	}
	output, err := exec.Command("losetup", "-f").CombinedOutput()
	if err != nil {
		return "", err
	}
	loopDev := strings.TrimSpace(string(output[:]))
	if _, err = exec.Command("mkfs.ext4", "-F", devicePath).CombinedOutput(); err != nil {
		return "", err
	}
	if _, err = exec.Command("losetup", loopDev, devicePath).CombinedOutput(); err != nil {
		return "", err
	}
	if err = os.Mkdir(mountPath, 0755); err != nil {
		return "", err
	}
	if err = syscall.Mount(loopDev, mountPath, "ext4", uintptr(0), ""); err != nil {
		return "", err
	}
	return loopDev, nil
}

func CleanupLoopbackDevice(loopDev, devicePath, mountPath string) {
	if mountPath != "" {
		_ = syscall.Unmount(mountPath, 0)
	}
	if loopDev != "" {
		_, _ = exec.Command("losetup", "-d", loopDev).CombinedOutput()
	}
	if _, err := os.Stat(devicePath); err == nil {
		_ = os.RemoveAll(devicePath)
	}
}

func TestGetDirectAssignedDiskMountInfoNoFile(t *testing.T) {
	_, err := getDirectAssignedDiskMountInfo("")
	assert.Error(t, err)
}

func TestGetDirectAssignedDiskMountInfoNoJson(t *testing.T) {
	file, err := ioutil.TempFile("", "testnojson")
	assert.NoError(t, err)
	defer os.Remove(file.Name())
	defer file.Close()

	_, err = getDirectAssignedDiskMountInfo(file.Name())
	assert.Error(t, err)
}

func TestGetDirectAssignedDiskMountInfoNotJson(t *testing.T) {
	file, err := ioutil.TempFile("", "testnot.json")
	assert.NoError(t, err)
	defer os.Remove(file.Name())
	defer file.Close()

	_, err = file.WriteString("foobar")
	assert.NoError(t, err)

	_, err = getDirectAssignedDiskMountInfo(file.Name())
	assert.Error(t, err)
}

func TestGetDirectAssignedDiskMountInfoUnexpectedJson(t *testing.T) {
	file, err := ioutil.TempFile("", "test-weird.json")
	assert.NoError(t, err)
	defer os.Remove(file.Name())
	defer file.Close()

	m := directvolume.DiskMountInfo{
		Device:     "/dev/loop13",
		VolumeType: "blk-filesystem",
		FsType:     "ext4",
		Options:    "ro",
	}

	_, err = file.WriteString("{\"Device\":\"/dev/loop13\",\"TargetPath\":\"/configs\",\"VolumeType\":\"blk-filesystem\",\"FsType\":\"ext4\",\"Options\":\"ro\", \"spaghetti\":\"overcooked\"}")
	assert.NoError(t, err)

	resDiskInfo, err := getDirectAssignedDiskMountInfo(file.Name())
	assert.NoError(t, err)

	// expect to read back m:
	assert.Equal(t, m, resDiskInfo)
}

func TestGetDirectAssignedDiskMountInfoValidJson(t *testing.T) {
	file, err := ioutil.TempFile("", "test.json")
	assert.NoError(t, err)
	defer os.Remove(file.Name())
	defer file.Close()

	m := directvolume.DiskMountInfo{
		Device:     "/dev/xda",
		VolumeType: "blk-filesystem",
	}

	err = WriteJsonFile(m, file.Name())
	assert.NoError(t, err)

	resDiskInfo, err := getDirectAssignedDiskMountInfo(file.Name())
	assert.NoError(t, err)

	// expect to read back m:
	assert.Equal(t, m, resDiskInfo)
}

func TestIsFileOnSameDeviceAsParentEmptyPath(t *testing.T) {
	fileOnMountedDevice, err := isFileOnSameDeviceAsParent("")
	assert.NoError(t, err)
	assert.False(t, fileOnMountedDevice)
}

func TestIsFileOnSameDeviceAsParentInvalidPath(t *testing.T) {
	fileOnMountedDevice, err := isFileOnSameDeviceAsParent("/totally/invalid/path")
	assert.Error(t, err)
	assert.False(t, fileOnMountedDevice)
}

func TestIsFileOnSameDeviceAsParentNotMountPoint(t *testing.T) {
	tmpdir, err := os.MkdirTemp(testDir, "TestIsFileOnSameDeviceAsParentNotMountPoint")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	fileDir := filepath.Join(tmpdir, "dir")
	err = os.MkdirAll(fileDir, 0755)
	assert.NoError(t, err)

	filePath := filepath.Join(fileDir, "test.json")
	_, err = os.Create(filePath)
	assert.NoError(t, err)

	fileOnMountedDevice, err := isFileOnSameDeviceAsParent(filePath)
	assert.NoError(t, err)
	assert.False(t, fileOnMountedDevice)
}

func TestIsFileOnSameDeviceAsParentDifferentDevice(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	tmpdir, err := os.MkdirTemp("", "TestIsFileOnSameDeviceAsParentDifferentDevice")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	diskPath := filepath.Join(tmpdir, "test-disk")
	mountPath := filepath.Join(tmpdir, "dir")
	loopDev, err := CreateAndMountLoopbackDevice(diskPath, mountPath)
	assert.NoError(t, err)
	defer CleanupLoopbackDevice(loopDev, diskPath, mountPath)

	filePath := filepath.Join(mountPath, "test.json")
	_, err = os.Create(filePath)
	assert.NoError(t, err)

	fileOnMountedDevice, err := isFileOnSameDeviceAsParent(filePath)
	assert.NoError(t, err)
	assert.True(t, fileOnMountedDevice)
}

func TestIsFileOnSameDeviceAsParentDifferentRootDisk(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	tmpdir, err := os.MkdirTemp("", "TestIsFileOnSameDeviceAsParentDifferentRootDisk")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	diskPath := filepath.Join(tmpdir, "test-disk")
	mountPath := filepath.Join(tmpdir, "dir")
	loopDev, err := CreateAndMountLoopbackDevice(diskPath, mountPath)
	assert.NoError(t, err)
	defer CleanupLoopbackDevice(loopDev, diskPath, mountPath)

	dirPath := filepath.Join(mountPath, "volume-dir")
	err = os.MkdirAll(dirPath, 0755)
	assert.NoError(t, err)

	filePath := filepath.Join(dirPath, "test.json")
	_, err = os.Create(filePath)
	assert.NoError(t, err)

	fileOnMountedDevice, err := isFileOnSameDeviceAsParent(filePath)
	assert.NoError(t, err)
	assert.False(t, fileOnMountedDevice)
}
