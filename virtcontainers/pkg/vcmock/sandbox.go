// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package vcmock

import (
	"syscall"

	vc "github.com/kata-containers/runtime/virtcontainers"
)

// ID implements the VCSandbox function of the same name.
func (p *Sandbox) ID() string {
	return p.MockID
}

// Annotations implements the VCSandbox function of the same name.
func (p *Sandbox) Annotations(key string) (string, error) {
	return p.MockAnnotations[key], nil
}

// SetAnnotations implements the VCSandbox function of the same name.
func (p *Sandbox) SetAnnotations(annotations map[string]string) error {
	return nil
}

// GetAnnotations implements the VCSandbox function of the same name.
func (p *Sandbox) GetAnnotations() map[string]string {
	return p.MockAnnotations
}

// GetAllContainers implements the VCSandbox function of the same name.
func (p *Sandbox) GetAllContainers() []vc.VCContainer {
	var ifa = make([]vc.VCContainer, len(p.MockContainers))

	for i, v := range p.MockContainers {
		ifa[i] = v
	}

	return ifa
}

// GetContainer implements the VCSandbox function of the same name.
func (p *Sandbox) GetContainer(containerID string) vc.VCContainer {
	for _, c := range p.MockContainers {
		if c.MockID == containerID {
			return c
		}
	}
	return &Container{}
}

// Release implements the VCSandbox function of the same name.
func (p *Sandbox) Release() error {
	return nil
}

// Pause implements the VCSandbox function of the same name.
func (p *Sandbox) Pause() error {
	return nil
}

// Resume implements the VCSandbox function of the same name.
func (p *Sandbox) Resume() error {
	return nil
}

// Delete implements the VCSandbox function of the same name.
func (p *Sandbox) Delete() error {
	return nil
}

// CreateContainer implements the VCSandbox function of the same name.
func (p *Sandbox) CreateContainer(conf vc.ContainerConfig) (vc.VCContainer, error) {
	return &Container{}, nil
}

// DeleteContainer implements the VCSandbox function of the same name.
func (p *Sandbox) DeleteContainer(contID string) (vc.VCContainer, error) {
	return &Container{}, nil
}

// StartContainer implements the VCSandbox function of the same name.
func (p *Sandbox) StartContainer(contID string) (vc.VCContainer, error) {
	return &Container{}, nil
}

// StatusContainer implements the VCSandbox function of the same name.
func (p *Sandbox) StatusContainer(contID string) (vc.ContainerStatus, error) {
	return vc.ContainerStatus{}, nil
}

// Status implements the VCSandbox function of the same name.
func (p *Sandbox) Status() vc.SandboxStatus {
	return vc.SandboxStatus{}
}

// EnterContainer implements the VCSandbox function of the same name.
func (p *Sandbox) EnterContainer(containerID string, cmd vc.Cmd) (vc.VCContainer, *vc.Process, error) {
	return &Container{}, &vc.Process{}, nil
}

// Monitor implements the VCSandbox function of the same name.
func (p *Sandbox) Monitor() (chan error, error) {
	return nil, nil
}

// WaitProcess implements the VCSandbox function of the same name.
func (p *Sandbox) WaitProcess(containerID, processID string) (int32, error) {
	return 0, nil
}

// SignalProcess implements the VCSandbox function of the same name.
func (p *Sandbox) SignalProcess(containerID, processID string, signal syscall.Signal, all bool) error {
	return nil
}
