// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package factory

import (
	"fmt"
	"reflect"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/factory/base"
	"github.com/kata-containers/runtime/virtcontainers/factory/cache"
	"github.com/kata-containers/runtime/virtcontainers/factory/direct"
	"github.com/kata-containers/runtime/virtcontainers/factory/template"
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

// NewFactory returns a working factory.
func NewFactory(config Config, fetchOnly bool) (vc.Factory, error) {
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
			b = template.New(config.VMConfig)
		}
	} else {
		b = direct.New(config.VMConfig)
	}

	if config.Cache > 0 {
		b = cache.New(config.Cache, b)
	}

	return &factory{b}, nil
}

// SetLogger sets the logger for the factory.
func SetLogger(logger logrus.FieldLogger) {
	fields := logrus.Fields{
		"source": "virtcontainers",
	}

	factoryLogger = logger.WithFields(fields)
}

func (f *factory) log() *logrus.Entry {
	return factoryLogger.WithField("subsystem", "factory")
}

func resetHypervisorConfig(config *vc.HypervisorConfig) {
	config.DefaultVCPUs = 0
	config.DefaultMemSz = 0
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
func (f *factory) GetVM(config vc.VMConfig) (*vc.VM, error) {
	hypervisorConfig := config.HypervisorConfig
	err := config.Valid()
	if err != nil {
		f.log().WithError(err).Error("invalid hypervisor config")
		return nil, err
	}

	err = f.checkConfig(config)
	if err != nil {
		f.log().WithError(err).Info("fallback to direct factory vm")
		return direct.New(config).GetBaseVM()
	}

	f.log().Info("get base VM")
	vm, err := f.base.GetBaseVM()
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

	online := false
	baseConfig := f.base.Config().HypervisorConfig
	if baseConfig.DefaultVCPUs < hypervisorConfig.DefaultVCPUs {
		err = vm.AddCPUs(hypervisorConfig.DefaultVCPUs - baseConfig.DefaultVCPUs)
		if err != nil {
			return nil, err
		}
		online = true
	}

	if baseConfig.DefaultMemSz < hypervisorConfig.DefaultMemSz {
		err = vm.AddMemory(hypervisorConfig.DefaultMemSz - baseConfig.DefaultMemSz)
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
func (f *factory) CloseFactory() {
	f.base.CloseFactory()
}
