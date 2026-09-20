// Copyright (c) 2026 Kata Containers Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"os"
	"path/filepath"
	"testing"

	shim "github.com/kata-containers/kata-containers/src/runtime/pkg/containerd-shim-v2"
	"github.com/stretchr/testify/assert"
)

func TestGetSandboxFSPaths(t *testing.T) {
	assert := assert.New(t)

	paths := getSandboxFSPaths()
	assert.Equal([]string{
		shim.GetSandboxesStoragePath(),
		shim.GetSandboxesStoragePathRust(),
	}, paths)
	assert.Contains(paths, "/run/vc/sbs")
	assert.Contains(paths, "/run/kata")
}

func TestSandboxRemoved(t *testing.T) {
	assert := assert.New(t)

	goPath := t.TempDir()
	rustPath := t.TempDir()
	paths := []string{goPath, rustPath}

	assert.True(sandboxRemoved(paths, "gone"), "absent from both paths")

	assert.NoError(os.Mkdir(filepath.Join(goPath, "go-sandbox"), 0o755))
	assert.False(sandboxRemoved(paths, "go-sandbox"), "present in the Go runtime path")

	assert.NoError(os.Mkdir(filepath.Join(rustPath, "rust-sandbox"), 0o755))
	assert.False(sandboxRemoved(paths, "rust-sandbox"), "present in the runtime-rs path")

	// A storage path that is a file makes the stat fail with ENOTDIR, not
	// ENOENT: the check is inconclusive, so the sandbox counts as present.
	notADir := filepath.Join(t.TempDir(), "file")
	assert.NoError(os.WriteFile(notADir, nil, 0o644))
	assert.False(sandboxRemoved([]string{notADir, rustPath}, "gone"), "inconclusive path check")
}
