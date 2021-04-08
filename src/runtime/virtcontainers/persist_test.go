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

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/manager"
	exp "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/experimental"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
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
		devManager: manager.NewDeviceManager(manager.VirtioSCSI, false, "", nil),
		hypervisor: &mockHypervisor{},
		ctx:        context.Background(),
		config:     &sconfig,
		state:      types.SandboxState{BlockIndexMap: make(map[int]struct{})},
	}

	sandbox.store, err = persist.GetDriver()
	assert.NoError(err)
	assert.NotNil(sandbox.store)

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
	assert.Equal(len(sandbox.state.BlockIndexMap), 0)

	// set state data and save again
	sandbox.state.State = types.StateString("running")
	sandbox.state.GuestMemoryBlockSizeMB = uint32(1024)
	sandbox.state.BlockIndexMap[2] = struct{}{}
	// flush data to disk
	err = sandbox.Save()
	assert.Nil(err)

	// empty the sandbox
	sandbox.state = types.SandboxState{}
	if sandbox.store, err = persist.GetDriver(); err != nil || sandbox.store == nil {
		t.Fatal("failed to get persist driver")
	}

	// restore data from disk
	err = sandbox.Restore()
	assert.NoError(err)
	assert.Equal(sandbox.state.State, types.StateString("running"))
	assert.Equal(sandbox.state.GuestMemoryBlockSizeMB, uint32(1024))
	assert.Equal(len(sandbox.state.BlockIndexMap), 1)
	assert.Equal(sandbox.state.BlockIndexMap[2], struct{}{})
}
