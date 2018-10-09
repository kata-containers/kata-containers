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

var templateProxyType = vc.KataBuiltInProxyType
var templateWaitForAgent = 2 * time.Second

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
	defer func() {
		if err != nil {
			t.close()
		}
	}()

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
func (t *template) GetBaseVM(ctx context.Context, config vc.VMConfig) (*vc.VM, error) {
	return t.createFromTemplateVM(ctx, config)
}

// CloseFactory cleans up the template VM.
func (t *template) CloseFactory(ctx context.Context) {
	t.close()
}

func (t *template) close() {
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
		t.close()
		return err
	}
	f, err := os.Create(t.statePath + "/memory")
	if err != nil {
		t.close()
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
	// template vm uses builtin proxy
	config.ProxyType = templateProxyType

	vm, err := vc.NewVM(ctx, config)
	if err != nil {
		return err
	}
	defer vm.Stop()

	if err = vm.Disconnect(); err != nil {
		return err
	}

	// Sleep a bit to let the agent grpc server clean up
	// When we close connection to the agent, it needs sometime to cleanup
	// and restart listening on the communication( serial or vsock) port.
	// That time can be saved if we sleep a bit to wait for the agent to
	// come around and start listening again. The sleep is only done when
	// creating new vm templates and saves time for every new vm that are
	// created from template, so it worth the invest.
	time.Sleep(templateWaitForAgent)

	if err = vm.Pause(); err != nil {
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
	config.HypervisorConfig.DevicesStatePath = t.statePath + "/state"
	config.ProxyType = c.ProxyType
	config.ProxyConfig = c.ProxyConfig

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
