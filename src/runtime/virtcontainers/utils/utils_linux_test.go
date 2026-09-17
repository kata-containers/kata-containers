// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"errors"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestFindContextID(t *testing.T) {
	assert := assert.New(t)

	ioctlFunc = func(fd uintptr, request, arg1 uintptr) error {
		return errors.New("ioctl")
	}

	orgVHostVSockDevicePath := VHostVSockDevicePath
	orgMaxUInt := maxUInt
	defer func() {
		VHostVSockDevicePath = orgVHostVSockDevicePath
		maxUInt = orgMaxUInt
	}()
	VHostVSockDevicePath = "/dev/null"
	maxUInt = uint64(1000000)

	f, cid, err := FindContextID()
	assert.Nil(f)
	assert.Zero(cid)
	assert.Error(err)
}

func TestGetDevicePathAndFsTypeOptionsEmptyMount(t *testing.T) {
	assert := assert.New(t)
	_, _, _, err := GetDevicePathAndFsTypeOptions("")
	assert.Error(err)
}

func TestGetDevicePathAndFsTypeOptionsSuccessful(t *testing.T) {
	assert := assert.New(t)
	mountPoint := "/__kata_mount_test__/mount"
	mounts := "none /other ext4 rw 0 0\n" +
		"tmpfs " + mountPoint + " tmpfs rw,nosuid,nodev 0 0\n"

	path, fstype, fsOptions, err := getDevicePathAndFsTypeOptionsFromReader(mountPoint, strings.NewReader(mounts))
	assert.NoError(err)
	assert.Equal("tmpfs", path)
	assert.Equal("tmpfs", fstype)
	assert.Equal([]string{"rw", "nosuid", "nodev"}, fsOptions)
}

func TestGetDevicePathAndFsTypeOptionsErrors(t *testing.T) {
	assert := assert.New(t)

	_, _, _, err := getDevicePathAndFsTypeOptionsFromReader("/not-mounted", strings.NewReader("none /other ext4 rw 0 0\n"))
	assert.EqualError(err, "Mount /not-mounted not found")

	_, _, _, err = getDevicePathAndFsTypeOptionsFromReader("/not-mounted", strings.NewReader("invalid entry\n"))
	assert.ErrorContains(err, "Incorrect no of fields")
}
