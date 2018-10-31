// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Description: The true virtcontainers function of the same name.
// This indirection is required to allow an alternative implemenation to be
// used for testing purposes.

package virtcontainers

import (
	"context"
	"syscall"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/pkg/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

// VCImpl is the official virtcontainers function of the same name.
type VCImpl struct {
	factory Factory
}

// SetLogger implements the VC function of the same name.
func (impl *VCImpl) SetLogger(ctx context.Context, logger *logrus.Entry) {
	SetLogger(ctx, logger)
}

// SetFactory implements the VC function of the same name.
func (impl *VCImpl) SetFactory(ctx context.Context, factory Factory) {
	impl.factory = factory
}

// CreateSandbox implements the VC function of the same name.
func (impl *VCImpl) CreateSandbox(ctx context.Context, sandboxConfig SandboxConfig) (VCSandbox, error) {
	return CreateSandbox(ctx, sandboxConfig, impl.factory)
}

// DeleteSandbox implements the VC function of the same name.
func (impl *VCImpl) DeleteSandbox(ctx context.Context, sandboxID string) (VCSandbox, error) {
	return DeleteSandbox(ctx, sandboxID)
}

// StartSandbox implements the VC function of the same name.
func (impl *VCImpl) StartSandbox(ctx context.Context, sandboxID string) (VCSandbox, error) {
	return StartSandbox(ctx, sandboxID)
}

// StopSandbox implements the VC function of the same name.
func (impl *VCImpl) StopSandbox(ctx context.Context, sandboxID string) (VCSandbox, error) {
	return StopSandbox(ctx, sandboxID)
}

// RunSandbox implements the VC function of the same name.
func (impl *VCImpl) RunSandbox(ctx context.Context, sandboxConfig SandboxConfig) (VCSandbox, error) {
	return RunSandbox(ctx, sandboxConfig, impl.factory)
}

// ListSandbox implements the VC function of the same name.
func (impl *VCImpl) ListSandbox(ctx context.Context) ([]SandboxStatus, error) {
	return ListSandbox(ctx)
}

// FetchSandbox will find out and connect to an existing sandbox and
// return the sandbox structure.
func (impl *VCImpl) FetchSandbox(ctx context.Context, sandboxID string) (VCSandbox, error) {
	return FetchSandbox(ctx, sandboxID)
}

// StatusSandbox implements the VC function of the same name.
func (impl *VCImpl) StatusSandbox(ctx context.Context, sandboxID string) (SandboxStatus, error) {
	return StatusSandbox(ctx, sandboxID)
}

// PauseSandbox implements the VC function of the same name.
func (impl *VCImpl) PauseSandbox(ctx context.Context, sandboxID string) (VCSandbox, error) {
	return PauseSandbox(ctx, sandboxID)
}

// ResumeSandbox implements the VC function of the same name.
func (impl *VCImpl) ResumeSandbox(ctx context.Context, sandboxID string) (VCSandbox, error) {
	return ResumeSandbox(ctx, sandboxID)
}

// CreateContainer implements the VC function of the same name.
func (impl *VCImpl) CreateContainer(ctx context.Context, sandboxID string, containerConfig ContainerConfig) (VCSandbox, VCContainer, error) {
	return CreateContainer(ctx, sandboxID, containerConfig)
}

// DeleteContainer implements the VC function of the same name.
func (impl *VCImpl) DeleteContainer(ctx context.Context, sandboxID, containerID string) (VCContainer, error) {
	return DeleteContainer(ctx, sandboxID, containerID)
}

// StartContainer implements the VC function of the same name.
func (impl *VCImpl) StartContainer(ctx context.Context, sandboxID, containerID string) (VCContainer, error) {
	return StartContainer(ctx, sandboxID, containerID)
}

// StopContainer implements the VC function of the same name.
func (impl *VCImpl) StopContainer(ctx context.Context, sandboxID, containerID string) (VCContainer, error) {
	return StopContainer(ctx, sandboxID, containerID)
}

// EnterContainer implements the VC function of the same name.
func (impl *VCImpl) EnterContainer(ctx context.Context, sandboxID, containerID string, cmd Cmd) (VCSandbox, VCContainer, *Process, error) {
	return EnterContainer(ctx, sandboxID, containerID, cmd)
}

// StatusContainer implements the VC function of the same name.
func (impl *VCImpl) StatusContainer(ctx context.Context, sandboxID, containerID string) (ContainerStatus, error) {
	return StatusContainer(ctx, sandboxID, containerID)
}

// StatsContainer implements the VC function of the same name.
func (impl *VCImpl) StatsContainer(ctx context.Context, sandboxID, containerID string) (ContainerStats, error) {
	return StatsContainer(ctx, sandboxID, containerID)
}

// KillContainer implements the VC function of the same name.
func (impl *VCImpl) KillContainer(ctx context.Context, sandboxID, containerID string, signal syscall.Signal, all bool) error {
	return KillContainer(ctx, sandboxID, containerID, signal, all)
}

// ProcessListContainer implements the VC function of the same name.
func (impl *VCImpl) ProcessListContainer(ctx context.Context, sandboxID, containerID string, options ProcessListOptions) (ProcessList, error) {
	return ProcessListContainer(ctx, sandboxID, containerID, options)
}

// UpdateContainer implements the VC function of the same name.
func (impl *VCImpl) UpdateContainer(ctx context.Context, sandboxID, containerID string, resources specs.LinuxResources) error {
	return UpdateContainer(ctx, sandboxID, containerID, resources)
}

// PauseContainer implements the VC function of the same name.
func (impl *VCImpl) PauseContainer(ctx context.Context, sandboxID, containerID string) error {
	return PauseContainer(ctx, sandboxID, containerID)
}

// ResumeContainer implements the VC function of the same name.
func (impl *VCImpl) ResumeContainer(ctx context.Context, sandboxID, containerID string) error {
	return ResumeContainer(ctx, sandboxID, containerID)
}

// AddDevice will add a device to sandbox
func (impl *VCImpl) AddDevice(ctx context.Context, sandboxID string, info config.DeviceInfo) (api.Device, error) {
	return AddDevice(ctx, sandboxID, info)
}

// AddInterface implements the VC function of the same name.
func (impl *VCImpl) AddInterface(ctx context.Context, sandboxID string, inf *types.Interface) (*types.Interface, error) {
	return AddInterface(ctx, sandboxID, inf)
}

// RemoveInterface implements the VC function of the same name.
func (impl *VCImpl) RemoveInterface(ctx context.Context, sandboxID string, inf *types.Interface) (*types.Interface, error) {
	return RemoveInterface(ctx, sandboxID, inf)
}

// ListInterfaces implements the VC function of the same name.
func (impl *VCImpl) ListInterfaces(ctx context.Context, sandboxID string) ([]*types.Interface, error) {
	return ListInterfaces(ctx, sandboxID)
}

// UpdateRoutes implements the VC function of the same name.
func (impl *VCImpl) UpdateRoutes(ctx context.Context, sandboxID string, routes []*types.Route) ([]*types.Route, error) {
	return UpdateRoutes(ctx, sandboxID, routes)
}

// ListRoutes implements the VC function of the same name.
func (impl *VCImpl) ListRoutes(ctx context.Context, sandboxID string) ([]*types.Route, error) {
	return ListRoutes(ctx, sandboxID)
}
