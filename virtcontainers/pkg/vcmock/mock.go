// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Description: A mock implementation of virtcontainers that can be used
// for testing.
//
// This implementation calls the function set in the object that
// corresponds to the name of the method (for example, when CreateSandbox()
// is called, that method will try to call CreateSandboxFunc). If no
// function is defined for the method, it will return an error in a
// well-known format. Callers can detect this scenario by calling
// IsMockError().

package vcmock

import (
	"fmt"
	"syscall"

	vc "github.com/kata-containers/runtime/virtcontainers"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

// mockErrorPrefix is a string that all errors returned by the mock
// implementation itself will contain as a prefix.
const mockErrorPrefix = "vcmock forced failure"

// SetLogger implements the VC function of the same name.
func (m *VCMock) SetLogger(logger logrus.FieldLogger) {
	if m.SetLoggerFunc != nil {
		m.SetLoggerFunc(logger)
	}
}

// CreateSandbox implements the VC function of the same name.
func (m *VCMock) CreateSandbox(sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
	if m.CreateSandboxFunc != nil {
		return m.CreateSandboxFunc(sandboxConfig)
	}

	return nil, fmt.Errorf("%s: %s (%+v): sandboxConfig: %v", mockErrorPrefix, getSelf(), m, sandboxConfig)
}

// DeleteSandbox implements the VC function of the same name.
func (m *VCMock) DeleteSandbox(sandboxID string) (vc.VCSandbox, error) {
	if m.DeleteSandboxFunc != nil {
		return m.DeleteSandboxFunc(sandboxID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v", mockErrorPrefix, getSelf(), m, sandboxID)
}

// FetchSandbox implements the VC function of the same name.
func (m *VCMock) FetchSandbox(sandboxID string) (vc.VCSandbox, error) {
	if m.FetchSandboxFunc != nil {
		return m.FetchSandboxFunc(sandboxID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v", mockErrorPrefix, getSelf(), m, sandboxID)
}

// StartSandbox implements the VC function of the same name.
func (m *VCMock) StartSandbox(sandboxID string) (vc.VCSandbox, error) {
	if m.StartSandboxFunc != nil {
		return m.StartSandboxFunc(sandboxID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v", mockErrorPrefix, getSelf(), m, sandboxID)
}

// StopSandbox implements the VC function of the same name.
func (m *VCMock) StopSandbox(sandboxID string) (vc.VCSandbox, error) {
	if m.StopSandboxFunc != nil {
		return m.StopSandboxFunc(sandboxID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v", mockErrorPrefix, getSelf(), m, sandboxID)
}

// RunSandbox implements the VC function of the same name.
func (m *VCMock) RunSandbox(sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
	if m.RunSandboxFunc != nil {
		return m.RunSandboxFunc(sandboxConfig)
	}

	return nil, fmt.Errorf("%s: %s (%+v): sandboxConfig: %v", mockErrorPrefix, getSelf(), m, sandboxConfig)
}

// ListSandbox implements the VC function of the same name.
func (m *VCMock) ListSandbox() ([]vc.SandboxStatus, error) {
	if m.ListSandboxFunc != nil {
		return m.ListSandboxFunc()
	}

	return nil, fmt.Errorf("%s: %s", mockErrorPrefix, getSelf())
}

// StatusSandbox implements the VC function of the same name.
func (m *VCMock) StatusSandbox(sandboxID string) (vc.SandboxStatus, error) {
	if m.StatusSandboxFunc != nil {
		return m.StatusSandboxFunc(sandboxID)
	}

	return vc.SandboxStatus{}, fmt.Errorf("%s: %s (%+v): sandboxID: %v", mockErrorPrefix, getSelf(), m, sandboxID)
}

// PauseSandbox implements the VC function of the same name.
func (m *VCMock) PauseSandbox(sandboxID string) (vc.VCSandbox, error) {
	if m.PauseSandboxFunc != nil {
		return m.PauseSandboxFunc(sandboxID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v", mockErrorPrefix, getSelf(), m, sandboxID)
}

// ResumeSandbox implements the VC function of the same name.
func (m *VCMock) ResumeSandbox(sandboxID string) (vc.VCSandbox, error) {
	if m.ResumeSandboxFunc != nil {
		return m.ResumeSandboxFunc(sandboxID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v", mockErrorPrefix, getSelf(), m, sandboxID)
}

// CreateContainer implements the VC function of the same name.
func (m *VCMock) CreateContainer(sandboxID string, containerConfig vc.ContainerConfig) (vc.VCSandbox, vc.VCContainer, error) {
	if m.CreateContainerFunc != nil {
		return m.CreateContainerFunc(sandboxID, containerConfig)
	}

	return nil, nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerConfig: %v", mockErrorPrefix, getSelf(), m, sandboxID, containerConfig)
}

// DeleteContainer implements the VC function of the same name.
func (m *VCMock) DeleteContainer(sandboxID, containerID string) (vc.VCContainer, error) {
	if m.DeleteContainerFunc != nil {
		return m.DeleteContainerFunc(sandboxID, containerID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, sandboxID, containerID)
}

// StartContainer implements the VC function of the same name.
func (m *VCMock) StartContainer(sandboxID, containerID string) (vc.VCContainer, error) {
	if m.StartContainerFunc != nil {
		return m.StartContainerFunc(sandboxID, containerID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, sandboxID, containerID)
}

// StopContainer implements the VC function of the same name.
func (m *VCMock) StopContainer(sandboxID, containerID string) (vc.VCContainer, error) {
	if m.StopContainerFunc != nil {
		return m.StopContainerFunc(sandboxID, containerID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, sandboxID, containerID)
}

// EnterContainer implements the VC function of the same name.
func (m *VCMock) EnterContainer(sandboxID, containerID string, cmd vc.Cmd) (vc.VCSandbox, vc.VCContainer, *vc.Process, error) {
	if m.EnterContainerFunc != nil {
		return m.EnterContainerFunc(sandboxID, containerID, cmd)
	}

	return nil, nil, nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerID: %v, cmd: %v", mockErrorPrefix, getSelf(), m, sandboxID, containerID, cmd)
}

// StatusContainer implements the VC function of the same name.
func (m *VCMock) StatusContainer(sandboxID, containerID string) (vc.ContainerStatus, error) {
	if m.StatusContainerFunc != nil {
		return m.StatusContainerFunc(sandboxID, containerID)
	}

	return vc.ContainerStatus{}, fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, sandboxID, containerID)
}

// StatsContainer implements the VC function of the same name.
func (m *VCMock) StatsContainer(sandboxID, containerID string) (vc.ContainerStats, error) {
	if m.StatsContainerFunc != nil {
		return m.StatsContainerFunc(sandboxID, containerID)
	}

	return vc.ContainerStats{}, fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, sandboxID, containerID)
}

// KillContainer implements the VC function of the same name.
func (m *VCMock) KillContainer(sandboxID, containerID string, signal syscall.Signal, all bool) error {
	if m.KillContainerFunc != nil {
		return m.KillContainerFunc(sandboxID, containerID, signal, all)
	}

	return fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerID: %v, signal: %v, all: %v", mockErrorPrefix, getSelf(), m, sandboxID, containerID, signal, all)
}

// ProcessListContainer implements the VC function of the same name.
func (m *VCMock) ProcessListContainer(sandboxID, containerID string, options vc.ProcessListOptions) (vc.ProcessList, error) {
	if m.ProcessListContainerFunc != nil {
		return m.ProcessListContainerFunc(sandboxID, containerID, options)
	}

	return nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, sandboxID, containerID)
}

// UpdateContainer implements the VC function of the same name.
func (m *VCMock) UpdateContainer(sandboxID, containerID string, resources specs.LinuxResources) error {
	if m.UpdateContainerFunc != nil {
		return m.UpdateContainerFunc(sandboxID, containerID, resources)
	}

	return fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, sandboxID, containerID)
}

// PauseContainer implements the VC function of the same name.
func (m *VCMock) PauseContainer(sandboxID, containerID string) error {
	if m.PauseContainerFunc != nil {
		return m.PauseContainerFunc(sandboxID, containerID)
	}

	return fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, sandboxID, containerID)
}

// ResumeContainer implements the VC function of the same name.
func (m *VCMock) ResumeContainer(sandboxID, containerID string) error {
	if m.ResumeContainerFunc != nil {
		return m.ResumeContainerFunc(sandboxID, containerID)
	}

	return fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, sandboxID, containerID)
}
