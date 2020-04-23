// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package cgroups

import (
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
		{"slice:kata:afhts2e5d4g5s", true},
		{"slice.system:kata:afhts2e5d4g5s", true},
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

		// valid system paths
		{"slice:kata:55555", true, false},
		{"slice.system:kata:afhts2e5d4g5s", true, false},
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
