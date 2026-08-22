// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

package fs

import (
	"os"
	"path/filepath"
	"syscall"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestRootlessInitPreparesStorageRoot(t *testing.T) {
	rootlessDir := t.TempDir()
	t.Setenv("XDG_RUNTIME_DIR", rootlessDir)

	driver, err := RootlessInit()
	require.NoError(t, err)
	_, err = RootlessInit()
	require.NoError(t, err, "rootless storage initialization must be idempotent")

	rootlessFS, ok := driver.(*RootlessFS)
	require.True(t, ok)
	expectedStorageRoot := filepath.Join(rootlessDir, "run", StoragePathSuffix)
	assert.Equal(t, expectedStorageRoot, rootlessFS.storageRootPath)

	rootInfo, err := os.Stat(rootlessDir)
	require.NoError(t, err)
	rootStat, ok := rootInfo.Sys().(*syscall.Stat_t)
	require.True(t, ok)

	for _, path := range []string{filepath.Join(rootlessDir, "run"), expectedStorageRoot} {
		info, err := os.Stat(path)
		require.NoError(t, err)
		stat, ok := info.Sys().(*syscall.Stat_t)
		require.True(t, ok)
		assert.Equal(t, rootStat.Uid, stat.Uid)
		assert.Equal(t, rootStat.Gid, stat.Gid)
	}
}
