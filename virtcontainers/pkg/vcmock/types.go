// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package vcmock

import (
	"context"
	"syscall"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	vcTypes "github.com/kata-containers/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/runtime/virtcontainers/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

// Sandbox is a fake Sandbox type used for testing
type Sandbox struct {
	MockID          string
	MockURL         string
	MockAnnotations map[string]string
	MockContainers  []*Container
	MockNetNs       string
}

// Container is a fake Container type used for testing
type Container struct {
	MockID          string
	MockURL         string
	MockToken       string
	MockProcess     vc.Process
	MockPid         int
	MockSandbox     *Sandbox
	MockAnnotations map[string]string
}

// VCMock is a type that provides an implementation of the VC interface.
// It is used for testing.
type VCMock struct {
	SetLoggerFunc  func(ctx context.Context, logger *logrus.Entry)
	SetFactoryFunc func(ctx context.Context, factory vc.Factory)

	CreateSandboxFunc  func(ctx context.Context, sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error)
	DeleteSandboxFunc  func(ctx context.Context, sandboxID string) (vc.VCSandbox, error)
	ListSandboxFunc    func(ctx context.Context) ([]vc.SandboxStatus, error)
	FetchSandboxFunc   func(ctx context.Context, sandboxID string) (vc.VCSandbox, error)
	PauseSandboxFunc   func(ctx context.Context, sandboxID string) (vc.VCSandbox, error)
	ResumeSandboxFunc  func(ctx context.Context, sandboxID string) (vc.VCSandbox, error)
	RunSandboxFunc     func(ctx context.Context, sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error)
	StartSandboxFunc   func(ctx context.Context, sandboxID string) (vc.VCSandbox, error)
	StatusSandboxFunc  func(ctx context.Context, sandboxID string) (vc.SandboxStatus, error)
	StatsContainerFunc func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStats, error)
	StopSandboxFunc    func(ctx context.Context, sandboxID string) (vc.VCSandbox, error)

	CreateContainerFunc      func(ctx context.Context, sandboxID string, containerConfig vc.ContainerConfig) (vc.VCSandbox, vc.VCContainer, error)
	DeleteContainerFunc      func(ctx context.Context, sandboxID, containerID string) (vc.VCContainer, error)
	EnterContainerFunc       func(ctx context.Context, sandboxID, containerID string, cmd types.Cmd) (vc.VCSandbox, vc.VCContainer, *vc.Process, error)
	KillContainerFunc        func(ctx context.Context, sandboxID, containerID string, signal syscall.Signal, all bool) error
	StartContainerFunc       func(ctx context.Context, sandboxID, containerID string) (vc.VCContainer, error)
	StatusContainerFunc      func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error)
	StopContainerFunc        func(ctx context.Context, sandboxID, containerID string) (vc.VCContainer, error)
	ProcessListContainerFunc func(ctx context.Context, sandboxID, containerID string, options vc.ProcessListOptions) (vc.ProcessList, error)
	UpdateContainerFunc      func(ctx context.Context, sandboxID, containerID string, resources specs.LinuxResources) error
	PauseContainerFunc       func(ctx context.Context, sandboxID, containerID string) error
	ResumeContainerFunc      func(ctx context.Context, sandboxID, containerID string) error

	AddDeviceFunc func(ctx context.Context, sandboxID string, info config.DeviceInfo) (api.Device, error)

	AddInterfaceFunc    func(ctx context.Context, sandboxID string, inf *vcTypes.Interface) (*vcTypes.Interface, error)
	RemoveInterfaceFunc func(ctx context.Context, sandboxID string, inf *vcTypes.Interface) (*vcTypes.Interface, error)
	ListInterfacesFunc  func(ctx context.Context, sandboxID string) ([]*vcTypes.Interface, error)
	UpdateRoutesFunc    func(ctx context.Context, sandboxID string, routes []*vcTypes.Route) ([]*vcTypes.Route, error)
	ListRoutesFunc      func(ctx context.Context, sandboxID string) ([]*vcTypes.Route, error)
}
