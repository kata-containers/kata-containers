// Copyright (c) 2019 Huawei Corporation
// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persist

import (
	"errors"
	"os"
	"strings"
	"testing"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/fs"
	"github.com/stretchr/testify/assert"
)

func TestGetDriverByName(t *testing.T) {
	nonexist, err := GetDriverByName("non-exist")
	assert.NotNil(t, err)
	assert.Nil(t, nonexist)

	// testing correct driver is returned
	fsDriver, err := GetDriverByName("fs")
	assert.Nil(t, err)
	assert.NotNil(t, fsDriver)

	// testing case when expErr is set
	expErr = errors.New("TEST-ERROR")
	defer func() {
		expErr = nil
	}()

	nonexist, err = GetDriverByName("fs")
	assert.NotNil(t, err)
	assert.Nil(t, nonexist)

	b := err.Error()
	assert.True(t, strings.Contains(b, "TEST-ERROR"))
}

func TestGetDriver(t *testing.T) {
	assert := assert.New(t)

	// testing correct driver is returned
	fsd, err := GetDriver()
	assert.NoError(err)

	var expectedFS persistapi.PersistDriver
	if os.Getuid() != 0 {
		expectedFS, err = fs.RootlessInit()
	} else {
		expectedFS, err = fs.Init()
	}

	assert.NoError(err)
	assert.Equal(expectedFS, fsd) // driver should match correct one for UID

	// testing case when expErr is set
	expErr = errors.New("TEST-ERROR")
	nonexist, err := GetDriver()
	assert.NotNil(err)
	assert.Nil(nonexist)
	expErr = nil

	// testing case when driver can't be found on supportedDrivers variable
	supportedDriversBU := supportedDrivers
	supportedDrivers = nil
	fsd, err = GetDriver()
	assert.Nil(fsd)
	assert.NotNil(err)
	b := err.Error()
	assert.True(strings.Contains(b, "Could not find a FS driver"))
	supportedDrivers = supportedDriversBU

	// testing case when mock driver is activated
	fs.EnableMockTesting(t.TempDir())
	mock, err := GetDriver()
	assert.NoError(err)
	expectedFS, err = fs.MockFSInit(fs.MockStorageRootPath())
	assert.NoError(err)
	assert.Equal(expectedFS, mock)

	fs.EnableMockTesting("")
}
