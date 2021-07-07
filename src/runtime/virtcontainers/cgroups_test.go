// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	"github.com/containerd/cgroups"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

type mockCgroup struct {
}

func (m *mockCgroup) New(string, *specs.LinuxResources) (cgroups.Cgroup, error) {
	return &mockCgroup{}, nil
}
func (m *mockCgroup) Add(cgroups.Process) error {
	return nil
}

func (m *mockCgroup) AddTask(cgroups.Process) error {
	return nil
}

func (m *mockCgroup) Delete() error {
	return nil
}

func (m *mockCgroup) MoveTo(cgroups.Cgroup) error {
	return nil
}

func (m *mockCgroup) Stat(...cgroups.ErrorHandler) (*cgroups.Metrics, error) {
	return &cgroups.Metrics{}, nil
}

func (m *mockCgroup) Update(resources *specs.LinuxResources) error {
	return nil
}

func (m *mockCgroup) Processes(cgroups.Name, bool) ([]cgroups.Process, error) {
	return nil, nil
}

func (m *mockCgroup) Freeze() error {
	return nil
}

func (m *mockCgroup) Thaw() error {
	return nil
}

func (m *mockCgroup) OOMEventFD() (uintptr, error) {
	return 0, nil
}

func (m *mockCgroup) State() cgroups.State {
	return ""
}

func (m *mockCgroup) Subsystems() []cgroups.Subsystem {
	return nil
}

func (m *mockCgroup) Tasks(cgroups.Name, bool) ([]cgroups.Task, error) {
	return nil, nil
}

func mockCgroupNew(hierarchy cgroups.Hierarchy, path cgroups.Path, resources *specs.LinuxResources, opts ...cgroups.InitOpts) (cgroups.Cgroup, error) {
	return &mockCgroup{}, nil
}

func mockCgroupLoad(hierarchy cgroups.Hierarchy, path cgroups.Path, opts ...cgroups.InitOpts) (cgroups.Cgroup, error) {
	return &mockCgroup{}, nil
}

func init() {
	cgroupsNewFunc = mockCgroupNew
	cgroupsLoadFunc = mockCgroupLoad
}

func TestV1Constraints(t *testing.T) {
	assert := assert.New(t)

	systems, err := V1Constraints()
	assert.NoError(err)
	assert.NotEmpty(systems)
}

func TestV1NoConstraints(t *testing.T) {
	assert := assert.New(t)

	systems, err := V1NoConstraints()
	assert.NoError(err)
	assert.NotEmpty(systems)
}

func TestCgroupNoConstraintsPath(t *testing.T) {
	assert := assert.New(t)

	cgrouPath := "abc"
	expectedPath := filepath.Join(cgroupKataPath, cgrouPath)
	path := cgroupNoConstraintsPath(cgrouPath)
	assert.Equal(expectedPath, path)
}

func TestUpdateCgroups(t *testing.T) {
	assert := assert.New(t)

	oldCgroupsNew := cgroupsNewFunc
	oldCgroupsLoad := cgroupsLoadFunc
	cgroupsNewFunc = cgroups.New
	cgroupsLoadFunc = cgroups.Load
	defer func() {
		cgroupsNewFunc = oldCgroupsNew
		cgroupsLoadFunc = oldCgroupsLoad
	}()

	s := &Sandbox{
		state: types.SandboxState{
			CgroupPath: "",
		},
		config: &SandboxConfig{SandboxCgroupOnly: false},
	}

	ctx := context.Background()

	// empty path
	err := s.cgroupsUpdate(ctx)
	assert.NoError(err)

	// path doesn't exist
	s.state.CgroupPath = "/abc/123/rgb"
	err = s.cgroupsUpdate(ctx)
	assert.Error(err)

	if os.Getuid() != 0 {
		return
	}

	s.state.CgroupPath = fmt.Sprintf("/kata-tests-%d", os.Getpid())
	testCgroup, err := cgroups.New(cgroups.V1, cgroups.StaticPath(s.state.CgroupPath), &specs.LinuxResources{})
	assert.NoError(err)
	defer testCgroup.Delete()
	s.hypervisor = &mockHypervisor{mockPid: 0}

	// bad pid
	err = s.cgroupsUpdate(ctx)
	assert.Error(err)

	// fake workload
	cmd := exec.Command("tail", "-f", "/dev/null")
	assert.NoError(cmd.Start())
	s.hypervisor = &mockHypervisor{mockPid: cmd.Process.Pid}

	// no containers
	err = s.cgroupsUpdate(ctx)
	assert.NoError(err)

	s.config = &SandboxConfig{}
	s.config.HypervisorConfig.NumVCPUs = 1

	s.containers = map[string]*Container{
		"abc": {
			process: Process{
				Pid: cmd.Process.Pid,
			},
			config: &ContainerConfig{
				Annotations: containerAnnotations,
				CustomSpec:  newEmptySpec(),
			},
		},
		"xyz": {
			process: Process{
				Pid: cmd.Process.Pid,
			},
			config: &ContainerConfig{
				Annotations: containerAnnotations,
				CustomSpec:  newEmptySpec(),
			},
		},
	}

	err = s.cgroupsUpdate(context.Background())
	assert.NoError(err)

	// cleanup
	assert.NoError(cmd.Process.Kill())
	err = s.cgroupsDelete()
	assert.NoError(err)
}
