// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/containerd/cgroups"
	"github.com/kata-containers/runtime/virtcontainers/types"
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

	// empty path
	err := s.cgroupsUpdate()
	assert.NoError(err)

	// path doesn't exist
	s.state.CgroupPath = "/abc/123/rgb"
	err = s.cgroupsUpdate()
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
	err = s.cgroupsUpdate()
	assert.Error(err)

	// fake workload
	cmd := exec.Command("tail", "-f", "/dev/null")
	assert.NoError(cmd.Start())
	s.hypervisor = &mockHypervisor{mockPid: cmd.Process.Pid}

	// no containers
	err = s.cgroupsUpdate()
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

	err = s.cgroupsUpdate()
	assert.NoError(err)

	// cleanup
	assert.NoError(cmd.Process.Kill())
	err = s.cgroupsDelete()
	assert.NoError(err)
}

func TestIsSystemdCgroup(t *testing.T) {
	assert := assert.New(t)

	tests := []struct {
		path     string
		expected bool
	}{
		{"slice:kata:afhts2e5d4g5s", true},
		{"slice.system:kata:afhts2e5d4g5s", true},
		{"/kata/afhts2e5d4g5s", false},
		{"a:b:c:d", false},
		{":::", false},
		{"", false},
		{":", false},
		{"::", false},
		{":::", false},
		{"a:b", false},
		{"a:b:", false},
		{":a:b", false},
		{"@:@:@", false},
	}

	for _, t := range tests {
		assert.Equal(t.expected, isSystemdCgroup(t.path), "invalid systemd cgroup path: %v", t.path)
	}
}

func TestValidCgroupPath(t *testing.T) {
	assert := assert.New(t)

	for _, t := range []struct {
		path          string
		systemdCgroup bool
		error         bool
	}{
		// empty paths
		{"../../../", false, false},
		{"../", false, false},
		{".", false, false},
		{"../../../", false, false},
		{"./../", false, false},

		// valid no-systemd paths
		{"../../../foo", false, false},
		{"/../hi", false, false},
		{"/../hi/foo", false, false},
		{"o / m /../ g", false, false},

		// invalid systemd paths
		{"o / m /../ g", true, true},
		{"slice:kata", true, true},
		{"/kata/afhts2e5d4g5s", true, true},
		{"a:b:c:d", true, true},
		{":::", true, true},
		{"", true, true},
		{":", true, true},
		{"::", true, true},
		{":::", true, true},
		{"a:b", true, true},
		{"a:b:", true, true},
		{":a:b", true, true},
		{"@:@:@", true, true},

		// valid system paths
		{"slice:kata:55555", true, false},
		{"slice.system:kata:afhts2e5d4g5s", true, false},
	} {
		path, err := validCgroupPath(t.path, t.systemdCgroup)
		if t.error {
			assert.Error(err)
			continue
		} else {
			assert.NoError(err)
		}

		if filepath.IsAbs(t.path) {
			cleanPath := filepath.Dir(filepath.Clean(t.path))
			assert.True(strings.HasPrefix(path, cleanPath),
				"%v should have prefix %v", cleanPath)
		} else if t.systemdCgroup {
			assert.Equal(t.path, path)
		} else {
			assert.True(strings.HasPrefix(path, "/"+cgroupKataPrefix) ||
				strings.HasPrefix(path, defaultCgroupPath),
				"%v should have prefix /%v or %v", path, cgroupKataPrefix, defaultCgroupPath)
		}
	}

}
