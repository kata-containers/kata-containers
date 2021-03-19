// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"io/ioutil"
	"os"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/stretchr/testify/assert"
)

func TestNewVM(t *testing.T) {
	assert := assert.New(t)

	testDir, err := ioutil.TempDir("", "vmfactory-tmp-")
	assert.Nil(err)
	defer os.RemoveAll(testDir)

	config := VMConfig{
		HypervisorType: MockHypervisor,
	}
	hyperConfig := HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}

	ctx := WithNewAgentFunc(context.Background(), newMockAgent)

	var vm *VM
	_, err = NewVM(ctx, config)
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

func TestVMConfigValid(t *testing.T) {
	assert := assert.New(t)

	config := VMConfig{}

	err := config.Valid()
	assert.Error(err)

	testDir, err := ioutil.TempDir("", "vmfactory-tmp-")
	assert.Nil(err)
	defer os.RemoveAll(testDir)

	config.HypervisorConfig = HypervisorConfig{
		KernelPath: testDir,
		InitrdPath: testDir,
	}
	err = config.Valid()
	assert.Nil(err)
}

func TestVMConfigGrpc(t *testing.T) {
	assert := assert.New(t)
	config := VMConfig{
		HypervisorType:   QemuHypervisor,
		HypervisorConfig: newQemuConfig(),
		AgentConfig: KataAgentConfig{
			LongLiveConn:       true,
			Debug:              false,
			Trace:              false,
			EnableDebugConsole: false,
			ContainerPipeSize:  0,
			TraceMode:          "",
			TraceType:          "",
			KernelModules:      []string{}},
	}

	p, err := config.ToGrpc()
	assert.Nil(err)

	config2, err := GrpcToVMConfig(p)
	assert.Nil(err)

	assert.True(utils.DeepCompare(config, *config2))
}
