// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package cgroups

import (
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestIsSystemdCgroup(t *testing.T) {
	assert := assert.New(t)

	tests := []struct {
		path     string
		expected bool
	}{
		{"foo.slice:kata:afhts2e5d4g5s", true},
		{"system.slice:kata:afhts2e5d4g5s", true},
		{"/kata/afhts2e5d4g5s", false},
		{"a:b:c:d", false},
		{":::", false},
		{"", false},
		{":", false},
		{"::", false},
		{":::", false},
		{"a:b", false},
		{"a:b:", false},
		{":a:b", false},
		{"@:@:@", false},
	}

	for _, t := range tests {
		assert.Equal(t.expected, IsSystemdCgroup(t.path), "invalid systemd cgroup path: %v", t.path)
	}
}

func TestValidCgroupPath(t *testing.T) {
	assert := assert.New(t)

	for _, t := range []struct {
		path          string
		systemdCgroup bool
		error         bool
	}{
		// empty paths
		{"../../../", false, false},
		{"../", false, false},
		{".", false, false},
		{"../../../", false, false},
		{"./../", false, false},

		// valid no-systemd paths
		{"../../../foo", false, false},
		{"/../hi", false, false},
		{"/../hi/foo", false, false},
		{"o / m /../ g", false, false},

		// invalid systemd paths
		{"o / m /../ g", true, true},
		{"slice:kata", true, true},
		{"/kata/afhts2e5d4g5s", true, true},
		{"a:b:c:d", true, true},
		{":::", true, true},
		{"", true, true},
		{":", true, true},
		{"::", true, true},
		{":::", true, true},
		{"a:b", true, true},
		{"a:b:", true, true},
		{":a:b", true, true},
		{"@:@:@", true, true},

		// valid systemd paths
		{"x.slice:kata:55555", true, false},
		{"system.slice:kata:afhts2e5d4g5s", true, false},
	} {
		path, err := ValidCgroupPath(t.path, t.systemdCgroup)
		if t.error {
			assert.Error(err)
			continue
		} else {
			assert.NoError(err)
		}

		if filepath.IsAbs(t.path) {
			cleanPath := filepath.Dir(filepath.Clean(t.path))
			assert.True(strings.HasPrefix(path, cleanPath),
				"%v should have prefix %v", cleanPath)
		} else if t.systemdCgroup {
			assert.Equal(t.path, path)
		} else {
			assert.True(strings.HasPrefix(path, "/"+CgroupKataPrefix) ||
				strings.HasPrefix(path, DefaultCgroupPath),
				"%v should have prefix /%v or %v", path, CgroupKataPrefix, DefaultCgroupPath)
		}
	}

}

func TestDeviceToCgroupDevice(t *testing.T) {
	assert := assert.New(t)

	f, err := ioutil.TempFile("", "device")
	assert.NoError(err)
	f.Close()

	// fail: regular file to device
	dev, err := DeviceToCgroupDevice(f.Name())
	assert.Error(err)
	assert.Nil(dev)

	// fail: no such file
	os.Remove(f.Name())
	dev, err = DeviceToCgroupDevice(f.Name())
	assert.Error(err)
	assert.Nil(dev)

	devPath := "/dev/null"
	if _, err := os.Stat(devPath); os.IsNotExist(err) {
		t.Skipf("no such device: %v", devPath)
		return
	}
	dev, err = DeviceToCgroupDevice(devPath)
	assert.NoError(err)
	assert.NotNil(dev)
	assert.Equal(dev.Type, 'c')
	assert.Equal(dev.Path, devPath)
	assert.NotZero(dev.Major)
	assert.NotZero(dev.Minor)
	assert.NotEmpty(dev.Permissions)
	assert.NotZero(dev.FileMode)
	assert.Zero(dev.Uid)
	assert.Zero(dev.Gid)
	assert.True(dev.Allow)
}

func TestDeviceToLinuxDevice(t *testing.T) {
	assert := assert.New(t)

	devPath := "/dev/null"
	if _, err := os.Stat(devPath); os.IsNotExist(err) {
		t.Skipf("no such device: %v", devPath)
		return
	}
	dev, err := DeviceToLinuxDevice(devPath)
	assert.NoError(err)
	assert.NotNil(dev)
	assert.Equal(dev.Type, "c")
	assert.NotNil(dev.Major)
	assert.NotZero(*dev.Major)
	assert.NotNil(dev.Minor)
	assert.NotZero(*dev.Minor)
	assert.NotEmpty(dev.Access)
	assert.True(dev.Allow)
}
