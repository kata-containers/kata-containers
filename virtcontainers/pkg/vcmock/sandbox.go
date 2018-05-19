// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package vcmock

import (
	"io"
	"syscall"

	vc "github.com/kata-containers/runtime/virtcontainers"
	specs "github.com/opencontainers/runtime-spec/specs-go"
)

// ID implements the VCSandbox function of the same name.
func (s *Sandbox) ID() string {
	return s.MockID
}

// Annotations implements the VCSandbox function of the same name.
func (s *Sandbox) Annotations(key string) (string, error) {
	return s.MockAnnotations[key], nil
}

// SetAnnotations implements the VCSandbox function of the same name.
func (s *Sandbox) SetAnnotations(annotations map[string]string) error {
	return nil
}

// GetAnnotations implements the VCSandbox function of the same name.
func (s *Sandbox) GetAnnotations() map[string]string {
	return s.MockAnnotations
}

// GetAllContainers implements the VCSandbox function of the same name.
func (s *Sandbox) GetAllContainers() []vc.VCContainer {
	var ifa = make([]vc.VCContainer, len(s.MockContainers))

	for i, v := range s.MockContainers {
		ifa[i] = v
	}

	return ifa
}

// GetContainer implements the VCSandbox function of the same name.
func (s *Sandbox) GetContainer(containerID string) vc.VCContainer {
	for _, c := range s.MockContainers {
		if c.MockID == containerID {
			return c
		}
	}
	return &Container{}
}

// Release implements the VCSandbox function of the same name.
func (s *Sandbox) Release() error {
	return nil
}

// Pause implements the VCSandbox function of the same name.
func (s *Sandbox) Pause() error {
	return nil
}

// Resume implements the VCSandbox function of the same name.
func (s *Sandbox) Resume() error {
	return nil
}

// Delete implements the VCSandbox function of the same name.
func (s *Sandbox) Delete() error {
	return nil
}

// CreateContainer implements the VCSandbox function of the same name.
func (s *Sandbox) CreateContainer(conf vc.ContainerConfig) (vc.VCContainer, error) {
	return &Container{}, nil
}

// DeleteContainer implements the VCSandbox function of the same name.
func (s *Sandbox) DeleteContainer(contID string) (vc.VCContainer, error) {
	return &Container{}, nil
}

// StartContainer implements the VCSandbox function of the same name.
func (s *Sandbox) StartContainer(contID string) (vc.VCContainer, error) {
	return &Container{}, nil
}

// StatusContainer implements the VCSandbox function of the same name.
func (s *Sandbox) StatusContainer(contID string) (vc.ContainerStatus, error) {
	return vc.ContainerStatus{}, nil
}

// StatsContainer implements the VCSandbox function of the same name.
func (s *Sandbox) StatsContainer(contID string) (vc.ContainerStats, error) {
	return vc.ContainerStats{}, nil
}

// Status implements the VCSandbox function of the same name.
func (s *Sandbox) Status() vc.SandboxStatus {
	return vc.SandboxStatus{}
}

// EnterContainer implements the VCSandbox function of the same name.
func (s *Sandbox) EnterContainer(containerID string, cmd vc.Cmd) (vc.VCContainer, *vc.Process, error) {
	return &Container{}, &vc.Process{}, nil
}

// Monitor implements the VCSandbox function of the same name.
func (s *Sandbox) Monitor() (chan error, error) {
	return nil, nil
}

// UpdateContainer implements the VCSandbox function of the same name.
func (s *Sandbox) UpdateContainer(containerID string, resources specs.LinuxResources) error {
	return nil
}

// WaitProcess implements the VCSandbox function of the same name.
func (s *Sandbox) WaitProcess(containerID, processID string) (int32, error) {
	return 0, nil
}

// SignalProcess implements the VCSandbox function of the same name.
func (s *Sandbox) SignalProcess(containerID, processID string, signal syscall.Signal, all bool) error {
	return nil
}

// WinsizeProcess implements the VCSandbox function of the same name.
func (s *Sandbox) WinsizeProcess(containerID, processID string, height, width uint32) error {
	return nil
}

// IOStream implements the VCSandbox function of the same name.
func (s *Sandbox) IOStream(containerID, processID string) (io.WriteCloser, io.Reader, io.Reader, error) {
	return nil, nil, nil, nil
}
