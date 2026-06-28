// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNewVM(t *testing.T) {
	assert := assert.New(t)

	testDir := t.TempDir()

	config := VMConfig{
		HypervisorType: MockHypervisor,
	}
	hyperConfig := HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}

	ctx := WithNewAgentFunc(context.Background(), newMockAgent)

	var vm *VM
	_, err := NewVM(ctx, config)
	assert.Error(err)

	config.HypervisorConfig = hyperConfig
	vm, err = NewVM(ctx, config)
	assert.Nil(err)

	// VM operations
	err = vm.Pause(context.Background())
	assert.Nil(err)
	err = vm.Resume(context.Background())
	assert.Nil(err)
	err = vm.Start(context.Background())
	assert.Nil(err)
	err = vm.Disconnect(context.Background())
	assert.Nil(err)
	err = vm.Save()
	assert.Nil(err)
	err = vm.Stop(context.Background())
	assert.Nil(err)
	err = vm.AddCPUs(context.Background(), 2)
	assert.Nil(err)
	err = vm.AddMemory(context.Background(), 128)
	assert.Nil(err)
	err = vm.OnlineCPUMemory(context.Background())
	assert.Nil(err)

	// mock urandom device
	savedUrandomDev := urandomDev
	defer func() {
		urandomDev = savedUrandomDev
	}()
	tmpdir := t.TempDir()
	urandomDev = filepath.Join(tmpdir, "urandom")
	data := make([]byte, 512)
	err = os.WriteFile(urandomDev, data, os.FileMode(0640))
	assert.NoError(err)

	err = vm.ReseedRNG(context.Background())
	assert.Nil(err)

	// template VM
	config.HypervisorConfig.BootFromTemplate = true
	_, err = NewVM(ctx, config)
	assert.Error(err)

	config.HypervisorConfig.MemoryPath = testDir
	_, err = NewVM(ctx, config)
	assert.Error(err)

	config.HypervisorConfig.DevicesStatePath = testDir
	_, err = NewVM(ctx, config)
	assert.Nil(err)
}

type checkAgentReadyTestAgent struct {
	mockAgent
	checkErr      error
	checked       bool
	disconnects   int
	disconnectErr error
}

func (a *checkAgentReadyTestAgent) check(ctx context.Context) error {
	a.checked = true
	return a.checkErr
}

func (a *checkAgentReadyTestAgent) disconnect(ctx context.Context) error {
	a.disconnects++
	return a.disconnectErr
}

func TestCheckAgentReadyDisconnectIsBestEffort(t *testing.T) {
	assert := assert.New(t)

	agent := &checkAgentReadyTestAgent{
		disconnectErr: context.Canceled,
	}
	vm := &VM{
		agent: agent,
	}

	err := vm.CheckAgentReady(context.Background())
	assert.NoError(err)
	assert.True(agent.checked)
	assert.Equal(1, agent.disconnects)
}

func TestCheckAgentReadyReturnsCheckError(t *testing.T) {
	assert := assert.New(t)

	agent := &checkAgentReadyTestAgent{
		checkErr: context.Canceled,
	}
	vm := &VM{
		agent: agent,
	}

	err := vm.CheckAgentReady(context.Background())
	assert.ErrorIs(err, context.Canceled)
	assert.Equal(1, agent.disconnects)
}
