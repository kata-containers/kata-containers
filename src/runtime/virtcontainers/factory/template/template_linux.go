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
	"time"

	pb "github.com/kata-containers/kata-containers/src/runtime/protocols/cache"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory/base"
)

type template struct {
	statePath string
	config    vc.VMConfig
}

var templateWaitForAgent = 2 * time.Second

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
	// The template source VM is backed by a shared memory file so that clones
	// can map the same file. The factory expresses this through the generic
	// file-backed memory config rather than template-specific flags.
	config.HypervisorConfig.FileBackedMemory = &vc.FileBackedMemoryConfig{
		Path:   t.statePath + "/memory",
		Shared: true,
	}
	config.HypervisorConfig.VMStorePath = t.statePath

	vm, err := vc.NewVM(ctx, config)
	if err != nil {
		return err
	}
	defer vm.Stop(ctx)

	if err = vm.Disconnect(ctx); err != nil {
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

	if err = vm.Pause(ctx); err != nil {
		return err
	}

	if err = vm.Save(t.statePath); err != nil {
		return err
	}

	// The template source VM runs with shared memory so that clones can map
	// the same backing file, but the snapshot must record the memory as
	// private so that clones restored from it get Copy-On-Write memory. The
	// factory owns this policy decision (when to make a snapshot private),
	// while the CLH snapshot-format details live in
	// vc.PatchCLHSnapshotMemoryPrivate. Only Cloud Hypervisor records a
	// config.json that needs patching; QEMU's device-state file does not.
	if t.config.HypervisorType == vc.ClhHypervisor {
		if err = vc.PatchCLHSnapshotMemoryPrivate(t.statePath); err != nil {
			return err
		}
	}

	return nil
}

func (t *template) createFromTemplateVM(ctx context.Context, c vc.VMConfig) (*vc.VM, error) {
	config := t.config
	// Clones restored from the template use private Copy-On-Write memory
	// backed by the template's shared memory file.
	config.HypervisorConfig.FileBackedMemory = &vc.FileBackedMemoryConfig{
		Path:   t.statePath + "/memory",
		Shared: false,
	}
	config.HypervisorConfig.SharedPath = c.HypervisorConfig.SharedPath
	config.HypervisorConfig.VMStorePath = c.HypervisorConfig.VMStorePath
	config.HypervisorConfig.RunStorePath = c.HypervisorConfig.RunStorePath

	return vc.NewVMFromSnapshot(ctx, config, t.statePath)
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
