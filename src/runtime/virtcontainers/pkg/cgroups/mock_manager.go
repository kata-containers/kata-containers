// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package cgroups

import (
	libcontcgroups "github.com/opencontainers/runc/libcontainer/cgroups"
	"github.com/opencontainers/runc/libcontainer/configs"
	"github.com/opencontainers/runc/libcontainer/devices"
)

// mockManager is an empty github.com/opencontainers/runc/libcontainer/cgroups
// Manager implementation, for testing and mocking purposes.
type mockManager struct {
}

// nolint:golint
func NewMockManager() libcontcgroups.Manager {
	return &mockManager{}
}

// Apply creates a cgroup, if not yet created, and adds a process
// with the specified pid into that cgroup.  A special value of -1
// can be used to merely create a cgroup.
func (n *mockManager) Apply(pid int) error {
	return nil
}

// GetPids returns the PIDs of all processes inside the cgroup.
func (n *mockManager) GetPids() ([]int, error) {
	return nil, nil
}

// GetAllPids returns the PIDs of all processes inside the cgroup
// any all its sub-cgroups.
func (n *mockManager) GetAllPids() ([]int, error) {
	return nil, nil
}

// GetStats returns cgroups statistics.
func (n *mockManager) GetStats() (*libcontcgroups.Stats, error) {
	return nil, nil
}

// Freeze sets the freezer cgroup to the specified state.
func (n *mockManager) Freeze(state configs.FreezerState) error {
	return nil
}

// Destroy removes cgroup.
func (n *mockManager) Destroy() error {
	return nil
}

// Path returns a cgroup path to the specified controller/subsystem.
// For cgroupv2, the argument is unused and can be empty.
func (n *mockManager) Path(string) string {
	return ""
}

// Set sets cgroup resources parameters/limits. If the argument is nil,
// the resources specified during Manager creation (or the previous call
// to Set) are used.
func (n *mockManager) Set(r *configs.Resources) error {
	return nil
}

// GetPaths returns cgroup path(s) to save in a state file in order to
// restore later.
func (n *mockManager) GetPaths() map[string]string {
	return nil
}

// GetCgroups returns the cgroup data as configured.
func (n *mockManager) GetCgroups() (*configs.Cgroup, error) {
	devPath := "/dev/null"
	dev, err := DeviceToCgroupDeviceRule(devPath)
	if err != nil {
		dev = &devices.Rule{}
	}
	return &configs.Cgroup{
		Name:        "mock",
		Path:        "",
		ScopePrefix: "",
		Paths:       nil,
		Resources: &configs.Resources{
			Devices: []*devices.Rule{dev},
		},
	}, nil
}

// GetFreezerState retrieves the current FreezerState of the cgroup.
func (n *mockManager) GetFreezerState() (configs.FreezerState, error) {
	return configs.Undefined, nil
}

// Exists returns whether the cgroup path exists or not.
func (n *mockManager) Exists() bool {
	return false
}

// OOMKillCount reports OOM kill count for the cgroup.
func (n *mockManager) OOMKillCount() (uint64, error) {
	return 0, nil
}
