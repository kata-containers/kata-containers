// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"io"
	"syscall"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/pkg/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

// VC is the Virtcontainers interface
type VC interface {
	SetLogger(ctx context.Context, logger *logrus.Entry)
	SetFactory(ctx context.Context, factory Factory)

	CreateSandbox(ctx context.Context, sandboxConfig SandboxConfig) (VCSandbox, error)
	DeleteSandbox(ctx context.Context, sandboxID string) (VCSandbox, error)
	FetchSandbox(ctx context.Context, sandboxID string) (VCSandbox, error)
	ListSandbox(ctx context.Context) ([]SandboxStatus, error)
	PauseSandbox(ctx context.Context, sandboxID string) (VCSandbox, error)
	ResumeSandbox(ctx context.Context, sandboxID string) (VCSandbox, error)
	RunSandbox(ctx context.Context, sandboxConfig SandboxConfig) (VCSandbox, error)
	StartSandbox(ctx context.Context, sandboxID string) (VCSandbox, error)
	StatusSandbox(ctx context.Context, sandboxID string) (SandboxStatus, error)
	StopSandbox(ctx context.Context, sandboxID string) (VCSandbox, error)

	CreateContainer(ctx context.Context, sandboxID string, containerConfig ContainerConfig) (VCSandbox, VCContainer, error)
	DeleteContainer(ctx context.Context, sandboxID, containerID string) (VCContainer, error)
	EnterContainer(ctx context.Context, sandboxID, containerID string, cmd Cmd) (VCSandbox, VCContainer, *Process, error)
	KillContainer(ctx context.Context, sandboxID, containerID string, signal syscall.Signal, all bool) error
	StartContainer(ctx context.Context, sandboxID, containerID string) (VCContainer, error)
	StatusContainer(ctx context.Context, sandboxID, containerID string) (ContainerStatus, error)
	StatsContainer(ctx context.Context, sandboxID, containerID string) (ContainerStats, error)
	StopContainer(ctx context.Context, sandboxID, containerID string) (VCContainer, error)
	ProcessListContainer(ctx context.Context, sandboxID, containerID string, options ProcessListOptions) (ProcessList, error)
	UpdateContainer(ctx context.Context, sandboxID, containerID string, resources specs.LinuxResources) error
	PauseContainer(ctx context.Context, sandboxID, containerID string) error
	ResumeContainer(ctx context.Context, sandboxID, containerID string) error

	AddDevice(ctx context.Context, sandboxID string, info config.DeviceInfo) (api.Device, error)

	AddInterface(ctx context.Context, sandboxID string, inf *types.Interface) (*types.Interface, error)
	RemoveInterface(ctx context.Context, sandboxID string, inf *types.Interface) (*types.Interface, error)
	ListInterfaces(ctx context.Context, sandboxID string) ([]*types.Interface, error)
	UpdateRoutes(ctx context.Context, sandboxID string, routes []*types.Route) ([]*types.Route, error)
	ListRoutes(ctx context.Context, sandboxID string) ([]*types.Route, error)
}

// VCSandbox is the Sandbox interface
// (required since virtcontainers.Sandbox only contains private fields)
type VCSandbox interface {
	Annotations(key string) (string, error)
	GetNetNs() string
	GetAllContainers() []VCContainer
	GetAnnotations() map[string]string
	GetContainer(containerID string) VCContainer
	ID() string
	SetAnnotations(annotations map[string]string) error

	Start() error
	Stop() error
	Pause() error
	Resume() error
	Release() error
	Monitor() (chan error, error)
	Delete() error
	Status() SandboxStatus
	CreateContainer(contConfig ContainerConfig) (VCContainer, error)
	DeleteContainer(contID string) (VCContainer, error)
	StartContainer(containerID string) (VCContainer, error)
	StopContainer(containerID string) (VCContainer, error)
	StatusContainer(containerID string) (ContainerStatus, error)
	StatsContainer(containerID string) (ContainerStats, error)
	EnterContainer(containerID string, cmd Cmd) (VCContainer, *Process, error)
	UpdateContainer(containerID string, resources specs.LinuxResources) error
	WaitProcess(containerID, processID string) (int32, error)
	SignalProcess(containerID, processID string, signal syscall.Signal, all bool) error
	WinsizeProcess(containerID, processID string, height, width uint32) error
	IOStream(containerID, processID string) (io.WriteCloser, io.Reader, io.Reader, error)

	AddDevice(info config.DeviceInfo) (api.Device, error)

	AddInterface(inf *types.Interface) (*types.Interface, error)
	RemoveInterface(inf *types.Interface) (*types.Interface, error)
	ListInterfaces() ([]*types.Interface, error)
	UpdateRoutes(routes []*types.Route) ([]*types.Route, error)
	ListRoutes() ([]*types.Route, error)
}

// VCContainer is the Container interface
// (required since virtcontainers.Container only contains private fields)
type VCContainer interface {
	GetAnnotations() map[string]string
	GetPid() int
	GetToken() string
	ID() string
	Sandbox() VCSandbox
	Process() Process
	SetPid(pid int) error
}
