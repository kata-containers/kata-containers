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

	Stats() (SandboxStats, error)

	Start() error
	Stop(force bool) error
	Release() error
	Monitor() (chan error, error)
	Delete() error
	Status() SandboxStatus
	CreateContainer(contConfig ContainerConfig) (VCContainer, error)
	DeleteContainer(contID string) (VCContainer, error)
	StartContainer(containerID string) (VCContainer, error)
	StopContainer(containerID string, force bool) (VCContainer, error)
	KillContainer(containerID string, signal syscall.Signal, all bool) error
	StatusContainer(containerID string) (ContainerStatus, error)
	StatsContainer(containerID string) (ContainerStats, error)
	PauseContainer(containerID string) error
	ResumeContainer(containerID string) error
	EnterContainer(containerID string, cmd types.Cmd) (VCContainer, *Process, error)
	UpdateContainer(containerID string, resources specs.LinuxResources) error
	ProcessListContainer(containerID string, options ProcessListOptions) (ProcessList, error)
	WaitProcess(containerID, processID string) (int32, error)
	SignalProcess(containerID, processID string, signal syscall.Signal, all bool) error
	WinsizeProcess(containerID, processID string, height, width uint32) error
	IOStream(containerID, processID string) (io.WriteCloser, io.Reader, io.Reader, error)

	AddDevice(info config.DeviceInfo) (api.Device, error)

	AddInterface(inf *pbTypes.Interface) (*pbTypes.Interface, error)
	RemoveInterface(inf *pbTypes.Interface) (*pbTypes.Interface, error)
	ListInterfaces() ([]*pbTypes.Interface, error)
	UpdateRoutes(routes []*pbTypes.Route) ([]*pbTypes.Route, error)
	ListRoutes() ([]*pbTypes.Route, error)

	GetOOMEvent() (string, error)

	UpdateRuntimeMetrics() error
	GetAgentMetrics() (string, error)
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
