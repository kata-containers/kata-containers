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

func (f *Config) validate() error {
	return f.VMConfig.Valid()
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

	err := config.validate()
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

func resetHypervisorConfig(config *vc.HypervisorConfig) {
	config.NumVCPUs = 0
	config.MemorySize = 0
	config.BootToBeTemplate = false
	config.BootFromTemplate = false
	config.MemoryPath = ""
	config.DevicesStatePath = ""
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
	resetHypervisorConfig(&config1.HypervisorConfig)
	resetHypervisorConfig(&config2.HypervisorConfig)

	if !reflect.DeepEqual(config1, config2) {
		return fmt.Errorf("hypervisor config does not match, base: %+v. new: %+v", config1, config2)
	}

	return nil
}

func (f *factory) checkConfig(config vc.VMConfig) error {
	baseConfig := f.base.Config()

	return checkVMConfig(config, baseConfig)
}

// GetVM returns a working blank VM created by the factory.
func (f *factory) GetVM(ctx context.Context, config vc.VMConfig) (*vc.VM, error) {
	span, _ := trace(ctx, "GetVM")
	defer span.Finish()

	hypervisorConfig := config.HypervisorConfig
	err := config.Valid()
	if err != nil {
		f.log().WithError(err).Error("invalid hypervisor config")
		return nil, err
	}

	err = f.checkConfig(config)
	if err != nil {
		f.log().WithError(err).Info("fallback to direct factory vm")
		return direct.New(ctx, config).GetBaseVM(ctx)
	}

	f.log().Info("get base VM")
	vm, err := f.base.GetBaseVM(ctx)
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
