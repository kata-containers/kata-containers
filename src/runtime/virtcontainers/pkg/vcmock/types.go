// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package vcmock

import (
	"context"
	"io"
	"syscall"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
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

	// functions for mocks
	AnnotationsFunc          func(key string) (string, error)
	SetAnnotationsFunc       func(annotations map[string]string) error
	GetAnnotationsFunc       func() map[string]string
	GetNetNsFunc             func() string
	GetAllContainersFunc     func() []vc.VCContainer
	GetContainerFunc         func(containerID string) vc.VCContainer
	ReleaseFunc              func() error
	StartFunc                func() error
	StopFunc                 func(force bool) error
	PauseFunc                func() error
	ResumeFunc               func() error
	DeleteFunc               func() error
	CreateContainerFunc      func(conf vc.ContainerConfig) (vc.VCContainer, error)
	DeleteContainerFunc      func(contID string) (vc.VCContainer, error)
	StartContainerFunc       func(contID string) (vc.VCContainer, error)
	StopContainerFunc        func(contID string, force bool) (vc.VCContainer, error)
	KillContainerFunc        func(contID string, signal syscall.Signal, all bool) error
	StatusContainerFunc      func(contID string) (vc.ContainerStatus, error)
	StatsContainerFunc       func(contID string) (vc.ContainerStats, error)
	PauseContainerFunc       func(contID string) error
	ResumeContainerFunc      func(contID string) error
	StatusFunc               func() vc.SandboxStatus
	EnterContainerFunc       func(containerID string, cmd types.Cmd) (vc.VCContainer, *vc.Process, error)
	MonitorFunc              func() (chan error, error)
	UpdateContainerFunc      func(containerID string, resources specs.LinuxResources) error
	ProcessListContainerFunc func(containerID string, options vc.ProcessListOptions) (vc.ProcessList, error)
	WaitProcessFunc          func(containerID, processID string) (int32, error)
	SignalProcessFunc        func(containerID, processID string, signal syscall.Signal, all bool) error
	WinsizeProcessFunc       func(containerID, processID string, height, width uint32) error
	IOStreamFunc             func(containerID, processID string) (io.WriteCloser, io.Reader, io.Reader, error)
	AddDeviceFunc            func(info config.DeviceInfo) (api.Device, error)
	AddInterfaceFunc         func(inf *vcTypes.Interface) (*vcTypes.Interface, error)
	RemoveInterfaceFunc      func(inf *vcTypes.Interface) (*vcTypes.Interface, error)
	ListInterfacesFunc       func() ([]*vcTypes.Interface, error)
	UpdateRoutesFunc         func(routes []*vcTypes.Route) ([]*vcTypes.Route, error)
	ListRoutesFunc           func() ([]*vcTypes.Route, error)
	UpdateRuntimeMetricsFunc func() error
	GetAgentMetricsFunc      func() (string, error)
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
	RunSandboxFunc     func(ctx context.Context, sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error)
	StartSandboxFunc   func(ctx context.Context, sandboxID string) (vc.VCSandbox, error)
	StatusSandboxFunc  func(ctx context.Context, sandboxID string) (vc.SandboxStatus, error)
	StatsContainerFunc func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStats, error)
	StatsSandboxFunc   func(ctx context.Context, sandboxID string) (vc.SandboxStats, []vc.ContainerStats, error)
	StopSandboxFunc    func(ctx context.Context, sandboxID string, force bool) (vc.VCSandbox, error)

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

	AddInterfaceFunc     func(ctx context.Context, sandboxID string, inf *vcTypes.Interface) (*vcTypes.Interface, error)
	RemoveInterfaceFunc  func(ctx context.Context, sandboxID string, inf *vcTypes.Interface) (*vcTypes.Interface, error)
	ListInterfacesFunc   func(ctx context.Context, sandboxID string) ([]*vcTypes.Interface, error)
	UpdateRoutesFunc     func(ctx context.Context, sandboxID string, routes []*vcTypes.Route) ([]*vcTypes.Route, error)
	ListRoutesFunc       func(ctx context.Context, sandboxID string) ([]*vcTypes.Route, error)
	CleanupContainerFunc func(ctx context.Context, sandboxID, containerID string, force bool) error
}
