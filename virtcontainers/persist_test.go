// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/runtime/virtcontainers/device/manager"
	exp "github.com/kata-containers/runtime/virtcontainers/experimental"
	"github.com/kata-containers/runtime/virtcontainers/persist"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

func TestSandboxRestore(t *testing.T) {
	var err error
	assert := assert.New(t)
	sconfig := SandboxConfig{
		ID:           "test-exp",
		Experimental: []exp.Feature{persist.NewStoreFeature},
	}
	container := make(map[string]*Container)
	container["test-exp"] = &Container{}

	sandbox := Sandbox{
		id:         "test-exp",
		containers: container,
		devManager: manager.NewDeviceManager(manager.VirtioSCSI, nil),
		hypervisor: &mockHypervisor{},
		ctx:        context.Background(),
		config:     &sconfig,
	}

	sandbox.newStore, err = persist.GetDriver("fs")
	assert.NoError(err)
	assert.NotNil(sandbox.newStore)

	// if we don't call Save(), we can get nothing from disk
	err = sandbox.Restore()
	assert.NotNil(t, err)
	assert.True(os.IsNotExist(err))

	// disk data are empty
	err = sandbox.Save()
	assert.NoError(err)

	err = sandbox.Restore()
	assert.NoError(err)
	assert.Equal(sandbox.state.State, types.StateString(""))
	assert.Equal(sandbox.state.GuestMemoryBlockSizeMB, uint32(0))
	assert.Equal(sandbox.state.BlockIndex, 0)

	// set state data and save again
	sandbox.state.State = types.StateString("running")
	sandbox.state.GuestMemoryBlockSizeMB = uint32(1024)
	sandbox.state.BlockIndex = 2
	// flush data to disk
	err = sandbox.Save()
	assert.Nil(err)

	// empty the sandbox
	sandbox.state = types.SandboxState{}

	// restore data from disk
	err = sandbox.Restore()
	assert.Nil(err)
	assert.Equal(sandbox.state.State, types.StateString("running"))
	assert.Equal(sandbox.state.GuestMemoryBlockSizeMB, uint32(1024))
	assert.Equal(sandbox.state.BlockIndex, 2)
}
