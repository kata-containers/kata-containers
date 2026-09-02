//go:build linux

// Copyright (c) 2026 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"path/filepath"
	"testing"

	cgroupsv2 "github.com/containerd/cgroups/v2"
	"github.com/stretchr/testify/assert"
)

func TestLinuxCgroupParent(t *testing.T) {
	assert := assert.New(t)

	tests := []struct {
		name     string
		cgroup   interface{}
		path     string
		expected string
	}{
		{
			// For cgroupsv2 the actual parent (e.g. /kubepods/besteffort/podXXX)
			// is a non-leaf cgroup and cannot accept processes (EBUSY). The root
			// cgroup "/" is the only universally-writable destination, so
			// Parent() always returns "/" for cgroupsv2 regardless of depth.
			name:     "cgroupsv2 deep path returns root",
			cgroup:   (*cgroupsv2.Manager)(nil),
			path:     "/kubepods/besteffort/podxxx/kata_abc123",
			expected: "/",
		},
		{
			name:     "cgroupsv2 shallow path returns root",
			cgroup:   (*cgroupsv2.Manager)(nil),
			path:     "/kata_abc123",
			expected: "/",
		},
		{
			// cgroupsv1 uses a hierarchy of writable controllers; returning
			// the real parent directory is correct.
			name:     "cgroupsv1 deep path returns parent dir",
			cgroup:   nil,
			path:     "/kubepods/besteffort/podxxx/kata_abc123",
			expected: filepath.Dir("/kubepods/besteffort/podxxx/kata_abc123"),
		},
		{
			name:     "cgroupsv1 shallow path returns parent dir",
			cgroup:   nil,
			path:     "/kata_abc123",
			expected: filepath.Dir("/kata_abc123"),
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			lc := &LinuxCgroup{
				cgroup: tt.cgroup,
				path:   tt.path,
			}
			assert.Equal(tt.expected, lc.Parent(), "path=%q", tt.path)
		})
	}
}
