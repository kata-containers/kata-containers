// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/containerd/cgroups"
	cgroupsv2 "github.com/containerd/cgroups/v2"
	systemdDbus "github.com/coreos/go-systemd/v22/dbus"
	"github.com/godbus/dbus/v5"
	runc_cgroups "github.com/opencontainers/runc/libcontainer/cgroups"
	"github.com/opencontainers/runc/libcontainer/cgroups/systemd"
	"golang.org/x/sys/unix"
)

// cgroup v2 mount point
const UnifiedMountpoint = "/sys/fs/cgroup"

// DefaultResourceControllerID runtime-determined location in the cgroups hierarchy.
const DefaultResourceControllerID = "/vc"

// CgroupMode is the cgroup mode in cgroup v2.
type CgroupMode string

const (
	CgroupModeDomain         CgroupMode = "domain"
	CgroupModeDomainThreaded CgroupMode = "domain threaded"
	CgroupModeDomainInvalid  CgroupMode = "domain invalid"
	CgroupModeThreaded       CgroupMode = "threaded"
)

// ValidCgroupPath returns a valid cgroup path.
// see https://github.com/opencontainers/runtime-spec/blob/master/config-linux.md#cgroups-path
func ValidCgroupPath(path string, isCgroupV2 bool, systemdCgroup bool) (string, error) {
	if IsSystemdCgroup(path) {
		if isCgroupV2 {
			return filepath.Join("/", path), nil
		} else {
			return path, nil
		}
	}

	if systemdCgroup {
		return "", fmt.Errorf("malformed systemd path '%v': expected to be of form 'slice:prefix:name'", path)
	}

	// In the case of an absolute path (starting with /), the runtime MUST
	// take the path to be relative to the cgroups mount point.
	if filepath.IsAbs(path) {
		return filepath.Clean(path), nil
	}

	// In the case of a relative path (not starting with /), the runtime MAY
	// interpret the path relative to a runtime-determined location in the cgroups hierarchy.
	// clean up path and return a new path relative to DefaultResourceControllerID
	return filepath.Join(DefaultResourceControllerID, filepath.Clean("/"+path)), nil
}

func newProperty(name string, units interface{}) systemdDbus.Property {
	return systemdDbus.Property{
		Name:  name,
		Value: dbus.MakeVariant(units),
	}
}

func cgroupHierarchy(path string, sandboxCgroupOnly bool) (cgroups.Hierarchy, cgroups.Path, error) {
	if !IsSystemdCgroup(path) || !sandboxCgroupOnly {
		return cgroups.V1, cgroups.StaticPath(path), nil
	} else {
		slice, unit, err := getSliceAndUnit(path)
		if err != nil {
			return nil, nil, err
		}

		cgroupSlicePath, _ := systemd.ExpandSlice(slice)
		if err != nil {
			return nil, nil, err
		}

		return cgroups.Systemd, cgroups.Slice(cgroupSlicePath, unit), nil
	}
}

func createCgroupsSystemd(slice string, unit string, pid int) error {
	ctx := context.TODO()
	conn, err := systemdDbus.NewWithContext(ctx)
	if err != nil {
		return err
	}
	defer conn.Close()

	properties := []systemdDbus.Property{
		systemdDbus.PropDescription("cgroup " + unit),
		newProperty("DefaultDependencies", false),
		newProperty("MemoryAccounting", true),
		newProperty("CPUAccounting", true),
		newProperty("IOAccounting", true),
	}

	if strings.HasSuffix(unit, ".slice") {
		// If we create a slice, the parent is defined via a Wants=.
		properties = append(properties, systemdDbus.PropWants(slice))
	} else {
		// Otherwise it's a scope, which we put into a Slice=.
		properties = append(properties, systemdDbus.PropSlice(slice))
	}

	// Assume scopes always support delegation (supported since systemd v218).
	properties = append(properties, newProperty("Delegate", true))

	if pid != -1 {
		properties = append(properties, systemdDbus.PropPids(uint32(pid)))
	}

	ch := make(chan string)
	// https://www.freedesktop.org/wiki/Software/systemd/ControlGroupInterface/
	_, err = conn.StartTransientUnitContext(ctx, unit, "replace", properties, ch)
	if err != nil {
		return err
	}
	<-ch
	return nil
}

func getSliceAndUnit(cgroupPath string) (string, string, error) {
	parts := strings.Split(cgroupPath, ":")
	if len(parts) == 3 && strings.HasSuffix(parts[0], ".slice") {
		return parts[0], fmt.Sprintf("%s-%s.scope", parts[1], parts[2]), nil
	}

	return "", "", fmt.Errorf("Path: %s is not valid systemd's cgroups path", cgroupPath)
}

func IsCgroupV1() (bool, error) {
	if cgroups.Mode() == cgroups.Legacy || cgroups.Mode() == cgroups.Hybrid {
		return true, nil
	} else if cgroups.Mode() == cgroups.Unified {
		return false, nil
	} else {
		return false, ErrCgroupMode
	}
}

// SandboxAndOverheadPath gets the cgroup path in thread mode in cgroup v2.
// The sandbox and overhead cgroup in threaded mode are placed under the same cgroup in domain threaded mode.
// In this way, vCPU threads and VMM processes can be separated into two cgroups.
// For details, please refer to https://github.com/kata-containers/kata-containers/issues/4886
// and host cgroups design document https://github.com/kata-containers/kata-containers/blob/main/docs/design/host-cgroups.md.
func SandboxAndOverheadPath(sandboxPath string, overheadPath string, sandboxCgroupOnly bool) (string, string, error) {
	isCgroupV1, err := IsCgroupV1()
	if err != nil {
		return "", "", err
	}

	sandboxThreadedPath := sandboxPath
	overheadThreadedPath := overheadPath

	// For cgroup v2, when sandboxCgroupOnly = false, need to use threaded mode for management.
	if !isCgroupV1 && !sandboxCgroupOnly {
		sandboxThreadedPath = filepath.Join(sandboxPath, "sandbox")
		overheadThreadedPath = filepath.Join(sandboxPath, "overhead")
	}

	return sandboxThreadedPath, overheadThreadedPath, nil
}

func SetThreadedMode(path string) error {
	if err := runc_cgroups.WriteFile(filepath.Join(UnifiedMountpoint, path), "cgroup.type", string(CgroupModeThreaded)); err != nil {
		return err
	}

	return nil
}

func GetThreadedMode(path string) (string, error) {
	cgroupType, err := runc_cgroups.ReadFile(filepath.Join(UnifiedMountpoint, path), "cgroup.type")
	if err != nil {
		return "", err
	}
	cgroupType = strings.Replace(cgroupType, "\n", "", -1)

	return cgroupType, nil
}

// AllowAddThread determine the cgroup mode that allows adding threads.
func AllowAddThread(cgroupType string) bool {
	if cgroupType == string(CgroupModeDomainThreaded) ||
		cgroupType == string(CgroupModeThreaded) {
		return true
	} else {
		return false
	}
}

func moveTo(manager *cgroupsv2.Manager, destination *cgroupsv2.Manager) error {
	var lastError error
	maxRetries := 5
	delay := 10 * time.Millisecond
	for i := 0; i < maxRetries; i++ {
		// Sleep for a short duration before retrying
		if i != 0 {
			time.Sleep(delay)
			delay *= 2
		}
		processes, err := manager.Procs(false)
		if err != nil {
			return err
		}
		if len(processes) == 0 {
			return nil
		}

		for _, p := range processes {
			if err := destination.AddProc(p); err != nil {
				if strings.Contains(err.Error(), "no such process") {
					continue
				}
				lastError = err
			}
		}
	}

	return fmt.Errorf("cgroups: unable to move all processes after %d retries. Last error: %v", maxRetries, lastError)
}

func deleteCgroup(manager *cgroupsv2.Manager, path string) error {
	// kernel prevents cgroups with running process from being removed, check the tree is empty
	processes, err := manager.Procs(false)
	if err != nil {
		return err
	}
	if len(processes) > 0 {
		return fmt.Errorf("cgroups: unable to remove path %q: still contains running processes %v", path, processes)
	}

	return remove(path)
}

// remove will remove a cgroup path handling EAGAIN and EBUSY errors and
// retrying the remove after a exp timeout
func remove(path string) error {
	var err error
	maxRetries := 5
	delay := 10 * time.Millisecond
	for i := 0; i < maxRetries; i++ {
		if i != 0 {
			time.Sleep(delay)
			delay *= 2
		}
		if err = os.RemoveAll(path); err == nil {
			return nil
		}
	}
	return fmt.Errorf("cgroups: unable to remove path %q: %w", path, err)
}

func SetThreadAffinity(threadID int, cpuSetSlice []int) error {
	unixCPUSet := unix.CPUSet{}

	for _, cpuId := range cpuSetSlice {
		unixCPUSet.Set(cpuId)
	}

	if err := unix.SchedSetaffinity(threadID, &unixCPUSet); err != nil {
		return fmt.Errorf("failed to set vcpu thread %d affinity to cpu %d: %v", threadID, cpuSetSlice, err)
	}

	return nil
}
