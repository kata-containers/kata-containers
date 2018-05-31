// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"io"
	"syscall"

	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

// VC is the Virtcontainers interface
type VC interface {
	SetLogger(logger logrus.FieldLogger)

	CreateSandbox(sandboxConfig SandboxConfig) (VCSandbox, error)
	DeleteSandbox(sandboxID string) (VCSandbox, error)
	FetchSandbox(sandboxID string) (VCSandbox, error)
	ListSandbox() ([]SandboxStatus, error)
	PauseSandbox(sandboxID string) (VCSandbox, error)
	ResumeSandbox(sandboxID string) (VCSandbox, error)
	RunSandbox(sandboxConfig SandboxConfig) (VCSandbox, error)
	StartSandbox(sandboxID string) (VCSandbox, error)
	StatusSandbox(sandboxID string) (SandboxStatus, error)
	StopSandbox(sandboxID string) (VCSandbox, error)

	CreateContainer(sandboxID string, containerConfig ContainerConfig) (VCSandbox, VCContainer, error)
	DeleteContainer(sandboxID, containerID string) (VCContainer, error)
	EnterContainer(sandboxID, containerID string, cmd Cmd) (VCSandbox, VCContainer, *Process, error)
	KillContainer(sandboxID, containerID string, signal syscall.Signal, all bool) error
	StartContainer(sandboxID, containerID string) (VCContainer, error)
	StatusContainer(sandboxID, containerID string) (ContainerStatus, error)
	StatsContainer(sandboxID, containerID string) (ContainerStats, error)
	StopContainer(sandboxID, containerID string) (VCContainer, error)
	ProcessListContainer(sandboxID, containerID string, options ProcessListOptions) (ProcessList, error)
	UpdateContainer(sandboxID, containerID string, resources specs.LinuxResources) error
	PauseContainer(sandboxID, containerID string) error
	ResumeContainer(sandboxID, containerID string) error
}

// VCSandbox is the Sandbox interface
// (required since virtcontainers.Sandbox only contains private fields)
type VCSandbox interface {
	Annotations(key string) (string, error)
	GetAllContainers() []VCContainer
	GetAnnotations() map[string]string
	GetContainer(containerID string) VCContainer
	ID() string
	SetAnnotations(annotations map[string]string) error

	Pause() error
	Resume() error
	Release() error
	Monitor() (chan error, error)
	Delete() error
	Status() SandboxStatus
	CreateContainer(contConfig ContainerConfig) (VCContainer, error)
	DeleteContainer(contID string) (VCContainer, error)
	StartContainer(containerID string) (VCContainer, error)
	StatusContainer(containerID string) (ContainerStatus, error)
	StatsContainer(containerID string) (ContainerStats, error)
	EnterContainer(containerID string, cmd Cmd) (VCContainer, *Process, error)
	UpdateContainer(containerID string, resources specs.LinuxResources) error
	WaitProcess(containerID, processID string) (int32, error)
	SignalProcess(containerID, processID string, signal syscall.Signal, all bool) error
	WinsizeProcess(containerID, processID string, height, width uint32) error
	IOStream(containerID, processID string) (io.WriteCloser, io.Reader, io.Reader, error)
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
