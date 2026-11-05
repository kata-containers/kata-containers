// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"os/exec"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
	"golang.org/x/sys/unix"
)

// TestFormatKeepsAllocation checks that formatting a raw disk keeps the
// blocks which fallocate allocated for it.
func TestFormatKeepsAllocation(t *testing.T) {
	for _, tool := range []string{"fallocate", "mkfs.ext4", "blkid"} {
		if _, err := exec.LookPath(tool); err != nil {
			t.Skipf("%s is not installed", tool)
		}
	}

	disk := filepath.Join(t.TempDir(), "directvol-rawdisk.64M")
	if out, err := exec.Command("fallocate", "-l", "64M", disk).CombinedOutput(); err != nil {
		t.Skipf("fallocate is not supported here: %v: %s", err, out)
	}

	formater := NewSafeMountFormater()
	require.NoError(t, formater.SafeFormatWithFstype(disk, "ext4", nil))

	format, err := formater.GetDiskFormat(disk)
	require.NoError(t, err)
	require.Equal(t, "ext4", format)

	var st unix.Stat_t
	require.NoError(t, unix.Stat(disk, &st))
	require.GreaterOrEqual(t, int64(st.Blocks)*512, 64*MiB, "mkfs must not discard the allocated blocks")
}
