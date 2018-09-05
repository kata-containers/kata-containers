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
	"syscall"
	"time"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/factory/base"
	"github.com/kata-containers/runtime/virtcontainers/factory/direct"
)

type template struct {
	statePath string
	config    vc.VMConfig
}

// Fetch finds and returns a pre-built template factory.
// TODO: save template metadata and fetch from storage.
func Fetch(config vc.VMConfig) (base.FactoryBase, error) {
	statePath := vc.RunVMStoragePath + "/template"
	t := &template{statePath, config}

	err := t.checkTemplateVM()
	if err != nil {
		return nil, err
	}

	return t, nil
}

// New creates a new VM template factory.
func New(ctx context.Context, config vc.VMConfig) base.FactoryBase {
	statePath := vc.RunVMStoragePath + "/template"
	t := &template{statePath, config}

	err := t.prepareTemplateFiles()
	if err != nil {
		// fallback to direct factory if template is not supported.
		return direct.New(ctx, config)
	}

	err = t.createTemplateVM(ctx)
	if err != nil {
		// fallback to direct factory if template is not supported.
		return direct.New(ctx, config)
	}

	return t
}

// Config returns template factory's configuration.
func (t *template) Config() vc.VMConfig {
	return t.config
}

// GetBaseVM creates a new paused VM from the template VM.
func (t *template) GetBaseVM(ctx context.Context) (*vc.VM, error) {
	return t.createFromTemplateVM(ctx)
}

// CloseFactory cleans up the template VM.
func (t *template) CloseFactory(ctx context.Context) {
	syscall.Unmount(t.statePath, 0)
	os.RemoveAll(t.statePath)
}

func (t *template) prepareTemplateFiles() error {
	// create and mount tmpfs for the shared memory file
	err := os.MkdirAll(t.statePath, 0700)
	if err != nil {
		return err
	}
	flags := uintptr(syscall.MS_NOSUID | syscall.MS_NODEV)
	opts := fmt.Sprintf("size=%dM", t.config.HypervisorConfig.MemorySize+8)
	if err = syscall.Mount("tmpfs", t.statePath, "tmpfs", flags, opts); err != nil {
		return err
	}
	f, err := os.Create(t.statePath + "/memory")
	if err != nil {
		return err
	}
	f.Close()

	return nil
}

func (t *template) createTemplateVM(ctx context.Context) error {
	// create the template vm
	config := t.config
	config.HypervisorConfig.BootToBeTemplate = true
	config.HypervisorConfig.BootFromTemplate = false
	config.HypervisorConfig.MemoryPath = t.statePath + "/memory"
	config.HypervisorConfig.DevicesStatePath = t.statePath + "/state"

	vm, err := vc.NewVM(ctx, config)
	if err != nil {
		return err
	}
	defer vm.Stop()

	err = vm.Pause()
	if err != nil {
		return err
	}

	err = vm.Save()
	if err != nil {
		return err
	}

	// qemu QMP does not wait for migration to finish...
	time.Sleep(1 * time.Second)

	return nil
}

func (t *template) createFromTemplateVM(ctx context.Context) (*vc.VM, error) {
	config := t.config
	config.HypervisorConfig.BootToBeTemplate = false
	config.HypervisorConfig.BootFromTemplate = true
	config.HypervisorConfig.MemoryPath = t.statePath + "/memory"
	config.HypervisorConfig.DevicesStatePath = t.statePath + "/state"

	return vc.NewVM(ctx, config)
}

func (t *template) checkTemplateVM() error {
	_, err := os.Stat(t.statePath + "/memory")
	if err != nil {
		return err
	}

	_, err = os.Stat(t.statePath + "/state")
	return err
}
