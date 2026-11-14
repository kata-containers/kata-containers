// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestCapacitySet(t *testing.T) {
	testCases := []struct {
		name     string
		args     []string
		expected map[string]int64
		errMsg   string
	}{
		{
			name:     "single pool",
			args:     []string{"fast=10Gi"},
			expected: map[string]int64{"fast": 10 * GiB},
		},
		{
			name:     "several pools",
			args:     []string{"fast=1Gi", "slow=2Ti"},
			expected: map[string]int64{"fast": GiB, "slow": 2 * TiB},
		},
		{
			name:     "later value wins",
			args:     []string{"fast=1Gi", "fast=3Gi"},
			expected: map[string]int64{"fast": 3 * GiB},
		},
		{
			name:     "zero size",
			args:     []string{"empty=0"},
			expected: map[string]int64{"empty": 0},
		},
		{
			name:     "decimal suffix",
			args:     []string{"fast=1G"},
			expected: map[string]int64{"fast": 1000 * 1000 * 1000},
		},
		{
			name:   "missing separator",
			args:   []string{"fast"},
			errMsg: "must be of format <pool>=<size>",
		},
		{
			name:   "empty pool name",
			args:   []string{"=1Gi"},
			errMsg: "must not be empty",
		},
		{
			name:   "invalid pool name",
			args:   []string{"fast pool=1Gi"},
			errMsg: "invalid capacity pool name",
		},
		{
			name:   "pool name too long",
			args:   []string{strings.Repeat("a", 64) + "=1Gi"},
			errMsg: "invalid capacity pool name",
		},
		{
			name:   "invalid size",
			args:   []string{"fast=lots"},
			errMsg: "invalid size",
		},
		{
			name:   "negative size",
			args:   []string{"fast=-1Gi"},
			errMsg: "must not be negative",
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			var capacity Capacity
			var err error
			for _, arg := range tc.args {
				if err = capacity.Set(arg); err != nil {
					break
				}
			}

			if tc.errMsg != "" {
				require.ErrorContains(t, err, tc.errMsg)
				return
			}
			require.NoError(t, err)
			require.True(t, capacity.Enabled())
			require.Len(t, capacity, len(tc.expected))
			for pool, size := range tc.expected {
				quantity, ok := capacity[pool]
				require.True(t, ok, "pool %q", pool)
				require.Equal(t, size, quantity.Value(), "pool %q", pool)
			}
		})
	}
}

func TestCapacityString(t *testing.T) {
	var capacity Capacity
	require.False(t, capacity.Enabled())
	require.Empty(t, capacity.String())

	require.NoError(t, capacity.Set("slow=2Gi"))
	require.NoError(t, capacity.Set("fast=1Gi"))
	require.Equal(t, "fast=1Gi,slow=2Gi", capacity.String())
}

func TestCapacityMerge(t *testing.T) {
	base, err := ParseCapacityConfig("fast=1Gi,slow=2Gi")
	require.NoError(t, err)
	override, err := ParseCapacityConfig("slow=8Gi\nlocal=4Gi")
	require.NoError(t, err)

	merged := base.Merge(override)
	require.Equal(t, "fast=1Gi,local=4Gi,slow=8Gi", merged.String())
	// Merge must not modify its inputs.
	require.Equal(t, "fast=1Gi,slow=2Gi", base.String())

	var empty Capacity
	require.Equal(t, "local=4Gi,slow=8Gi", empty.Merge(override).String())
}

func TestParseCapacityConfig(t *testing.T) {
	testCases := []struct {
		name     string
		content  string
		expected string
		errMsg   string
	}{
		{name: "empty", content: "", expected: ""},
		{name: "one entry per line", content: "fast=2Gi\nslow=8Gi\n", expected: "fast=2Gi,slow=8Gi"},
		{name: "comma separated", content: "fast=2Gi, slow=8Gi", expected: "fast=2Gi,slow=8Gi"},
		{name: "comments and blank lines", content: "# node-a\n\n  fast=2Gi  \n# end\n", expected: "fast=2Gi"},
		{name: "CRLF line endings", content: "fast=2Gi\r\nslow=8Gi\r\n", expected: "fast=2Gi,slow=8Gi"},
		{name: "duplicate pool", content: "fast=2Gi\nfast=4Gi", errMsg: "configured more than once"},
		{name: "malformed entry", content: "fast:2Gi", errMsg: "must be of format"},
		{name: "negative size", content: "fast=-2Gi", errMsg: "must not be negative"},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			capacity, err := ParseCapacityConfig(tc.content)
			if tc.errMsg != "" {
				require.ErrorContains(t, err, tc.errMsg)
				return
			}
			require.NoError(t, err)
			require.Equal(t, tc.expected, capacity.String())
		})
	}
}

func TestLoadNodeCapacity(t *testing.T) {
	dir := t.TempDir()
	require.NoError(t, os.WriteFile(filepath.Join(dir, "node-a"), []byte("default=2Gi\n"), 0600))
	require.NoError(t, os.WriteFile(filepath.Join(dir, "node-b"), []byte("default=8Gi,fast=1Gi"), 0600))
	require.NoError(t, os.WriteFile(filepath.Join(dir, "node-bad"), []byte("default=much"), 0600))

	testCases := []struct {
		name     string
		dir      string
		nodeID   string
		found    bool
		expected string
		errMsg   string
	}{
		{name: "node-a", dir: dir, nodeID: "node-a", found: true, expected: "default=2Gi"},
		{name: "node-b", dir: dir, nodeID: "node-b", found: true, expected: "default=8Gi,fast=1Gi"},
		{name: "node without file", dir: dir, nodeID: "node-c"},
		{name: "missing directory", dir: filepath.Join(dir, "missing"), nodeID: "node-a"},
		{name: "invalid file", dir: dir, nodeID: "node-bad", errMsg: "invalid size"},
		{name: "path in node id", dir: dir, nodeID: "../node-a", errMsg: "cannot be used"},
		{name: "hidden node id", dir: dir, nodeID: "..data", errMsg: "cannot be used"},
		{name: "empty node id", dir: dir, nodeID: "", errMsg: "cannot be used"},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			capacity, found, err := LoadNodeCapacity(tc.dir, tc.nodeID)
			if tc.errMsg != "" {
				require.ErrorContains(t, err, tc.errMsg)
				return
			}
			require.NoError(t, err)
			require.Equal(t, tc.found, found)
			require.Equal(t, tc.expected, capacity.String())
		})
	}
}

func TestDirectBlockDevicePath(t *testing.T) {
	require.Equal(t, "/pool/vol-1/directvol-rawdisk.2048M", DirectBlockDevicePath("/pool", "vol-1", 2*GiB))
	// The name is rounded down to whole MiB like the size given to fallocate.
	require.Equal(t, "/pool/vol-1/directvol-rawdisk.953M", DirectBlockDevicePath("/pool", "vol-1", 1000*1000*1000))
}
