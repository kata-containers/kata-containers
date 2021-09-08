// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package cgroups

import (
	"context"
	"io/ioutil"
	"os"
	"strings"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	"github.com/opencontainers/runc/libcontainer/configs"
	"github.com/stretchr/testify/assert"
)

//very very basic test; should be expanded
func TestNew(t *testing.T) {
	assert := assert.New(t)

	// create a cgroupfs cgroup manager
	c := &Config{
		Cgroups:    nil,
		CgroupPath: "",
	}

	mgr, err := New(c)
	assert.NoError(err)
	assert.NotNil(mgr.mgr)

	// create a systemd cgroup manager
	s := &Config{
		Cgroups:    nil,
		CgroupPath: "system.slice:kubepod:container",
	}

	mgr, err = New(s)
	assert.NoError(err)
	assert.NotNil(mgr.mgr)

}

func TestHypervisorDevices(t *testing.T) {
	assert := assert.New(t)

	devices := hypervisorDevices()
	assert.NotNil(devices)
	assert.NotEmpty(devices)
}

func TestWritePids(t *testing.T) {
	if rootless.IsRootless() {
		t.Skipf("Unable to write pids to cgroup.procs: running rootless")
		return
	}

	assert := assert.New(t)

	pids := []int{1}
	tmpDir := "/vc-pkg-cgroup-test-nil"
	err := writePids(pids, tmpDir)
	assert.Error(err)

	tmpDir, err = ioutil.TempDir("", "vc-pkg-cgroup-test")
	if err != nil {
		t.Skipf("no such path: %v", tmpDir)
		return
	}
	err = writePids(pids, tmpDir)
	assert.NoError(err)
}

func TestReadPids(t *testing.T) {
	if rootless.IsRootless() {
		t.Skipf("Unable to read pids from cgroup.procs: running rootless")
		return
	}

	assert := assert.New(t)

	pids := []int{1}
	tmpDir, err := ioutil.TempDir("", "vc-pkg-cgroup-test")
	if err != nil {
		t.Skipf("no such path: %v", tmpDir)
		return
	}
	err = writePids(pids, tmpDir)
	assert.NoError(err)
	pids, err = readPids(tmpDir)
	assert.NoError(err)
	assert.NotEmpty(pids)
	assert.Equal(1, pids[0])

	pids, err = readPids("/vc-pkg-cgroup-test-nil")
	assert.Error(err)
	assert.Nil(pids)
}

func TestMoveToParent(t *testing.T) {
	if rootless.IsRootless() {
		t.Skipf("Unable to move pids to parent: running rootless")
		return
	}

	assert := assert.New(t)

	pids := []int{1}
	path, err := ioutil.TempDir("", "vc-pkg-cgroup-test")
	if err != nil {
		t.Skipf("no such path: %v", path)
		return
	}
	err = writePids(pids, path)
	assert.NoError(err)

	// create a cgroupfs cgroup manager
	c := &Config{
		Cgroups:     nil,
		CgroupPath:  "system.slice:kubepod:container",
		CgroupPaths: map[string]string{"unit_test": path},
	}
	mgr, err := New(c)
	assert.NoError(err)
	assert.NotNil(mgr.mgr)

	err = mgr.moveToParent()
	assert.NoError(err)
	pids, err = readPids(path + "/..")
	assert.NoError(err)
	assert.NotEmpty(pids)
	assert.Equal(1, pids[0])
}

func TestMoveToParentWithErrorPath(t *testing.T) {
	assert := assert.New(t)

	// create a cgroupfs cgroup manager
	c := &Config{
		Cgroups:     nil,
		CgroupPath:  "system.slice:kubepod:container",
		CgroupPaths: map[string]string{"unit_test": "/tmp/vc-cgroup-not-exist"},
	}
	mgr, err := New(c)
	assert.NoError(err)
	assert.NotNil(mgr.mgr)

	err = mgr.moveToParent()
	assert.NoError(err)
}

func TestMoveToParentWithNoPids(t *testing.T) {
	assert := assert.New(t)

	path, err := ioutil.TempDir("", "vc-pkg-cgroup-test")
	if err != nil {
		t.Skipf("no such path: %v", path)
		return
	}

	// create a cgroupfs cgroup manager
	c := &Config{
		Cgroups:     nil,
		CgroupPath:  "system.slice:kubepod:container",
		CgroupPaths: map[string]string{"unit_test": path},
	}
	mgr, err := New(c)
	assert.NoError(err)
	assert.NotNil(mgr.mgr)

	err = mgr.moveToParent()
	assert.NoError(err)
}

func TestAdd(t *testing.T) {
	assert := assert.New(t)

	mgr := &Manager{
		mgr: NewMockManager(),
	}
	assert.NotNil(mgr.mgr)

	err := mgr.Add(1)
	assert.NoError(err)
}

func TestApply(t *testing.T) {
	assert := assert.New(t)

	mgr := &Manager{
		mgr: NewMockManager(),
	}
	assert.NotNil(mgr.mgr)

	err := mgr.Apply()
	assert.NoError(err)
}

func TestGetCgroups(t *testing.T) {
	assert := assert.New(t)

	mockCgroups := &configs.Cgroup{
		Name:        "mock",
		Path:        "",
		ScopePrefix: "",
		Paths:       map[string]string{},
		Resources:   nil,
	}
	// create a cgroupfs cgroup manager
	c := &Config{
		Cgroups:    mockCgroups,
		CgroupPath: "",
	}
	mgr, err := New(c)
	assert.NoError(err)
	assert.NotNil(mgr.mgr)

	cgroups, err := mgr.GetCgroups()
	assert.NoError(err)
	assert.NotNil(cgroups)
	assert.Equal(mockCgroups.Name, cgroups.Name)
}

func TestGetPaths(t *testing.T) {
	assert := assert.New(t)

	key := "unit_test"
	path := "/tmp/vc-cgroup-path-test"
	// create a cgroupfs cgroup manager
	c := &Config{
		Cgroups:     nil,
		CgroupPath:  "system.slice:kubepod:container",
		CgroupPaths: map[string]string{key: path},
	}
	mgr, err := New(c)
	assert.NoError(err)
	assert.NotNil(mgr.mgr)

	cgroupPaths := mgr.GetPaths()
	assert.NoError(err)
	assert.NotNil(cgroupPaths)
	value, ok := cgroupPaths[key]
	assert.True(ok)
	assert.Equal(path, value)
}

func TestDestroy(t *testing.T) {
	assert := assert.New(t)

	mgr := &Manager{
		mgr: NewMockManager(),
	}
	assert.NotNil(mgr.mgr)

	err := mgr.Destroy()
	assert.NoError(err)
}

func TestAddDevice(t *testing.T) {
	assert := assert.New(t)

	mgr := &Manager{
		mgr: NewMockManager(),
	}
	assert.NotNil(mgr.mgr)

	ctx := context.Background()
	device := "/dev/null"
	if _, err := os.Stat(device); os.IsNotExist(err) {
		t.Skipf("no such device: %v", device)
		return
	}
	err := mgr.AddDevice(ctx, device)
	assert.NoError(err)
}

func TestRemoveDevice(t *testing.T) {
	assert := assert.New(t)

	device := "/dev/null"
	if _, err := os.Stat(device); os.IsNotExist(err) {
		t.Skipf("no such device: %v", device)
		return
	}
	mgr := &Manager{
		mgr: NewMockManager(),
	}
	assert.NotNil(mgr.mgr)
	err := mgr.RemoveDevice(device)
	assert.NoError(err)

	device = "/dev/cdrom"
	if _, err := os.Stat(device); os.IsNotExist(err) {
		t.Skipf("no such device: %v", device)
		return
	}
	err = mgr.RemoveDevice(device)
	assert.Error(err)
	assert.True(strings.Contains(err.Error(), "not found in the cgroup"))
}
