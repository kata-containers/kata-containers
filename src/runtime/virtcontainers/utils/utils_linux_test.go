// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"bytes"
	"errors"
	"os/exec"
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

	cmdStr := "grep ^proc  /proc/mounts"
	cmd := exec.Command("sh", "-c", cmdStr)
	output, err := cmd.Output()
	assert.NoError(err)

	data := bytes.Split(output, []byte(" "))
	fstypeOut := string(data[2])
	optsOut := strings.Split(string(data[3]), ",")

	path, fstype, fsOptions, err := GetDevicePathAndFsTypeOptions("/proc")
	assert.NoError(err)

	assert.Equal(path, "proc")
	assert.Equal(fstype, "proc")
	assert.Equal(fstype, fstypeOut)
	assert.Equal(fsOptions, optsOut)
}
