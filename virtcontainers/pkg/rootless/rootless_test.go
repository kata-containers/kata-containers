// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package rootless

import (
	"os"
	"testing"

	"github.com/opencontainers/runc/libcontainer/system"
	"github.com/stretchr/testify/assert"
)

func TestIsRootless(t *testing.T) {
	assert := assert.New(t)
	isRootless = nil

	var rootless bool
	if os.Getuid() != 0 {
		rootless = true
	} else {
		rootless = system.RunningInUserNS()
	}

	assert.Equal(rootless, isRootlessFunc())

	SetRootless(true)
	assert.True(isRootlessFunc())

	SetRootless(false)
	assert.False(isRootlessFunc())

	isRootless = nil
}
