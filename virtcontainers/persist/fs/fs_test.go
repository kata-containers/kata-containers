// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package fs

import (
	"fmt"
	"os"
	"testing"

	persistapi "github.com/kata-containers/runtime/virtcontainers/persist/api"
	"github.com/stretchr/testify/assert"
)

func getFsDriver() (*FS, error) {
	driver, err := Init()
	if err != nil {
		return nil, fmt.Errorf("failed to init fs driver")
	}
	fs, ok := driver.(*FS)
	if !ok {
		return nil, fmt.Errorf("failed to convert driver to *FS")
	}

	return fs, nil
}

func TestFsLock(t *testing.T) {
	fs, err := getFsDriver()
	assert.Nil(t, err)
	assert.NotNil(t, fs)

	fs.sandboxState.SandboxContainer = "test-fs-driver"
	sandboxDir, err := fs.sandboxDir()
	assert.Nil(t, err)

	err = os.MkdirAll(sandboxDir, dirMode)
	assert.Nil(t, err)

	assert.Nil(t, fs.lock())
	assert.NotNil(t, fs.lock())

	assert.Nil(t, fs.unlock())
	assert.Nil(t, fs.unlock())
}

func TestFsDriver(t *testing.T) {
	fs, err := getFsDriver()
	assert.Nil(t, err)
	assert.NotNil(t, fs)

	ss := persistapi.SandboxState{}
	cs := make(map[string]persistapi.ContainerState)
	// missing sandbox container id
	assert.NotNil(t, fs.ToDisk(ss, cs))

	id := "test-fs-driver"
	ss.SandboxContainer = id
	assert.Nil(t, fs.ToDisk(ss, cs))

	// try non-existent dir
	_, _, err = fs.FromDisk("test-fs")
	assert.NotNil(t, err)

	// since we didn't call ToDisk, state is still empty in disk file
	ss, cs, err = fs.FromDisk(id)
	assert.Nil(t, err)
	assert.NotNil(t, ss)
	assert.Equal(t, len(cs), 0)

	assert.Equal(t, ss.SandboxContainer, id)
	assert.Equal(t, ss.State, "")

	// flush all to disk
	ss.State = "running"
	assert.Nil(t, fs.ToDisk(ss, cs))
	ss, cs, err = fs.FromDisk(id)
	assert.Nil(t, err)
	assert.NotNil(t, ss)
	assert.Equal(t, len(cs), 0)

	assert.Equal(t, ss.SandboxContainer, id)
	assert.Equal(t, ss.State, "running")

	assert.Nil(t, fs.Destroy())

	dir, err := fs.sandboxDir()
	assert.Nil(t, err)
	assert.NotEqual(t, len(dir), 0)

	_, err = os.Stat(dir)
	assert.NotNil(t, err)
	assert.True(t, os.IsNotExist(err))
}
