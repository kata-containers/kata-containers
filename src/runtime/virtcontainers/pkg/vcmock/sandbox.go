// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package vcmock

import (
	"context"
	"fmt"
	"io"
	"syscall"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
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

// GetNetNs returns the network namespace of the current sandbox.
func (s *Sandbox) GetNetNs() string {
	return s.MockNetNs
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
func (s *Sandbox) Release(ctx context.Context) error {
	return nil
}

// Start implements the VCSandbox function of the same name.
func (s *Sandbox) Start(ctx context.Context) error {
	return nil
}

// Stop implements the VCSandbox function of the same name.
func (s *Sandbox) Stop(ctx context.Context, force bool) error {
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
func (s *Sandbox) Delete(ctx context.Context) error {
	return nil
}

// CreateContainer implements the VCSandbox function of the same name.
func (s *Sandbox) CreateContainer(ctx context.Context, conf vc.ContainerConfig) (vc.VCContainer, error) {
	if s.CreateContainerFunc != nil {
		return s.CreateContainerFunc(conf)
	}
	return nil, fmt.Errorf("%s: %s (%+v): sandboxID: %v, containerConfig: %v", mockErrorPrefix, getSelf(), s, s.MockID, conf)
}

// DeleteContainer implements the VCSandbox function of the same name.
func (s *Sandbox) DeleteContainer(ctx context.Context, contID string) (vc.VCContainer, error) {
	return &Container{}, nil
}

// StartContainer implements the VCSandbox function of the same name.
func (s *Sandbox) StartContainer(ctx context.Context, contID string) (vc.VCContainer, error) {
	return &Container{}, nil
}

// StopContainer implements the VCSandbox function of the same name.
func (s *Sandbox) StopContainer(ctx context.Context, contID string, force bool) (vc.VCContainer, error) {
	return &Container{}, nil
}

// KillContainer implements the VCSandbox function of the same name.
func (s *Sandbox) KillContainer(ctx context.Context, contID string, signal syscall.Signal, all bool) error {
	return nil
}

// StatusContainer implements the VCSandbox function of the same name.
func (s *Sandbox) StatusContainer(contID string) (vc.ContainerStatus, error) {
	return vc.ContainerStatus{}, nil
}

// StatsContainer implements the VCSandbox function of the same name.
func (s *Sandbox) StatsContainer(ctx context.Context, contID string) (vc.ContainerStats, error) {
	if s.StatsContainerFunc != nil {
		return s.StatsContainerFunc(contID)
	}
	return vc.ContainerStats{}, nil
}

// PauseContainer implements the VCSandbox function of the same name.
func (s *Sandbox) PauseContainer(ctx context.Context, contID string) error {
	return nil
}

// ResumeContainer implements the VCSandbox function of the same name.
func (s *Sandbox) ResumeContainer(ctx context.Context, contID string) error {
	return nil
}

// Status implements the VCSandbox function of the same name.
func (s *Sandbox) Status() vc.SandboxStatus {
	return vc.SandboxStatus{}
}

// EnterContainer implements the VCSandbox function of the same name.
func (s *Sandbox) EnterContainer(ctx context.Context, containerID string, cmd types.Cmd) (vc.VCContainer, *vc.Process, error) {
	return &Container{}, &vc.Process{}, nil
}

// Monitor implements the VCSandbox function of the same name.
func (s *Sandbox) Monitor(ctx context.Context) (chan error, error) {
	return nil, nil
}

// UpdateContainer implements the VCSandbox function of the same name.
func (s *Sandbox) UpdateContainer(ctx context.Context, containerID string, resources specs.LinuxResources) error {
	return nil
}

// WaitProcess implements the VCSandbox function of the same name.
func (s *Sandbox) WaitProcess(ctx context.Context, containerID, processID string) (int32, error) {
	return 0, nil
}

// SignalProcess implements the VCSandbox function of the same name.
func (s *Sandbox) SignalProcess(ctx context.Context, containerID, processID string, signal syscall.Signal, all bool) error {
	return nil
}

// WinsizeProcess implements the VCSandbox function of the same name.
func (s *Sandbox) WinsizeProcess(ctx context.Context, containerID, processID string, height, width uint32) error {
	return nil
}

// IOStream implements the VCSandbox function of the same name.
func (s *Sandbox) IOStream(containerID, processID string) (io.WriteCloser, io.Reader, io.Reader, error) {
	return nil, nil, nil, nil
}

// AddDevice adds a device to sandbox
func (s *Sandbox) AddDevice(ctx context.Context, info config.DeviceInfo) (api.Device, error) {
	return nil, nil
}

// AddInterface implements the VCSandbox function of the same name.
func (s *Sandbox) AddInterface(ctx context.Context, inf *pbTypes.Interface) (*pbTypes.Interface, error) {
	return nil, nil
}

// RemoveInterface implements the VCSandbox function of the same name.
func (s *Sandbox) RemoveInterface(ctx context.Context, inf *pbTypes.Interface) (*pbTypes.Interface, error) {
	return nil, nil
}

// ListInterfaces implements the VCSandbox function of the same name.
func (s *Sandbox) ListInterfaces(ctx context.Context) ([]*pbTypes.Interface, error) {
	return nil, nil
}

// UpdateRoutes implements the VCSandbox function of the same name.
func (s *Sandbox) UpdateRoutes(ctx context.Context, routes []*pbTypes.Route) ([]*pbTypes.Route, error) {
	return nil, nil
}

// ListRoutes implements the VCSandbox function of the same name.
func (s *Sandbox) ListRoutes(ctx context.Context) ([]*pbTypes.Route, error) {
	return nil, nil
}

func (s *Sandbox) GetOOMEvent(ctx context.Context) (string, error) {
	return "", nil
}

// UpdateRuntimeMetrics implements the VCSandbox function of the same name.
func (s *Sandbox) UpdateRuntimeMetrics() error {
	if s.UpdateRuntimeMetricsFunc != nil {
		return s.UpdateRuntimeMetricsFunc()
	}
	return nil
}

// GetAgentMetrics implements the VCSandbox function of the same name.
func (s *Sandbox) GetAgentMetrics(ctx context.Context) (string, error) {
	if s.GetAgentMetricsFunc != nil {
		return s.GetAgentMetricsFunc()
	}
	return "", nil
}

// Stats implements the VCSandbox function of the same name.
func (s *Sandbox) Stats(ctx context.Context) (vc.SandboxStats, error) {
	if s.StatsFunc != nil {
		return s.StatsFunc()
	}
	return vc.SandboxStats{}, nil
}

func (s *Sandbox) GetAgentURL() (string, error) {
	if s.GetAgentURLFunc != nil {
		return s.GetAgentURLFunc()
	}
	return "", nil
}

func (s *Sandbox) GetHypervisorPid() (int, error) {
	return 0, nil
}

func (s *Sandbox) GuestVolumeStats(ctx context.Context, path string) ([]byte, error) {
	return nil, nil
}
func (s *Sandbox) ResizeGuestVolume(ctx context.Context, path string, size uint64) error {
	return nil
}

func (s *Sandbox) GetIPTables(ctx context.Context, isIPv6 bool) ([]byte, error) {
	return nil, nil
}

func (s *Sandbox) SetIPTables(ctx context.Context, isIPv6 bool, data []byte) error {
	return nil
}

func (s *Sandbox) SetPolicy(ctx context.Context, policy string) error {
	return nil
}
