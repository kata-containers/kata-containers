//
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//
// template implements base vm factory with vm templating.

package template

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"syscall"

	pb "github.com/kata-containers/kata-containers/src/runtime/protocols/cache"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory/base"
)

type template struct {
	statePath string
	config    vc.VMConfig
}

// Fetch finds and returns a pre-built template factory.
// TODO: save template metadata and fetch from storage.
func Fetch(config vc.VMConfig, templatePath string) (base.FactoryBase, error) {
	t := &template{templatePath, config}

	err := t.checkTemplateVM()
	if err != nil {
		return nil, err
	}

	return t, nil
}

// New creates a new VM template factory.
func New(ctx context.Context, config vc.VMConfig, templatePath string) (base.FactoryBase, error) {
	t := &template{templatePath, config}

	err := t.checkTemplateVM()
	if err == nil {
		return nil, fmt.Errorf("There is already a VM template in %s", templatePath)
	}

	err = t.prepareTemplateFiles()
	if err != nil {
		return nil, err
	}
	defer func() {
		if err != nil {
			t.close()
		}
	}()

	err = t.createTemplateVM(ctx)
	if err != nil {
		return nil, err
	}

	return t, nil
}

// Config returns template factory's configuration.
func (t *template) Config() vc.VMConfig {
	return t.config
}

// GetBaseVM creates a new paused VM from the template VM.
func (t *template) GetBaseVM(ctx context.Context, config vc.VMConfig) (*vc.VM, error) {
	return t.createFromTemplateVM(ctx, config)
}

// CloseFactory cleans up the template VM.
func (t *template) CloseFactory(ctx context.Context) {
	t.close()
}

// GetVMStatus is not supported
func (t *template) GetVMStatus() []*pb.GrpcVMStatus {
	panic("ERROR: package template does not support GetVMStatus")
}

func (t *template) close() {
	if err := syscall.Unmount(t.statePath, syscall.MNT_DETACH); err != nil {
		t.Logger().WithError(err).Errorf("failed to unmount %s", t.statePath)
	}

	if err := os.RemoveAll(t.statePath); err != nil {
		t.Logger().WithError(err).Errorf("failed to remove %s", t.statePath)
	}
}

func (t *template) prepareTemplateFiles() error {
	// create and mount tmpfs for the shared memory file
	err := os.MkdirAll(t.statePath, 0700)
	if err != nil {
		return err
	}
	flags := uintptr(syscall.MS_NOSUID | syscall.MS_NODEV)
	opts := fmt.Sprintf("size=%dM", t.config.HypervisorConfig.MemorySize+templateDeviceStateSize)
	if err = syscall.Mount("tmpfs", t.statePath, "tmpfs", flags, opts); err != nil {
		t.close()
		return err
	}
	f, err := os.Create(t.statePath + "/memory")
	if err != nil {
		t.close()
		return err
	}
	f.Close()

	// truncate the memory file to the exact size of the VM memory
	memoryInBytes := int64(t.config.HypervisorConfig.MemorySize) * 1024 * 1024
	t.Logger().Infof("truncating memory file %s to %d bytes", t.statePath+"/memory", memoryInBytes)
	err = os.Truncate(t.statePath+"/memory", memoryInBytes)
	if err != nil {
		t.close()
		return err
	}

	return nil
}

func (t *template) createTemplateVM(ctx context.Context) error {
	// create the template vm
	config := t.config
	config.HypervisorConfig.BootToBeTemplate = true
	config.HypervisorConfig.BootFromTemplate = false
	config.HypervisorConfig.MemoryPath = t.statePath + "/memory"
	config.HypervisorConfig.DevicesStatePath = t.deviceStatePath()
	config.HypervisorConfig.VMStorePath = t.statePath

	vm, err := vc.NewVM(ctx, config)
	if err != nil {
		return err
	}
	defer vm.Stop(ctx)

	if err = vm.Disconnect(ctx); err != nil {
		return err
	}

	if err = vm.CheckAgentReady(ctx); err != nil {
		return err
	}

	if err = vm.Pause(ctx); err != nil {
		return err
	}

	if err = vm.Save(); err != nil {
		return err
	}

	return nil
}

func (t *template) createFromTemplateVM(ctx context.Context, c vc.VMConfig) (*vc.VM, error) {
	config := t.config
	config.HypervisorConfig.BootToBeTemplate = false
	config.HypervisorConfig.BootFromTemplate = true
	config.HypervisorConfig.MemoryPath = t.statePath + "/memory"
	config.HypervisorConfig.DevicesStatePath = t.deviceStatePath()
	config.HypervisorConfig.SharedPath = c.HypervisorConfig.SharedPath
	config.HypervisorConfig.VMStorePath = c.HypervisorConfig.VMStorePath
	config.HypervisorConfig.RunStorePath = c.HypervisorConfig.RunStorePath

	return vc.NewVM(ctx, config)
}

func (t *template) checkTemplateVM() error {
	_, err := os.Stat(t.statePath + "/memory")
	if err != nil {
		return err
	}

	_, err = os.Stat(t.deviceStatePath())
	return err
}

func (t *template) deviceStatePath() string {
	stateFileName := "state"
	if t.config.HypervisorType == vc.ClhHypervisor {
		stateFileName = "state.json"
	}

	return filepath.Join(t.statePath, stateFileName)
}
