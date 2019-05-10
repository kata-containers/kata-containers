// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/runtime/virtcontainers/device/manager"
	exp "github.com/kata-containers/runtime/virtcontainers/experimental"
	"github.com/kata-containers/runtime/virtcontainers/persist"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

func testCreateExpSandbox() (*Sandbox, error) {
	sconfig := SandboxConfig{
		ID:               "test-exp",
		HypervisorType:   MockHypervisor,
		HypervisorConfig: newHypervisorConfig(nil, nil),
		AgentType:        NoopAgentType,
		NetworkConfig:    NetworkConfig{},
		Volumes:          nil,
		Containers:       nil,
		Experimental:     []exp.Feature{persist.NewStoreFeature},
	}

	// support experimental
	sandbox, err := createSandbox(context.Background(), sconfig, nil)
	if err != nil {
		return nil, fmt.Errorf("Could not create sandbox: %s", err)
	}

	if err := sandbox.agent.startSandbox(sandbox); err != nil {
		return nil, err
	}

	return sandbox, nil
}

func TestSupportNewStore(t *testing.T) {
	hConfig := newHypervisorConfig(nil, nil)
	sandbox, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NoopAgentType, NetworkConfig{}, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	// not support experimental
	assert.False(t, sandbox.supportNewStore())

	// support experimental
	sandbox, err = testCreateExpSandbox()
	if err != nil {
		t.Fatal(err)
	}
	assert.True(t, sandbox.supportNewStore())
}

func TestSandboxRestore(t *testing.T) {
	var err error
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

	if sandbox.newStore, err = persist.GetDriver("fs"); err != nil || sandbox.newStore == nil {
		t.Fatalf("failed to get fs persist driver")
	}

	// if we don't call ToDisk, we can get nothing from disk
	err = sandbox.Restore()
	assert.NotNil(t, err)
	assert.True(t, os.IsNotExist(err))

	// disk data are empty
	err = sandbox.Save()
	assert.Nil(t, err)

	err = sandbox.Restore()
	assert.Nil(t, err)
	assert.Equal(t, sandbox.state.State, types.StateString(""))
	assert.Equal(t, sandbox.state.GuestMemoryBlockSizeMB, uint32(0))
	assert.Equal(t, sandbox.state.BlockIndex, 0)

	// set state data and save again
	sandbox.state.State = types.StateString("running")
	sandbox.state.GuestMemoryBlockSizeMB = uint32(1024)
	sandbox.state.BlockIndex = 2
	// flush data to disk
	err = sandbox.Save()
	assert.Nil(t, err)

	// empty the sandbox
	sandbox.state = types.SandboxState{}

	// restore data from disk
	err = sandbox.Restore()
	assert.Nil(t, err)
	assert.Equal(t, sandbox.state.State, types.StateString("running"))
	assert.Equal(t, sandbox.state.GuestMemoryBlockSizeMB, uint32(1024))
	assert.Equal(t, sandbox.state.BlockIndex, 2)
}
