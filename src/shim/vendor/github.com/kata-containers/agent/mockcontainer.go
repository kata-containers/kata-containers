//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"

	"github.com/opencontainers/runc/libcontainer"
	"github.com/opencontainers/runc/libcontainer/configs"
	"github.com/opencontainers/runtime-spec/specs-go"
)

type mockContainer struct {
	id        string
	status    libcontainer.Status
	stats     libcontainer.Stats
	processes []int
}

func (m *mockContainer) ID() string {
	return m.id
}

func (m *mockContainer) Status() (libcontainer.Status, error) {
	return m.status, nil
}

func (m *mockContainer) State() (*libcontainer.State, error) {
	return nil, nil
}

func (m *mockContainer) OCIState() (*specs.State, error) {
	return nil, nil
}

func (m *mockContainer) Config() configs.Config {
	return configs.Config{
		Capabilities: &configs.Capabilities{},
		Cgroups: &configs.Cgroup{
			Resources: &configs.Resources{},
			Path:      fmt.Sprintf("/cgroup/%s", m.id),
		},
		Seccomp: &configs.Seccomp{},
	}
}

func (m *mockContainer) Processes() ([]int, error) {
	return m.processes, nil
}

func (m *mockContainer) Stats() (*libcontainer.Stats, error) {
	return &m.stats, nil
}

func (m *mockContainer) Set(config configs.Config) error {
	return nil
}

func (m *mockContainer) Start(process *libcontainer.Process) (err error) {
	return nil
}

func (m *mockContainer) Run(process *libcontainer.Process) (err error) {
	return nil
}

func (m *mockContainer) Destroy() error {
	return nil
}

func (m *mockContainer) Signal(s os.Signal, all bool) error {
	return nil
}

func (m *mockContainer) Exec() error {
	return nil
}

func (m *mockContainer) Checkpoint(criuOpts *libcontainer.CriuOpts) error {
	return nil
}

func (m *mockContainer) Restore(process *libcontainer.Process, criuOpts *libcontainer.CriuOpts) error {
	return nil
}

func (m *mockContainer) Pause() error {
	return nil
}

func (m *mockContainer) Resume() error {
	return nil
}

func (m *mockContainer) NotifyOOM() (<-chan struct{}, error) {
	return nil, nil
}

func (m *mockContainer) NotifyMemoryPressure(level libcontainer.PressureLevel) (<-chan struct{}, error) {
	return nil, nil
}
