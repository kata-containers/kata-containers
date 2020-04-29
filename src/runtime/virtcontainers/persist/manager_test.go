// Copyright (c) 2019 Huawei Corporation
// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persist

import (
	"os"
	"testing"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/fs"
	"github.com/stretchr/testify/assert"
)

func TestGetDriverByName(t *testing.T) {
	nonexist, err := GetDriverByName("non-exist")
	assert.NotNil(t, err)
	assert.Nil(t, nonexist)

	fsDriver, err := GetDriverByName("fs")
	assert.Nil(t, err)
	assert.NotNil(t, fsDriver)
}

func TestGetDriver(t *testing.T) {
	assert := assert.New(t)
	orgMockTesting := mockTesting
	defer func() {
		mockTesting = orgMockTesting
	}()

	mockTesting = false

	fsd, err := GetDriver()
	assert.NoError(err)

	var expectedFS persistapi.PersistDriver
	if os.Getuid() != 0 {
		expectedFS, err = fs.RootlessInit()
	} else {
		expectedFS, err = fs.Init()
	}

	assert.NoError(err)
	assert.Equal(expectedFS, fsd)

	// Testing mock driver
	mockTesting = true
	fsd, err = GetDriver()
	assert.NoError(err)
	expectedFS, err = fs.MockFSInit()
	assert.NoError(err)
	assert.Equal(expectedFS, fsd)
}
