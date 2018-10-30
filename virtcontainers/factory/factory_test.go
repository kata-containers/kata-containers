// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package factory

import (
	"context"
	"io/ioutil"
	"testing"

	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/factory/base"
)

func TestNewFactory(t *testing.T) {
	var config Config

	assert := assert.New(t)

	ctx := context.Background()
	_, err := NewFactory(ctx, config, true)
	assert.Error(err)
	_, err = NewFactory(ctx, config, false)
	assert.Error(err)

	config.VMConfig = vc.VMConfig{
		HypervisorType: vc.MockHypervisor,
		AgentType:      vc.NoopAgentType,
		ProxyType:      vc.NoopProxyType,
	}

	_, err = NewFactory(ctx, config, false)
	assert.Error(err)

	testDir, _ := ioutil.TempDir("", "vmfactory-tmp-")

	config.VMConfig.HypervisorConfig = vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}

	// direct
	f, err := NewFactory(ctx, config, false)
	assert.Nil(err)
	f.CloseFactory(ctx)
	f, err = NewFactory(ctx, config, true)
	assert.Nil(err)
	f.CloseFactory(ctx)

	// template
	config.Template = true
	f, err = NewFactory(ctx, config, false)
	assert.Nil(err)
	f.CloseFactory(ctx)
	_, err = NewFactory(ctx, config, true)
	assert.Error(err)

	// Cache
	config.Cache = 10
	f, err = NewFactory(ctx, config, false)
	assert.Nil(err)
	f.CloseFactory(ctx)
	_, err = NewFactory(ctx, config, true)
	assert.Error(err)

	config.Template = false
	f, err = NewFactory(ctx, config, false)
	assert.Nil(err)
	f.CloseFactory(ctx)
	_, err = NewFactory(ctx, config, true)
	assert.Error(err)
}

func TestFactorySetLogger(t *testing.T) {
	assert := assert.New(t)

	testLog := logrus.WithFields(logrus.Fields{"testfield": "foobar"})
	testLog.Level = logrus.DebugLevel
	SetLogger(context.Background(), testLog)

	var config Config
	config.VMConfig.HypervisorConfig = vc.HypervisorConfig{
		KernelPath: "foo",
		ImagePath:  "bar",
	}
	ctx := context.Background()
	vf, err := NewFactory(ctx, config, false)
	assert.Nil(err)

	f, ok := vf.(*factory)
	assert.True(ok)

	assert.Equal(f.log().Logger.Level, testLog.Logger.Level)
}

func TestVMConfigValid(t *testing.T) {
	assert := assert.New(t)

	testDir, _ := ioutil.TempDir("", "vmfactory-tmp-")

	config := vc.VMConfig{
		HypervisorType: vc.MockHypervisor,
		HypervisorConfig: vc.HypervisorConfig{
			KernelPath: testDir,
			ImagePath:  testDir,
		},
	}

	f := factory{}

	err := f.validateNewVMConfig(config)
	assert.NotNil(err)

	config.AgentType = vc.NoopAgentType
	err = f.validateNewVMConfig(config)
	assert.NotNil(err)

	config.ProxyType = vc.NoopProxyType
	err = f.validateNewVMConfig(config)
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
		HypervisorConfig: hyperConfig,
		AgentType:        vc.NoopAgentType,
		ProxyType:        vc.NoopProxyType,
	}

	err := vmConfig.Valid()
	assert.Nil(err)

	ctx := context.Background()

	// direct factory
	f, err := NewFactory(ctx, Config{VMConfig: vmConfig}, false)
	assert.Nil(err)

	_, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	f.CloseFactory(ctx)

	// template factory
	f, err = NewFactory(ctx, Config{Template: true, VMConfig: vmConfig}, false)
	assert.Nil(err)

	_, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	f.CloseFactory(ctx)

	// fetch template factory
	f, err = NewFactory(ctx, Config{Template: true, VMConfig: vmConfig}, false)
	assert.Nil(err)

	_, err = NewFactory(ctx, Config{Template: true, VMConfig: vmConfig}, true)
	assert.Error(err)

	_, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	f.CloseFactory(ctx)

	// cache factory over direct factory
	f, err = NewFactory(ctx, Config{Cache: 2, VMConfig: vmConfig}, false)
	assert.Nil(err)

	_, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	f.CloseFactory(ctx)

	// cache factory over template factory
	f, err = NewFactory(ctx, Config{Template: true, Cache: 2, VMConfig: vmConfig}, false)
	assert.Nil(err)

	_, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	// CPU hotplug
	vmConfig.HypervisorConfig.NumVCPUs++
	_, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	// Memory hotplug
	vmConfig.HypervisorConfig.MemorySize += 128
	_, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	// checkConfig fall back
	vmConfig.HypervisorConfig.Mlock = true
	_, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	f.CloseFactory(ctx)
}

func TestDeepCompare(t *testing.T) {
	assert := assert.New(t)

	foo := vc.VMConfig{}
	bar := vc.VMConfig{}
	assert.True(deepCompare(foo, bar))

	foo.HypervisorConfig.NumVCPUs = 1
	assert.False(deepCompare(foo, bar))
	bar.HypervisorConfig.NumVCPUs = 1
	assert.True(deepCompare(foo, bar))

	// slice
	foo.HypervisorConfig.KernelParams = []vc.Param{}
	assert.True(deepCompare(foo, bar))
	foo.HypervisorConfig.KernelParams = append(foo.HypervisorConfig.KernelParams, vc.Param{Key: "key", Value: "value"})
	assert.False(deepCompare(foo, bar))
	bar.HypervisorConfig.KernelParams = append(bar.HypervisorConfig.KernelParams, vc.Param{Key: "key", Value: "value"})
	assert.True(deepCompare(foo, bar))

	// map
	var fooMap map[string]vc.VMConfig
	var barMap map[string]vc.VMConfig
	assert.False(deepCompare(foo, fooMap))
	assert.True(deepCompare(fooMap, barMap))
	fooMap = make(map[string]vc.VMConfig)
	assert.True(deepCompare(fooMap, barMap))
	fooMap["foo"] = foo
	assert.False(deepCompare(fooMap, barMap))
	barMap = make(map[string]vc.VMConfig)
	assert.False(deepCompare(fooMap, barMap))
	barMap["foo"] = bar
	assert.True(deepCompare(fooMap, barMap))

	// invalid interface
	var f1 vc.Factory
	var f2 vc.Factory
	var f3 base.FactoryBase
	assert.True(deepCompare(f1, f2))
	assert.True(deepCompare(f1, f3))

	// valid interface
	var config Config
	var err error
	ctx := context.Background()
	config.VMConfig = vc.VMConfig{
		HypervisorType: vc.MockHypervisor,
		AgentType:      vc.NoopAgentType,
		ProxyType:      vc.NoopProxyType,
	}
	testDir, _ := ioutil.TempDir("", "vmfactory-tmp-")
	config.VMConfig.HypervisorConfig = vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}
	f1, err = NewFactory(ctx, config, false)
	assert.Nil(err)
	assert.True(deepCompare(f1, f1))
	f2, err = NewFactory(ctx, config, false)
	assert.Nil(err)
	assert.False(deepCompare(f1, f2))
}
