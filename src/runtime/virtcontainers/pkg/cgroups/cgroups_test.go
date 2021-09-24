// Copyright (c) 2021 Inspur Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package cgroups

import (
	"testing"

	"github.com/containerd/cgroups"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

var tc ktu.TestConstraint

func init() {
	tc = ktu.NewTestConstraint(false)
}

func newCgroup() *Cgroup {
	return &Cgroup{
		cgroup:  &mockCgroup{},
		path:    "",
		cpusets: &specs.LinuxCPU{},
		devices: []specs.LinuxDeviceCgroup{},
	}
}

func TestSandboxDevices(t *testing.T) {
	assert := assert.New(t)

	devices := sandboxDevices()
	assert.NotNil(devices)

	assertDevices := []specs.LinuxDeviceCgroup{}
	defaultDevices := []string{
		"/dev/null",
		"/dev/random",
		"/dev/full",
		"/dev/tty",
		"/dev/zero",
		"/dev/urandom",
		"/dev/console",
		"/dev/kvm",
		"/dev/vhost-net",
		"/dev/vfio/vfio",
	}
	for _, device := range defaultDevices {
		ldevice, err := DeviceToLinuxDevice(device)
		if err != nil {
			continue
		}
		assertDevices = append(assertDevices, ldevice)
	}
	wildcardMajor := int64(-1)
	wildcardMinor := int64(-1)
	ptsMajor := int64(136)
	tunMajor := int64(10)
	tunMinor := int64(200)
	assertDevices = append(assertDevices, []specs.LinuxDeviceCgroup{
		// allow mknod for any device
		{
			Allow:  true,
			Type:   "c",
			Major:  &wildcardMajor,
			Minor:  &wildcardMinor,
			Access: "m",
		},
		{
			Allow:  true,
			Type:   "b",
			Major:  &wildcardMajor,
			Minor:  &wildcardMinor,
			Access: "m",
		},
		// /dev/pts/ - pts namespaces are "coming soon"
		{
			Allow:  true,
			Type:   "c",
			Major:  &ptsMajor,
			Minor:  &wildcardMinor,
			Access: "rwm",
		},
		// tuntap
		{
			Allow:  true,
			Type:   "c",
			Major:  &tunMajor,
			Minor:  &tunMinor,
			Access: "rwm",
		},
	}...)
	assert.Equal(len(assertDevices), len(devices))
	for i, assertCgroup := range assertDevices {
		assert.Equal(assertCgroup.Major, devices[i].Major)
		assert.Equal(assertCgroup.Minor, devices[i].Minor)
		assert.Equal(assertCgroup.Access, devices[i].Access)
		assert.Equal(assertCgroup.Type, devices[i].Type)
	}
}

func TestNewCgroup(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}
	assert := assert.New(t)

	for _, t := range []struct {
		path string
	}{
		{""},
		{"system.slice:kata:dfb3b2a6af34d"},
	} {
		cg, err := NewCgroup(t.path, &specs.LinuxResources{})
		assert.NoError(err)
		assert.NotNil(cg)
		_ = cg.Delete()
	}
}

func TestNewSandboxCgroup(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}
	assert := assert.New(t)

	for _, path := range []string{"", "system.slice:kata:dfb3b2a6af34d"} {
		cg, err := NewSandboxCgroup(path, &specs.LinuxResources{})
		assert.NoError(err)
		assert.NotNil(cg)
		_ = cg.Delete()
	}
}

func TestLoad(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}
	assert := assert.New(t)

	for _, t := range []struct {
		path         string
		createCgroup bool
		error        bool
	}{
		{"test", false, true},
		{"test", true, false},
	} {
		cg, _ := cgroups.New(cgroups.V1, cgroups.StaticPath(t.path), &specs.LinuxResources{})
		if !t.createCgroup && cg != nil {
			if err := cg.Delete(); err != nil {
				continue
			}
		}
		_, err := Load(t.path)
		if t.error {
			assert.Error(err)
		} else {
			assert.NoError(err)
			_ = cg.Delete()
		}
	}
}

func TestLogger(t *testing.T) {
	assert := assert.New(t)

	cg := &Cgroup{}
	logger := cg.Logger()
	assert.NotNil(logger)
}

func TestDelete(t *testing.T) {
	assert := assert.New(t)

	cg := newCgroup()
	err := cg.Delete()
	assert.NoError(err)
}

func TestStat(t *testing.T) {
	assert := assert.New(t)

	cg := newCgroup()
	metrics, err := cg.Stat()
	assert.NoError(err)
	assert.Nil(metrics)
}

func TestAddProcess(t *testing.T) {
	assert := assert.New(t)

	cg := newCgroup()
	err := cg.AddProcess(0, "test")
	assert.NoError(err)
}

func TestAddTask(t *testing.T) {
	assert := assert.New(t)

	cg := newCgroup()
	err := cg.AddTask(0, "test")
	assert.NoError(err)
}

func TestUpdate(t *testing.T) {
	assert := assert.New(t)

	cg := newCgroup()
	err := cg.Update(&specs.LinuxResources{})
	assert.NoError(err)
}

func TestMoveTo(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}
	assert := assert.New(t)

	path := "test"
	cg := newCgroup()
	err := cg.MoveTo(path)
	assert.Error(err)

	cGroup, err := cgroups.New(cgroups.V1, cgroups.StaticPath(path), &specs.LinuxResources{})
	if err != nil {
		t.Skipf("create cGroup failed, path: %s", path)
	}
	defer func() {
		_ = cGroup.Delete()
	}()
	err = cg.MoveTo(path)
	assert.NoError(err)
}

func TestMoveToParent(t *testing.T) {
	assert := assert.New(t)

	cg := newCgroup()

	err := cg.MoveToParent()
	assert.NoError(err)
}

func TestAddDevice(t *testing.T) {
	assert := assert.New(t)

	cg := newCgroup()
	err := cg.AddDevice("/dev/null")
	assert.NoError(err)
}

func TestRemoveDevice(t *testing.T) {
	assert := assert.New(t)

	cg := newCgroup()
	err := cg.RemoveDevice("/dev/null")
	assert.NoError(err)
}

func TestUpdateCpuSet(t *testing.T) {
	assert := assert.New(t)
	for _, t := range []struct {
		cpusets *specs.LinuxCPU
		cpuset  string
		memset  string
	}{
		{nil, "", ""},
		{nil, "1,2,4", "1024"},
		{&specs.LinuxCPU{}, "1", "512"},
	} {
		cg := &Cgroup{
			cgroup:  &mockCgroup{},
			path:    "",
			cpusets: t.cpusets,
			devices: nil,
		}
		err := cg.UpdateCpuSet(t.cpuset, t.memset)
		assert.NoError(err)
	}
}

func TestPath(t *testing.T) {
	assert := assert.New(t)

	cg := newCgroup()
	path := cg.Path()
	assert.NotNil(path)
	assert.Equal("", path)
}
