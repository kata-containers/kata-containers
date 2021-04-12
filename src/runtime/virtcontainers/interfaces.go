// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"io"
	"syscall"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

// VC is the Virtcontainers interface
type VC interface {
	SetLogger(ctx context.Context, logger *logrus.Entry)
	SetFactory(ctx context.Context, factory Factory)

	CreateSandbox(ctx context.Context, sandboxConfig SandboxConfig) (VCSandbox, error)
	CleanupContainer(ctx context.Context, sandboxID, containerID string, force bool) error
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

	Stats(ctx context.Context) (SandboxStats, error)

	Start(ctx context.Context) error
	Stop(ctx context.Context, force bool) error
	Release(ctx context.Context) error
	Monitor(ctx context.Context) (chan error, error)
	Delete(ctx context.Context) error
	Status() SandboxStatus
	CreateContainer(ctx context.Context, contConfig ContainerConfig) (VCContainer, error)
	DeleteContainer(ctx context.Context, containerID string) (VCContainer, error)
	StartContainer(ctx context.Context, containerID string) (VCContainer, error)
	StopContainer(ctx context.Context, containerID string, force bool) (VCContainer, error)
	KillContainer(ctx context.Context, containerID string, signal syscall.Signal, all bool) error
	StatusContainer(containerID string) (ContainerStatus, error)
	StatsContainer(ctx context.Context, containerID string) (ContainerStats, error)
	PauseContainer(ctx context.Context, containerID string) error
	ResumeContainer(ctx context.Context, containerID string) error
	EnterContainer(ctx context.Context, containerID string, cmd types.Cmd) (VCContainer, *Process, error)
	UpdateContainer(ctx context.Context, containerID string, resources specs.LinuxResources) error
	WaitProcess(ctx context.Context, containerID, processID string) (int32, error)
	SignalProcess(ctx context.Context, containerID, processID string, signal syscall.Signal, all bool) error
	WinsizeProcess(ctx context.Context, containerID, processID string, height, width uint32) error
	IOStream(containerID, processID string) (io.WriteCloser, io.Reader, io.Reader, error)

	AddDevice(ctx context.Context, info config.DeviceInfo) (api.Device, error)

	AddInterface(ctx context.Context, inf *pbTypes.Interface) (*pbTypes.Interface, error)
	RemoveInterface(ctx context.Context, inf *pbTypes.Interface) (*pbTypes.Interface, error)
	ListInterfaces(ctx context.Context) ([]*pbTypes.Interface, error)
	UpdateRoutes(ctx context.Context, routes []*pbTypes.Route) ([]*pbTypes.Route, error)
	ListRoutes(ctx context.Context) ([]*pbTypes.Route, error)

	GetOOMEvent(ctx context.Context) (string, error)
	GetHypervisorPid() (int, error)

	UpdateRuntimeMetrics() error
	GetAgentMetrics(ctx context.Context) (string, error)
	GetAgentURL() (string, error)
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
}
