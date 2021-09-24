// Copyright (c) 2021 Inspur Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package cgroups

import (
	"github.com/containerd/cgroups"
	v1 "github.com/containerd/cgroups/stats/v1"
	"github.com/opencontainers/runtime-spec/specs-go"
)

// mockCgroup is an empty github.com/containerd/cgroups Cgroup implementation, for testing and mocking purposes.
type mockCgroup struct {
	subsystems []cgroups.Subsystem
}

// New returns a new sub cgroup
func (c *mockCgroup) New(name string, resources *specs.LinuxResources) (cgroups.Cgroup, error) {
	return &mockCgroup{}, nil
}

// Subsystems returns all the subsystems that are currently being
// consumed by the group
func (c *mockCgroup) Subsystems() []cgroups.Subsystem {
	return c.subsystems
}

// Add moves the provided process into the new cgroup
func (c *mockCgroup) Add(process cgroups.Process) error {
	return nil
}

// AddProc moves the provided process id into the new cgroup
func (c *mockCgroup) AddProc(pid uint64) error {
	return nil
}

// AddTask moves the provided tasks (threads) into the new cgroup
func (c *mockCgroup) AddTask(process cgroups.Process) error {
	return nil
}

// Delete will remove the control group from each of the subsystems registered
func (c *mockCgroup) Delete() error {
	return nil
}

// Stat returns the current metrics for the cgroup
func (c *mockCgroup) Stat(handlers ...cgroups.ErrorHandler) (*v1.Metrics, error) {
	return nil, nil
}

// Update updates the cgroup with the new resource values provided
//
// Be prepared to handle EBUSY when trying to update a cgroup with
// live processes and other operations like Stats being performed at the
// same time
func (c *mockCgroup) Update(resources *specs.LinuxResources) error {
	return nil
}

// Processes returns the processes running inside the cgroup along
// with the subsystem used, pid, and path
func (c *mockCgroup) Processes(subsystem cgroups.Name, recursive bool) ([]cgroups.Process, error) {
	return nil, nil
}

// Tasks returns the tasks running inside the cgroup along
// with the subsystem used, pid, and path
func (c *mockCgroup) Tasks(subsystem cgroups.Name, recursive bool) ([]cgroups.Task, error) {
	return nil, nil
}

// Freeze freezes the entire cgroup and all the processes inside it
func (c *mockCgroup) Freeze() error {
	return nil
}

// Thaw thaws out the cgroup and all the processes inside it
func (c *mockCgroup) Thaw() error {
	return nil
}

// OOMEventFD returns the memory cgroup's out of memory event fd that triggers
// when processes inside the cgroup receive an oom event. Returns
// ErrMemoryNotSupported if memory cgroups is not supported.
func (c *mockCgroup) OOMEventFD() (uintptr, error) {
	return 0, nil
}

// RegisterMemoryEvent allows the ability to register for all v1 memory cgroups
// notifications.
func (c *mockCgroup) RegisterMemoryEvent(event cgroups.MemoryEvent) (uintptr, error) {
	return 0, nil
}

// State returns the state of the cgroup and its processes
func (c *mockCgroup) State() cgroups.State {
	return cgroups.Unknown
}

// MoveTo does a recursive move subsystem by subsystem of all the processes
// inside the group
func (c *mockCgroup) MoveTo(destination cgroups.Cgroup) error {
	return nil
}
