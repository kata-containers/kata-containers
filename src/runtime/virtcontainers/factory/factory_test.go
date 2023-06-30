// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package factory

import (
	"context"
	"os"
	"strings"
	"testing"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory/base"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/mock"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
)

const testDisabledAsNonRoot = "Test disabled as requires root privileges"

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
	}

	_, err = NewFactory(ctx, config, false)
	assert.Error(err)

	testDir := t.TempDir()
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
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)
	vc.MockHybridVSockPath = url

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	config.Template = true
	config.TemplatePath = testDir
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

	err = checkVMConfig(config1, config2)
	assert.Nil(err)

	testDir := t.TempDir()

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

	testDir := t.TempDir()

	hyperConfig := vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}
	vmConfig := vc.VMConfig{
		HypervisorType:   vc.MockHypervisor,
		HypervisorConfig: hyperConfig,
	}

	err := vmConfig.Valid()
	assert.Nil(err)

	ctx := context.Background()

	// direct factory
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)
	vc.MockHybridVSockPath = url

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	f, err := NewFactory(ctx, Config{VMConfig: vmConfig}, false)
	assert.Nil(err)

	vm, err := f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	f.CloseFactory(ctx)

	// template factory
	f, err = NewFactory(ctx, Config{Template: true, TemplatePath: testDir, VMConfig: vmConfig}, false)
	assert.Nil(err)

	vm, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	f.CloseFactory(ctx)

	// fetch template factory
	f, err = NewFactory(ctx, Config{Template: true, TemplatePath: testDir, VMConfig: vmConfig}, false)
	assert.Nil(err)

	_, err = NewFactory(ctx, Config{Template: true, TemplatePath: testDir, VMConfig: vmConfig}, true)
	assert.Error(err)

	vm, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	f.CloseFactory(ctx)

	// cache factory over direct factory
	f, err = NewFactory(ctx, Config{Cache: 2, VMConfig: vmConfig}, false)
	assert.Nil(err)

	vm, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	f.CloseFactory(ctx)

	// cache factory over template factory
	f, err = NewFactory(ctx, Config{Template: true, TemplatePath: testDir, Cache: 2, VMConfig: vmConfig}, false)
	assert.Nil(err)

	vm, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	// CPU hotplug
	vmConfig.HypervisorConfig.NumVCPUsF++
	vm, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	// Memory hotplug
	vmConfig.HypervisorConfig.MemorySize += 128
	vm, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	// checkConfig fall back
	vm, err = f.GetVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	f.CloseFactory(ctx)
}

func TestDeepCompare(t *testing.T) {
	assert := assert.New(t)

	foo := vc.VMConfig{}
	bar := vc.VMConfig{}
	assert.True(utils.DeepCompare(foo, bar))

	foo.HypervisorConfig.NumVCPUsF = 1
	assert.False(utils.DeepCompare(foo, bar))
	bar.HypervisorConfig.NumVCPUsF = 1
	assert.True(utils.DeepCompare(foo, bar))

	// slice
	foo.HypervisorConfig.KernelParams = []vc.Param{}
	assert.True(utils.DeepCompare(foo, bar))
	foo.HypervisorConfig.KernelParams = append(foo.HypervisorConfig.KernelParams, vc.Param{Key: "key", Value: "value"})
	assert.False(utils.DeepCompare(foo, bar))
	bar.HypervisorConfig.KernelParams = append(bar.HypervisorConfig.KernelParams, vc.Param{Key: "key", Value: "value"})
	assert.True(utils.DeepCompare(foo, bar))

	// map
	var fooMap map[string]vc.VMConfig
	var barMap map[string]vc.VMConfig
	assert.False(utils.DeepCompare(foo, fooMap))
	assert.True(utils.DeepCompare(fooMap, barMap))
	fooMap = make(map[string]vc.VMConfig)
	assert.True(utils.DeepCompare(fooMap, barMap))
	fooMap["foo"] = foo
	assert.False(utils.DeepCompare(fooMap, barMap))
	barMap = make(map[string]vc.VMConfig)
	assert.False(utils.DeepCompare(fooMap, barMap))
	barMap["foo"] = bar
	assert.True(utils.DeepCompare(fooMap, barMap))

	// invalid interface
	var f1 vc.Factory
	var f2 vc.Factory
	var f3 base.FactoryBase
	assert.True(utils.DeepCompare(f1, f2))
	assert.True(utils.DeepCompare(f1, f3))

	// valid interface
	var config Config
	var err error
	ctx := context.Background()
	config.VMConfig = vc.VMConfig{
		HypervisorType: vc.MockHypervisor,
	}
	testDir := t.TempDir()

	config.VMConfig.HypervisorConfig = vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}
	f1, err = NewFactory(ctx, config, false)
	assert.Nil(err)
	assert.True(utils.DeepCompare(f1, f1))
	f2, err = NewFactory(ctx, config, false)
	assert.Nil(err)
	assert.False(utils.DeepCompare(f1, f2))
}

func TestFactoryConfig(t *testing.T) {
	assert := assert.New(t)

	// Valid config
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

	vmc := f.Config()

	assert.Equal(config.VMConfig.HypervisorConfig.KernelPath, vmc.HypervisorConfig.KernelPath)
	assert.Equal(config.VMConfig.HypervisorConfig.ImagePath, vmc.HypervisorConfig.ImagePath)
}

func TestFactoryGetBaseVM(t *testing.T) {
	assert := assert.New(t)

	// Set configs
	var config Config
	testDir := t.TempDir()

	hyperConfig := vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}
	vmConfig := vc.VMConfig{
		HypervisorType:   vc.MockHypervisor,
		HypervisorConfig: hyperConfig,
	}
	config.VMConfig = vmConfig
	config.TemplatePath = testDir

	err := vmConfig.Valid()
	assert.Nil(err)

	ctx := context.Background()

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)
	vc.MockHybridVSockPath = url

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	// New factory
	vf, err := NewFactory(ctx, config, false)
	assert.Nil(err)

	f, ok := vf.(*factory)
	assert.True(ok)

	// Check VM Config
	assert.Equal(f.Config(), vmConfig)

	// GetBaseVM
	vm, err := f.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	// Get VM Status
	defer func() {
		r := recover()
		assert.NotNil(r)

		// Close
		err = vm.Stop(ctx)
		assert.Nil(err)
	}()
	vmStatus := f.GetVMStatus()
	assert.NotNil(vmStatus) // line of code to make golang happy. This is never executed.
}

func TestNewFactoryWithCache(t *testing.T) {
	assert := assert.New(t)

	// Config
	var config Config
	config.VMConfig.HypervisorConfig = vc.HypervisorConfig{
		KernelPath: "foo",
		ImagePath:  "bar",
	}
	ctx := context.Background()

	// cache>0 and fetch only should throw error
	config.Cache = 1
	vf, err := NewFactory(ctx, config, true)

	assert.Nil(vf)
	assert.Error(err)
	b := err.Error()
	assert.True(strings.Contains(b, "cache factory does not support fetch"))
}

func TestNewFactoryWrongCacheEndpoint(t *testing.T) {
	assert := assert.New(t)

	// Config
	var config Config
	config.VMConfig.HypervisorConfig = vc.HypervisorConfig{
		KernelPath: "foo",
		ImagePath:  "bar",
	}
	ctx := context.Background()

	config.VMCache = true
	vf, err := NewFactory(ctx, config, false)

	assert.Nil(vf)
	assert.Error(err)
	b := err.Error()
	assert.True(strings.Contains(b, "rpc error")) // sanity check
}
