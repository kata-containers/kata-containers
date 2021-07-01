// Copyright (c) 2018 Huawei Corporation
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/containerd/cgroups"
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

// V1Constraints returns the cgroups that are compatible with the VC architecture
// and hypervisor, constraints can be applied to these cgroups.
func V1Constraints() ([]cgroups.Subsystem, error) {
	root, err := cgroupV1MountPoint()
	if err != nil {
		return nil, err
	}
	subsystems := []cgroups.Subsystem{
		cgroups.NewCpuset(root),
		cgroups.NewCpu(root),
		cgroups.NewCpuacct(root),
	}
	return cgroupsSubsystems(subsystems)
}

// V1NoConstraints returns the cgroups that are *not* compatible with the VC
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
