// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package fs

import (
	"fmt"
	"io/ioutil"
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

func TestFsLockShared(t *testing.T) {
	fs, err := getFsDriver()
	assert.Nil(t, err)
	assert.NotNil(t, fs)

	testDir, err := ioutil.TempDir("", "fs-tmp-")
	assert.Nil(t, err)
	TestSetRunStoragePath(testDir)
	defer func() {
		os.RemoveAll(testDir)
	}()

	sid := "test-fs-driver"
	fs.sandboxState.SandboxContainer = sid
	sandboxDir, err := fs.sandboxDir(sid)
	assert.Nil(t, err)

	err = os.MkdirAll(sandboxDir, dirMode)
	assert.Nil(t, err)

	// Take 2 shared locks
	unlockFunc, err := fs.Lock(sid, false)
	assert.Nil(t, err)

	unlockFunc2, err := fs.Lock(sid, false)
	assert.Nil(t, err)

	assert.Nil(t, unlockFunc())
	assert.Nil(t, unlockFunc2())
	assert.NotNil(t, unlockFunc2())
}

func TestFsLockExclusive(t *testing.T) {
	fs, err := getFsDriver()
	assert.Nil(t, err)
	assert.NotNil(t, fs)

	sid := "test-fs-driver"
	fs.sandboxState.SandboxContainer = sid
	sandboxDir, err := fs.sandboxDir(sid)
	assert.Nil(t, err)

	err = os.MkdirAll(sandboxDir, dirMode)
	assert.Nil(t, err)

	// Take 1 exclusive lock
	unlockFunc, err := fs.Lock(sid, true)
	assert.Nil(t, err)

	assert.Nil(t, unlockFunc())

	unlockFunc, err = fs.Lock(sid, true)
	assert.Nil(t, err)

	assert.Nil(t, unlockFunc())
	assert.NotNil(t, unlockFunc())
}

func TestFsDriver(t *testing.T) {
	fs, err := getFsDriver()
	assert.Nil(t, err)
	assert.NotNil(t, fs)

	testDir, err := ioutil.TempDir("", "fs-tmp-")
	assert.Nil(t, err)
	TestSetRunStoragePath(testDir)
	defer func() {
		os.RemoveAll(testDir)
	}()

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

	// flush all to disk.
	ss.State = "running"
	assert.Nil(t, fs.ToDisk(ss, cs))
	ss, cs, err = fs.FromDisk(id)
	assert.Nil(t, err)
	assert.NotNil(t, ss)
	assert.Equal(t, len(cs), 0)

	assert.Equal(t, ss.SandboxContainer, id)
	assert.Equal(t, ss.State, "running")

	// add new container state.
	cs["test-container"] = persistapi.ContainerState{
		State: "ready",
	}
	assert.Nil(t, fs.ToDisk(ss, cs))
	ss, cs, err = fs.FromDisk(id)
	assert.Nil(t, err)
	assert.NotNil(t, ss)
	assert.Equal(t, len(cs), 1)
	c, ok := cs["test-container"]
	assert.True(t, ok)
	assert.Equal(t, c.State, "ready")

	// clean all container.
	cs = make(map[string]persistapi.ContainerState)
	assert.Nil(t, fs.ToDisk(ss, cs))
	ss, cs, err = fs.FromDisk(id)
	assert.Nil(t, err)
	assert.NotNil(t, ss)
	assert.Equal(t, len(cs), 0)

	// destroy whole sandbox dir.
	assert.Nil(t, fs.Destroy(id))

	dir, err := fs.sandboxDir(id)
	assert.Nil(t, err)
	assert.NotEqual(t, len(dir), 0)

	_, err = os.Stat(dir)
	assert.NotNil(t, err)
	assert.True(t, os.IsNotExist(err))
}
