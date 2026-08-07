// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package rootless

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/moby/sys/userns"
	"github.com/stretchr/testify/assert"
)

func TestIsRootless(t *testing.T) {
	assert := assert.New(t)
	isRootless = nil

	var rootless bool
	if os.Getuid() != 0 {
		rootless = true
	} else {
		rootless = userns.RunningInUserNS()
	}

	assert.Equal(rootless, isRootlessFunc())

	SetRootless(true)
	assert.True(isRootlessFunc())

	SetRootless(false)
	assert.False(isRootlessFunc())

	isRootless = nil
}

func TestVmmUserRuntimeDir(t *testing.T) {
	assert.Equal(t, filepath.Join(vmmUserRuntimeBaseDir, "1000"), VmmUserRuntimeDir(1000))
}
