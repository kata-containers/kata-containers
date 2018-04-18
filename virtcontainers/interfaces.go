// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"syscall"

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
	StopContainer(sandboxID, containerID string) (VCContainer, error)
	ProcessListContainer(sandboxID, containerID string, options ProcessListOptions) (ProcessList, error)
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
	Delete() error
	CreateContainer(contConfig ContainerConfig) (VCContainer, error)
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
