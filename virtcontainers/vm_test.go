// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"io/ioutil"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNewVM(t *testing.T) {
	assert := assert.New(t)

	testDir, _ := ioutil.TempDir("", "vmfactory-tmp-")
	config := VMConfig{
		HypervisorType: MockHypervisor,
		AgentType:      NoopAgentType,
	}
	hyperConfig := HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}

	ctx := context.Background()

	var vm *VM
	_, err := NewVM(ctx, config)
	assert.Error(err)

	config.HypervisorConfig = hyperConfig
	vm, err = NewVM(ctx, config)
	assert.Nil(err)

	// VM operations
	err = vm.Pause()
	assert.Nil(err)
	err = vm.Resume()
	assert.Nil(err)
	err = vm.Start()
	assert.Nil(err)
	err = vm.Save()
	assert.Nil(err)
	err = vm.Stop()
	assert.Nil(err)
	err = vm.AddCPUs(2)
	assert.Nil(err)
	err = vm.AddMemory(128)
	assert.Nil(err)
	err = vm.OnlineCPUMemory()
	assert.Nil(err)
	err = vm.ReseedRNG()
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

func TestVMConfigValid(t *testing.T) {
	assert := assert.New(t)

	config := VMConfig{}

	err := config.Valid()
	assert.Error(err)

	testDir, _ := ioutil.TempDir("", "vmfactory-tmp-")
	config.HypervisorConfig = HypervisorConfig{
		KernelPath: testDir,
		InitrdPath: testDir,
	}
	err = config.Valid()
	assert.Nil(err)
}
