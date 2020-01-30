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
	"regexp"
	"strings"

	"github.com/containerd/cgroups"
	"github.com/kata-containers/runtime/virtcontainers/pkg/rootless"
	libcontcgroups "github.com/opencontainers/runc/libcontainer/cgroups"
	libcontcgroupsfs "github.com/opencontainers/runc/libcontainer/cgroups/fs"
	libcontcgroupssystemd "github.com/opencontainers/runc/libcontainer/cgroups/systemd"
	"github.com/opencontainers/runc/libcontainer/configs"
	specconv "github.com/opencontainers/runc/libcontainer/specconv"
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

// prepend a kata specific string to oci cgroup path to
// form a different cgroup path, thus cAdvisor couldn't
// find kata containers cgroup path on host to prevent it
// from grabbing the stats data.
const cgroupKataPrefix = "kata"

// DefaultCgroupPath runtime-determined location in the cgroups hierarchy.
const defaultCgroupPath = "/vc"

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
		cgroups.NewCputset(root),
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

func renameCgroupPath(path string) (string, error) {
	if path == "" {
		return "", fmt.Errorf("Cgroup path is empty")
	}

	cgroupPathDir := filepath.Dir(path)
	cgroupPathName := fmt.Sprintf("%s_%s", cgroupKataPrefix, filepath.Base(path))
	return filepath.Join(cgroupPathDir, cgroupPathName), nil

}

// validCgroupPath returns a valid cgroup path.
// see https://github.com/opencontainers/runtime-spec/blob/master/config-linux.md#cgroups-path
func validCgroupPath(path string, systemdCgroup bool) (string, error) {
	if isSystemdCgroup(path) {
		return path, nil
	}

	if systemdCgroup {
		return "", fmt.Errorf("malformed systemd path '%v': expected to be of form 'slice:prefix:name'", path)
	}

	// In the case of an absolute path (starting with /), the runtime MUST
	// take the path to be relative to the cgroups mount point.
	if filepath.IsAbs(path) {
		return renameCgroupPath(filepath.Clean(path))
	}

	// In the case of a relative path (not starting with /), the runtime MAY
	// interpret the path relative to a runtime-determined location in the cgroups hierarchy.
	// clean up path and return a new path relative to defaultCgroupPath
	return renameCgroupPath(filepath.Join(defaultCgroupPath, filepath.Clean("/"+path)))
}

func isSystemdCgroup(cgroupPath string) bool {
	// systemd cgroup path: slice:prefix:name
	re := regexp.MustCompile(`([[:alnum:]]|\.)+:([[:alnum:]]|\.)+:([[:alnum:]]|\.)+`)
	found := re.FindStringIndex(cgroupPath)

	// if found string is equal to cgroupPath then
	// it's a correct systemd cgroup path.
	return found != nil && cgroupPath[found[0]:found[1]] == cgroupPath
}

func newCgroupManager(cgroups *configs.Cgroup, cgroupPaths map[string]string, spec *specs.Spec) (libcontcgroups.Manager, error) {
	var err error

	rootless := rootless.IsRootless()
	systemdCgroup := isSystemdCgroup(spec.Linux.CgroupsPath)

	// Create a new cgroup if the current one is nil
	// this cgroups must be saved later
	if cgroups == nil {
		if cgroups, err = specconv.CreateCgroupConfig(&specconv.CreateOpts{
			// cgroup name is taken from spec
			CgroupName:       "",
			UseSystemdCgroup: systemdCgroup,
			Spec:             spec,
			RootlessCgroups:  rootless,
		}); err != nil {
			return nil, fmt.Errorf("Could not create cgroup config: %v", err)
		}
	}

	// Set cgroupPaths to nil when the map is empty, it can and will be
	// populated by `Manager.Apply()` when the runtime or any other process
	// is moved to the cgroup. See sandbox.setupSandboxCgroup().
	if len(cgroupPaths) == 0 {
		cgroupPaths = nil
	}

	if systemdCgroup {
		systemdCgroupFunc, err := libcontcgroupssystemd.NewSystemdCgroupsManager()
		if err != nil {
			return nil, fmt.Errorf("Could not create systemd cgroup manager: %v", err)
		}
		libcontcgroupssystemd.UseSystemd()
		return systemdCgroupFunc(cgroups, cgroupPaths), nil
	}

	return &libcontcgroupsfs.Manager{
		Cgroups:  cgroups,
		Rootless: rootless,
		Paths:    cgroupPaths,
	}, nil
}
