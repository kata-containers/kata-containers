// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package cgroups

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestEnableSystemdCgroup(t *testing.T) {
	assert := assert.New(t)

	orgSystemdCgroup := systemdCgroup
	defer func() {
		systemdCgroup = orgSystemdCgroup
	}()

	useSystemdCgroup := UseSystemdCgroup()
	if systemdCgroup != nil {
		assert.Equal(*systemdCgroup, useSystemdCgroup)
	} else {
		assert.False(useSystemdCgroup)
	}

	EnableSystemdCgroup()
	assert.True(UseSystemdCgroup())
}

func TestNew(t *testing.T) {
	assert := assert.New(t)
	useSystemdCgroup := false
	orgSystemdCgroup := systemdCgroup
	defer func() {
		systemdCgroup = orgSystemdCgroup
	}()
	systemdCgroup = &useSystemdCgroup

	c := &Config{
		Cgroups:    nil,
		CgroupPath: "",
	}

	mgr, err := New(c)
	assert.NoError(err)
	assert.NotNil(mgr.mgr)

	useSystemdCgroup = true
	mgr, err = New(c)
	assert.Error(err)
	assert.Nil(mgr)
}
