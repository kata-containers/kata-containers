// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"errors"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestFindContextID(t *testing.T) {
	assert := assert.New(t)

	ioctlFunc = func(fd uintptr, request int, arg1 uint64) error {
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
