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

	fs.AddSaveCallback("test", func(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) error {
		return nil
	})
	// missing sandbox container id
	assert.NotNil(t, fs.ToDisk())

	id := "test-fs-driver"
	// missing sandbox container id
	fs.AddSaveCallback("test", func(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) error {
		ss.SandboxContainer = id
		return nil
	})
	assert.Nil(t, fs.ToDisk())

	fs.AddSaveCallback("test1", func(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) error {
		ss.State = "running"
		return nil
	})

	// try non-existent dir
	assert.NotNil(t, fs.Restore("test-fs"))

	// since we didn't call ToDisk, Callbacks are not invoked, and state is still empty in disk file
	assert.Nil(t, fs.Restore(id))
	ss, cs, err := fs.GetStates()
	assert.Nil(t, err)
	assert.NotNil(t, ss)
	assert.Equal(t, len(cs), 0)

	assert.Equal(t, ss.SandboxContainer, id)
	assert.Equal(t, ss.State, "")

	// flush all to disk
	assert.Nil(t, fs.ToDisk())
	assert.Nil(t, fs.Restore(id))
	ss, cs, err = fs.GetStates()
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
