// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package cgroups

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

//very very basic test; should be expanded
func TestNew(t *testing.T) {
	assert := assert.New(t)

	// create a cgroupfs cgroup manager
	c := &Config{
		Cgroups:    nil,
		CgroupPath: "",
	}

	mgr, err := New(c)
	assert.NoError(err)
	assert.NotNil(mgr.mgr)

	// create a systemd cgroup manager
	s := &Config{
		Cgroups:    nil,
		CgroupPath: "system.slice:kubepod:container",
	}

	mgr, err = New(s)
	assert.NoError(err)
	assert.NotNil(mgr.mgr)

}
