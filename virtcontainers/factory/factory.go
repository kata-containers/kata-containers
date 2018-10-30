// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package factory

import (
	"context"
	"fmt"
	"reflect"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/factory/base"
	"github.com/kata-containers/runtime/virtcontainers/factory/cache"
	"github.com/kata-containers/runtime/virtcontainers/factory/direct"
	"github.com/kata-containers/runtime/virtcontainers/factory/template"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/sirupsen/logrus"
)

var factoryLogger = logrus.FieldLogger(logrus.New())

// Config is a collection of VM factory configurations.
type Config struct {
	Template bool
	Cache    uint

	VMConfig vc.VMConfig
}

type factory struct {
	base base.FactoryBase
}

func trace(parent context.Context, name string) (opentracing.Span, context.Context) {
	span, ctx := opentracing.StartSpanFromContext(parent, name)

	span.SetTag("subsystem", "factory")

	return span, ctx
}

// NewFactory returns a working factory.
func NewFactory(ctx context.Context, config Config, fetchOnly bool) (vc.Factory, error) {
	span, _ := trace(ctx, "NewFactory")
	defer span.Finish()

	err := config.VMConfig.Valid()
	if err != nil {
		return nil, err
	}

	if fetchOnly && config.Cache > 0 {
		return nil, fmt.Errorf("cache factory does not support fetch")
	}

	var b base.FactoryBase
	if config.Template {
		if fetchOnly {
			b, err = template.Fetch(config.VMConfig)
			if err != nil {
				return nil, err
			}
		} else {
			b = template.New(ctx, config.VMConfig)
		}
	} else {
		b = direct.New(ctx, config.VMConfig)
	}

	if config.Cache > 0 {
		b = cache.New(ctx, config.Cache, b)
	}

	return &factory{b}, nil
}

// SetLogger sets the logger for the factory.
func SetLogger(ctx context.Context, logger logrus.FieldLogger) {
	fields := logrus.Fields{
		"source": "virtcontainers",
	}

	factoryLogger = logger.WithFields(fields)
}

func (f *factory) log() *logrus.Entry {
	return factoryLogger.WithField("subsystem", "factory")
}

func resetHypervisorConfig(config *vc.VMConfig) {
	config.HypervisorConfig.NumVCPUs = 0
	config.HypervisorConfig.MemorySize = 0
	config.HypervisorConfig.BootToBeTemplate = false
	config.HypervisorConfig.BootFromTemplate = false
	config.HypervisorConfig.MemoryPath = ""
	config.HypervisorConfig.DevicesStatePath = ""
	config.ProxyType = vc.NoopProxyType
	config.ProxyConfig = vc.ProxyConfig{}
}

func compareStruct(foo, bar reflect.Value) bool {
	for i := 0; i < foo.NumField(); i++ {
		if !deepCompareValue(foo.Field(i), bar.Field(i)) {
			return false
		}
	}

	return true
}

func compareMap(foo, bar reflect.Value) bool {
	if foo.Len() != bar.Len() {
		return false
	}

	for _, k := range foo.MapKeys() {
		if !deepCompareValue(foo.MapIndex(k), bar.MapIndex(k)) {
			return false
		}
	}

	return true
}

func compareSlice(foo, bar reflect.Value) bool {
	if foo.Len() != bar.Len() {
		return false
	}
	for j := 0; j < foo.Len(); j++ {
		if !deepCompareValue(foo.Index(j), bar.Index(j)) {
			return false
		}
	}
	return true
}

func deepCompareValue(foo, bar reflect.Value) bool {
	if !foo.IsValid() || !bar.IsValid() {
		return foo.IsValid() == bar.IsValid()
	}

	if foo.Type() != bar.Type() {
		return false
	}
	switch foo.Kind() {
	case reflect.Map:
		return compareMap(foo, bar)
	case reflect.Array:
		fallthrough
	case reflect.Slice:
		return compareSlice(foo, bar)
	case reflect.Struct:
		return compareStruct(foo, bar)
	default:
		return foo.Interface() == bar.Interface()
	}
}

func deepCompare(foo, bar interface{}) bool {
	v1 := reflect.ValueOf(foo)
	v2 := reflect.ValueOf(bar)

	return deepCompareValue(v1, v2)
}

// It's important that baseConfig and newConfig are passed by value!
func checkVMConfig(config1, config2 vc.VMConfig) error {
	if config1.HypervisorType != config2.HypervisorType {
		return fmt.Errorf("hypervisor type does not match: %s vs. %s", config1.HypervisorType, config2.HypervisorType)
	}

	if config1.AgentType != config2.AgentType {
		return fmt.Errorf("agent type does not match: %s vs. %s", config1.AgentType, config2.AgentType)
	}

	// check hypervisor config details
	resetHypervisorConfig(&config1)
	resetHypervisorConfig(&config2)

	if !deepCompare(config1, config2) {
		return fmt.Errorf("hypervisor config does not match, base: %+v. new: %+v", config1, config2)
	}

	return nil
}

func (f *factory) checkConfig(config vc.VMConfig) error {
	baseConfig := f.base.Config()

	return checkVMConfig(config, baseConfig)
}

func (f *factory) validateNewVMConfig(config vc.VMConfig) error {
	if len(config.AgentType.String()) == 0 {
		return fmt.Errorf("Missing agent type")
	}

	if len(config.ProxyType.String()) == 0 {
		return fmt.Errorf("Missing proxy type")
	}

	return config.Valid()
}

// GetVM returns a working blank VM created by the factory.
func (f *factory) GetVM(ctx context.Context, config vc.VMConfig) (*vc.VM, error) {
	span, _ := trace(ctx, "GetVM")
	defer span.Finish()

	hypervisorConfig := config.HypervisorConfig
	err := f.validateNewVMConfig(config)
	if err != nil {
		f.log().WithError(err).Error("invalid hypervisor config")
		return nil, err
	}

	err = f.checkConfig(config)
	if err != nil {
		f.log().WithError(err).Info("fallback to direct factory vm")
		return direct.New(ctx, config).GetBaseVM(ctx, config)
	}

	f.log().Info("get base VM")
	vm, err := f.base.GetBaseVM(ctx, config)
	if err != nil {
		f.log().WithError(err).Error("failed to get base VM")
		return nil, err
	}

	// cleanup upon error
	defer func() {
		if err != nil {
			f.log().WithError(err).Error("clean up vm")
			vm.Stop()
		}
	}()

	err = vm.Resume()
	if err != nil {
		return nil, err
	}

	// reseed RNG so that shared memory VMs do not generate same random numbers.
	err = vm.ReseedRNG()
	if err != nil {
		return nil, err
	}

	online := false
	baseConfig := f.base.Config().HypervisorConfig
	if baseConfig.NumVCPUs < hypervisorConfig.NumVCPUs {
		err = vm.AddCPUs(hypervisorConfig.NumVCPUs - baseConfig.NumVCPUs)
		if err != nil {
			return nil, err
		}
		online = true
	}

	if baseConfig.MemorySize < hypervisorConfig.MemorySize {
		err = vm.AddMemory(hypervisorConfig.MemorySize - baseConfig.MemorySize)
		if err != nil {
			return nil, err
		}
		online = true
	}

	if online {
		err = vm.OnlineCPUMemory()
		if err != nil {
			return nil, err
		}
	}

	return vm, nil
}

// CloseFactory closes the factory.
func (f *factory) CloseFactory(ctx context.Context) {
	f.base.CloseFactory(ctx)
}
