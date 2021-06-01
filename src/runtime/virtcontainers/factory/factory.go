// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package factory

import (
	"context"
	"fmt"

	pb "github.com/kata-containers/kata-containers/src/runtime/protocols/cache"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory/base"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory/cache"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory/direct"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory/grpccache"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory/template"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/sirupsen/logrus"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/label"
	otelTrace "go.opentelemetry.io/otel/trace"
)

var factoryLogger = logrus.FieldLogger(logrus.New())

// Config is a collection of VM factory configurations.
type Config struct {
	Template        bool
	VMCache         bool
	Cache           uint
	TemplatePath    string
	VMCacheEndpoint string

	VMConfig vc.VMConfig
}

type factory struct {
	base base.FactoryBase
}

func trace(parent context.Context, name string) (otelTrace.Span, context.Context) {
	tracer := otel.Tracer("kata")
	ctx, span := tracer.Start(parent, name, otelTrace.WithAttributes(label.String("source", "runtime"), label.String("package", "factory"), label.String("subsystem", "factory")))

	return span, ctx
}

// NewFactory returns a working factory.
func NewFactory(ctx context.Context, config Config, fetchOnly bool) (vc.Factory, error) {
	span, _ := trace(ctx, "NewFactory")
	defer span.End()

	err := config.VMConfig.Valid()
	if err != nil {
		return nil, err
	}

	if fetchOnly && config.Cache > 0 {
		return nil, fmt.Errorf("cache factory does not support fetch")
	}

	var b base.FactoryBase
	if config.VMCache && config.Cache == 0 {
		// For VMCache client
		b, err = grpccache.New(ctx, config.VMCacheEndpoint)
		if err != nil {
			return nil, err
		}
	} else {
		if config.Template {
			if fetchOnly {
				b, err = template.Fetch(config.VMConfig, config.TemplatePath)
				if err != nil {
					return nil, err
				}
			} else {
				b, err = template.New(ctx, config.VMConfig, config.TemplatePath)
				if err != nil {
					return nil, err
				}
			}
		} else {
			b = direct.New(ctx, config.VMConfig)
		}

		if config.Cache > 0 {
			b = cache.New(ctx, config.Cache, b)
		}
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
}

// It's important that baseConfig and newConfig are passed by value!
func checkVMConfig(config1, config2 vc.VMConfig) error {
	if config1.HypervisorType != config2.HypervisorType {
		return fmt.Errorf("hypervisor type does not match: %s vs. %s", config1.HypervisorType, config2.HypervisorType)
	}

	// check hypervisor config details
	resetHypervisorConfig(&config1)
	resetHypervisorConfig(&config2)

	if !utils.DeepCompare(config1, config2) {
		return fmt.Errorf("hypervisor config does not match, base: %+v. new: %+v", config1, config2)
	}

	return nil
}

func (f *factory) checkConfig(config vc.VMConfig) error {
	baseConfig := f.base.Config()

	return checkVMConfig(baseConfig, config)
}

// GetVM returns a working blank VM created by the factory.
func (f *factory) GetVM(ctx context.Context, config vc.VMConfig) (*vc.VM, error) {
	span, ctx := trace(ctx, "GetVM")
	defer span.End()

	hypervisorConfig := config.HypervisorConfig
	if err := config.Valid(); err != nil {
		f.log().WithError(err).Error("invalid hypervisor config")
		return nil, err
	}

	err := f.checkConfig(config)
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
			vm.Stop(ctx)
		}
	}()

	err = vm.Resume(ctx)
	if err != nil {
		return nil, err
	}

	// reseed RNG so that shared memory VMs do not generate same random numbers.
	err = vm.ReseedRNG(ctx)
	if err != nil {
		return nil, err
	}

	// sync guest time since we might have paused it for a long time.
	err = vm.SyncTime(ctx)
	if err != nil {
		return nil, err
	}

	online := false
	baseConfig := f.base.Config().HypervisorConfig
	if baseConfig.NumVCPUs < hypervisorConfig.NumVCPUs {
		err = vm.AddCPUs(ctx, hypervisorConfig.NumVCPUs-baseConfig.NumVCPUs)
		if err != nil {
			return nil, err
		}
		online = true
	}

	if baseConfig.MemorySize < hypervisorConfig.MemorySize {
		err = vm.AddMemory(ctx, hypervisorConfig.MemorySize-baseConfig.MemorySize)
		if err != nil {
			return nil, err
		}
		online = true
	}

	if online {
		err = vm.OnlineCPUMemory(ctx)
		if err != nil {
			return nil, err
		}
	}

	return vm, nil
}

// Config returns base factory config.
func (f *factory) Config() vc.VMConfig {
	return f.base.Config()
}

// GetVMStatus returns the status of the paused VM created by the base factory.
func (f *factory) GetVMStatus() []*pb.GrpcVMStatus {
	return f.base.GetVMStatus()
}

// GetBaseVM returns a paused VM created by the base factory.
func (f *factory) GetBaseVM(ctx context.Context, config vc.VMConfig) (*vc.VM, error) {
	return f.base.GetBaseVM(ctx, config)
}

// CloseFactory closes the factory.
func (f *factory) CloseFactory(ctx context.Context) {
	f.base.CloseFactory(ctx)
}
