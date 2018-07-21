// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package factory

import (
	"io/ioutil"
	"testing"

	"github.com/stretchr/testify/assert"

	vc "github.com/kata-containers/runtime/virtcontainers"
)

func TestNewFactory(t *testing.T) {
	var config Config

	assert := assert.New(t)

	_, err := NewFactory(config, true)
	assert.Error(err)
	_, err = NewFactory(config, false)
	assert.Error(err)

	config.VMConfig = vc.VMConfig{
		HypervisorType: vc.MockHypervisor,
		AgentType:      vc.NoopAgentType,
	}

	_, err = NewFactory(config, false)
	assert.Error(err)

	testDir, _ := ioutil.TempDir("", "vmfactory-tmp-")

	config.VMConfig.HypervisorConfig = vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}

	// direct
	_, err = NewFactory(config, false)
	assert.Nil(err)
	_, err = NewFactory(config, true)
	assert.Nil(err)

	// template
	config.Template = true
	_, err = NewFactory(config, false)
	assert.Nil(err)
	_, err = NewFactory(config, true)
	assert.Error(err)

	// Cache
	config.Cache = 10
	_, err = NewFactory(config, false)
	assert.Nil(err)
	_, err = NewFactory(config, true)
	assert.Error(err)

	config.Template = false
	_, err = NewFactory(config, false)
	assert.Nil(err)
	_, err = NewFactory(config, true)
	assert.Error(err)
}

func TestVMConfigValid(t *testing.T) {
	assert := assert.New(t)

	config := Config{}

	err := config.validate()
	assert.Error(err)

	testDir, _ := ioutil.TempDir("", "vmfactory-tmp-")

	config.VMConfig = vc.VMConfig{
		HypervisorType: vc.MockHypervisor,
		AgentType:      vc.NoopAgentType,
		HypervisorConfig: vc.HypervisorConfig{
			KernelPath: testDir,
			ImagePath:  testDir,
		},
	}

	err = config.validate()
	assert.Nil(err)
}

func TestCheckVMConfig(t *testing.T) {
	assert := assert.New(t)

	var config1, config2 vc.VMConfig

	// default config should equal
	err := checkVMConfig(config1, config2)
	assert.Nil(err)

	config1.HypervisorType = vc.MockHypervisor
	err = checkVMConfig(config1, config2)
	assert.Error(err)

	config2.HypervisorType = vc.MockHypervisor
	err = checkVMConfig(config1, config2)
	assert.Nil(err)

	config1.AgentType = vc.NoopAgentType
	err = checkVMConfig(config1, config2)
	assert.Error(err)

	config2.AgentType = vc.NoopAgentType
	err = checkVMConfig(config1, config2)
	assert.Nil(err)

	testDir, _ := ioutil.TempDir("", "vmfactory-tmp-")
	config1.HypervisorConfig = vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}
	err = checkVMConfig(config1, config2)
	assert.Error(err)

	config2.HypervisorConfig = vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}
	err = checkVMConfig(config1, config2)
	assert.Nil(err)
}

func TestFactoryGetVM(t *testing.T) {
	assert := assert.New(t)

	testDir, _ := ioutil.TempDir("", "vmfactory-tmp-")
	hyperConfig := vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}
	vmConfig := vc.VMConfig{
		HypervisorType:   vc.MockHypervisor,
		AgentType:        vc.NoopAgentType,
		HypervisorConfig: hyperConfig,
	}

	// direct factory
	f, err := NewFactory(Config{VMConfig: vmConfig}, false)
	assert.Nil(err)

	_, err = f.GetVM(vmConfig)
	assert.Nil(err)

	f.CloseFactory()

	// template factory
	f, err = NewFactory(Config{Template: true, VMConfig: vmConfig}, false)
	assert.Nil(err)

	_, err = f.GetVM(vmConfig)
	assert.Nil(err)

	f.CloseFactory()

	// fetch template factory
	f, err = NewFactory(Config{Template: true, VMConfig: vmConfig}, false)
	assert.Nil(err)

	_, err = NewFactory(Config{Template: true, VMConfig: vmConfig}, true)
	assert.Error(err)

	_, err = f.GetVM(vmConfig)
	assert.Nil(err)

	f.CloseFactory()

	// cache factory over direct factory
	f, err = NewFactory(Config{Cache: 2, VMConfig: vmConfig}, false)
	assert.Nil(err)

	_, err = f.GetVM(vmConfig)
	assert.Nil(err)

	f.CloseFactory()

	// cache factory over template factory
	f, err = NewFactory(Config{Template: true, Cache: 2, VMConfig: vmConfig}, false)
	assert.Nil(err)

	_, err = f.GetVM(vmConfig)
	assert.Nil(err)

	// CPU hotplug
	vmConfig.HypervisorConfig.DefaultVCPUs++
	_, err = f.GetVM(vmConfig)
	assert.Nil(err)

	// Memory hotplug
	vmConfig.HypervisorConfig.DefaultMemSz += 128
	_, err = f.GetVM(vmConfig)
	assert.Nil(err)

	// checkConfig fall back
	vmConfig.HypervisorConfig.Mlock = true
	_, err = f.GetVM(vmConfig)
	assert.Nil(err)

	f.CloseFactory()
}
