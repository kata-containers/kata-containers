// Copyright (c) 2018 Huawei Corporation
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"strings"

	"github.com/containerd/cgroups"
	"github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	specs "github.com/opencontainers/runtime-spec/specs-go"
)

type cgroupPather interface {
	cgroups.Subsystem
	Path(path string) string
}

// unconstrained cgroups are placed here.
// for example /sys/fs/cgroup/memory/kata/$CGPATH
// where path is defined by the containers manager
const cgroupKataPath = "/kata/"

var cgroupsLoadFunc = cgroups.Load
var cgroupsNewFunc = cgroups.New

// V1Constraints returns the cgroups that are compatible with th VC architecture
// and hypervisor, constraints can be applied to these cgroups.
func V1Constraints() ([]cgroups.Subsystem, error) {
	root, err := cgroupV1MountPoint()
	if err != nil {
		return nil, err
	}
	subsystems := []cgroups.Subsystem{
		cgroups.NewCputset(root),
		cgroups.NewCpu(root),
		cgroups.NewCpuacct(root),
	}
	return cgroupsSubsystems(subsystems)
}

// V1NoConstraints returns the cgroups that are *not* compatible with th VC
// architecture and hypervisor, constraints MUST NOT be applied to these cgroups.
func V1NoConstraints() ([]cgroups.Subsystem, error) {
	root, err := cgroupV1MountPoint()
	if err != nil {
		return nil, err
	}
	subsystems := []cgroups.Subsystem{
		// Some constainers managers, like k8s, take the control of cgroups.
		// k8s: the memory cgroup for the dns containers is small to place
		// a hypervisor there.
		cgroups.NewMemory(root),
	}
	return cgroupsSubsystems(subsystems)
}

func cgroupsSubsystems(subsystems []cgroups.Subsystem) ([]cgroups.Subsystem, error) {
	var enabled []cgroups.Subsystem
	for _, s := range cgroupPathers(subsystems) {
		// check and remove the default groups that do not exist
		if _, err := os.Lstat(s.Path("/")); err == nil {
			enabled = append(enabled, s)
		}
	}
	return enabled, nil
}

func cgroupPathers(subystems []cgroups.Subsystem) []cgroupPather {
	var out []cgroupPather
	for _, s := range subystems {
		if p, ok := s.(cgroupPather); ok {
			out = append(out, p)
		}
	}
	return out
}

// v1MountPoint returns the mount point where the cgroup
// mountpoints are mounted in a single hiearchy
func cgroupV1MountPoint() (string, error) {
	f, err := os.Open("/proc/self/mountinfo")
	if err != nil {
		return "", err
	}
	defer f.Close()
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		if err := scanner.Err(); err != nil {
			return "", err
		}
		var (
			text   = scanner.Text()
			fields = strings.Split(text, " ")
			// safe as mountinfo encodes mountpoints with spaces as \040.
			index               = strings.Index(text, " - ")
			postSeparatorFields = strings.Fields(text[index+3:])
			numPostFields       = len(postSeparatorFields)
		)
		// this is an error as we can't detect if the mount is for "cgroup"
		if numPostFields == 0 {
			return "", fmt.Errorf("Found no fields post '-' in %q", text)
		}
		if postSeparatorFields[0] == "cgroup" {
			// check that the mount is properly formated.
			if numPostFields < 3 {
				return "", fmt.Errorf("Error found less than 3 fields post '-' in %q", text)
			}
			return filepath.Dir(fields[4]), nil
		}
	}
	return "", cgroups.ErrMountPointNotExist
}

func cgroupNoConstraintsPath(path string) string {
	return filepath.Join(cgroupKataPath, path)
}

// return the parent cgroup for the given path
func parentCgroup(hierarchy cgroups.Hierarchy, path string) (cgroups.Cgroup, error) {
	// append '/' just in case CgroupsPath doesn't start with it
	parent := filepath.Dir("/" + path)

	parentCgroup, err := cgroupsLoadFunc(hierarchy,
		cgroups.StaticPath(parent))
	if err != nil {
		return nil, fmt.Errorf("Could not load parent cgroup %v: %v", parent, err)
	}

	return parentCgroup, nil
}

func (s *Sandbox) updateCgroups() error {
	if s.state.CgroupPath == "" {
		s.Logger().Warn("sandbox's cgroup won't be updated: cgroup path is empty")
		return nil
	}

	cgroup, err := cgroupsLoadFunc(V1Constraints, cgroups.StaticPath(s.state.CgroupPath))
	if err != nil {
		return fmt.Errorf("Could not load cgroup %v: %v", s.state.CgroupPath, err)
	}

	if err := s.constrainHypervisor(cgroup); err != nil {
		return err
	}

	if len(s.containers) <= 1 {
		// nothing to update
		return nil
	}

	resources, err := s.resources()
	if err != nil {
		return err
	}

	if err := cgroup.Update(&resources); err != nil {
		return fmt.Errorf("Could not update cgroup %v: %v", s.state.CgroupPath, err)
	}

	return nil
}

func (s *Sandbox) deleteCgroups() error {
	s.Logger().Debug("Deleting sandbox cgroup")

	path := cgroupNoConstraintsPath(s.state.CgroupPath)
	s.Logger().WithField("path", path).Debug("Deleting no constraints cgroup")
	noConstraintsCgroup, err := cgroupsLoadFunc(V1NoConstraints, cgroups.StaticPath(path))
	if err == cgroups.ErrCgroupDeleted {
		// cgroup already deleted
		return nil
	}

	if err != nil {
		return fmt.Errorf("Could not load cgroup without constraints %v: %v", path, err)
	}

	// move running process here, that way cgroup can be removed
	parent, err := parentCgroup(V1NoConstraints, path)
	if err != nil {
		// parent cgroup doesn't exist, that means there are no process running
		// and the no constraints cgroup was removed.
		s.Logger().WithError(err).Warn("Parent cgroup doesn't exist")
		return nil
	}

	if err := noConstraintsCgroup.MoveTo(parent); err != nil {
		// Don't fail, cgroup can be deleted
		s.Logger().WithError(err).Warn("Could not move process from no constraints to parent cgroup")
	}

	return noConstraintsCgroup.Delete()
}

func (s *Sandbox) constrainHypervisor(cgroup cgroups.Cgroup) error {
	pid := s.hypervisor.pid()
	if pid <= 0 {
		return fmt.Errorf("Invalid hypervisor PID: %d", pid)
	}

	// Move hypervisor into cgroups without constraints,
	// those cgroups are not yet supported.
	resources := &specs.LinuxResources{}
	path := cgroupNoConstraintsPath(s.state.CgroupPath)
	noConstraintsCgroup, err := cgroupsNewFunc(V1NoConstraints, cgroups.StaticPath(path), resources)
	if err != nil {
		return fmt.Errorf("Could not create cgroup %v: %v", path, err)
	}

	if err := noConstraintsCgroup.Add(cgroups.Process{Pid: pid}); err != nil {
		return fmt.Errorf("Could not add hypervisor PID %d to cgroup %v: %v", pid, path, err)
	}

	// when new container joins, new CPU could be hotplugged, so we
	// have to query fresh vcpu info from hypervisor for every time.
	tids, err := s.hypervisor.getThreadIDs()
	if err != nil {
		return fmt.Errorf("failed to get thread ids from hypervisor: %v", err)
	}
	if tids == nil || len(tids.vcpus) == 0 {
		// If there's no tid returned from the hypervisor, this is not
		// a bug. It simply means there is nothing to constrain, hence
		// let's return without any error from here.
		return nil
	}

	// We are about to move just the vcpus (threads) into cgroups with constraints.
	// Move whole hypervisor process whould be easier but the IO/network performance
	// whould be impacted.
	for _, i := range tids.vcpus {
		// In contrast, AddTask will write thread id to `tasks`
		// After this, vcpu threads are in "vcpu" sub-cgroup, other threads in
		// qemu will be left in parent cgroup untouched.
		if err := cgroup.AddTask(cgroups.Process{
			Pid: i,
		}); err != nil {
			return err
		}
	}

	return nil
}

func (s *Sandbox) resources() (specs.LinuxResources, error) {
	resources := specs.LinuxResources{
		CPU: s.cpuResources(),
	}

	return resources, nil
}

func (s *Sandbox) cpuResources() *specs.LinuxCPU {
	quota := int64(0)
	period := uint64(0)
	shares := uint64(0)
	realtimePeriod := uint64(0)
	realtimeRuntime := int64(0)

	cpu := &specs.LinuxCPU{
		Quota:           &quota,
		Period:          &period,
		Shares:          &shares,
		RealtimePeriod:  &realtimePeriod,
		RealtimeRuntime: &realtimeRuntime,
	}

	for _, c := range s.containers {
		ann := c.GetAnnotations()
		if ann[annotations.ContainerTypeKey] == string(PodSandbox) {
			// skip sandbox container
			continue
		}

		if c.config.Resources.CPU == nil {
			continue
		}

		if c.config.Resources.CPU.Shares != nil {
			shares = uint64(math.Max(float64(*c.config.Resources.CPU.Shares), float64(shares)))
		}

		if c.config.Resources.CPU.Quota != nil {
			quota += *c.config.Resources.CPU.Quota
		}

		if c.config.Resources.CPU.Period != nil {
			period = uint64(math.Max(float64(*c.config.Resources.CPU.Period), float64(period)))
		}

		if c.config.Resources.CPU.Cpus != "" {
			cpu.Cpus += c.config.Resources.CPU.Cpus + ","
		}

		if c.config.Resources.CPU.RealtimeRuntime != nil {
			realtimeRuntime += *c.config.Resources.CPU.RealtimeRuntime
		}

		if c.config.Resources.CPU.RealtimePeriod != nil {
			realtimePeriod += *c.config.Resources.CPU.RealtimePeriod
		}

		if c.config.Resources.CPU.Mems != "" {
			cpu.Mems += c.config.Resources.CPU.Mems + ","
		}
	}

	cpu.Cpus = strings.Trim(cpu.Cpus, " \n\t,")

	// use a default constraint for sandboxes without cpu constraints
	if period == uint64(0) && quota == int64(0) {
		// set a quota and period equal to the default number of vcpus
		quota = int64(s.config.HypervisorConfig.NumVCPUs) * 100000
		period = 100000
	}

	return validCPUResources(cpu)
}

// validCPUResources checks CPU resources coherency
func validCPUResources(cpuSpec *specs.LinuxCPU) *specs.LinuxCPU {
	if cpuSpec == nil {
		return nil
	}

	cpu := *cpuSpec
	if cpu.Period != nil && *cpu.Period < 1 {
		cpu.Period = nil
	}

	if cpu.Quota != nil && *cpu.Quota < 1 {
		cpu.Quota = nil
	}

	if cpu.Shares != nil && *cpu.Shares < 1 {
		cpu.Shares = nil
	}

	if cpu.RealtimePeriod != nil && *cpu.RealtimePeriod < 1 {
		cpu.RealtimePeriod = nil
	}

	if cpu.RealtimeRuntime != nil && *cpu.RealtimeRuntime < 1 {
		cpu.RealtimeRuntime = nil
	}

	return &cpu
}
