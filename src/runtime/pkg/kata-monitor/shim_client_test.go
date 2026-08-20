// Copyright (c) 2026 Kata Containers Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
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
