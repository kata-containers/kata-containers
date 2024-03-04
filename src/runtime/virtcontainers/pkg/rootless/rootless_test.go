// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package rootless

import (
	"os"
	"testing"

	"github.com/opencontainers/runc/libcontainer/userns"
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

func TestNewNS(t *testing.T) {
	tmpdir := t.TempDir()

	tcs := []struct {
		Name    string
		Dir     string
		Message string
		user    string
	}{
		{
			Name:    "No fails with root",
			Dir:     tmpdir,
			Message: "",
			user:    "root",
		},
		{
			Name:    "cannot create ns without root",
			Dir:     tmpdir,
			Message: "failed to create namespace: no root permission",
			user:    "user",
		},
	}

	for _, ts := range tcs {
		rootlessDir = ts.Dir
		_, err := NewNS()
		if os.Getuid() != 0 && ts.user == "root" {
			continue
		}
		if os.Getuid() == 0 && ts.user == "user" {
			continue
		}
		if err != nil && err.Error() != ts.Message {
			t.Errorf("test %v, want %v, got %v", ts.Name, ts.Message, err.Error())
		}
	}

}
