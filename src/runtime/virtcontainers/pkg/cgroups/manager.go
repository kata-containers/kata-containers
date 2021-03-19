// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package cgroups

import (
	"bufio"
	"context"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	libcontcgroups "github.com/opencontainers/runc/libcontainer/cgroups"
	libcontcgroupsfs "github.com/opencontainers/runc/libcontainer/cgroups/fs"
	libcontcgroupssystemd "github.com/opencontainers/runc/libcontainer/cgroups/systemd"
	"github.com/opencontainers/runc/libcontainer/configs"
	"github.com/opencontainers/runc/libcontainer/specconv"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

type Config struct {
	// Cgroups specifies specific cgroup settings for the various subsystems that the container is
	// placed into to limit the resources the container has available
	// If nil, New() will create one.
	Cgroups *configs.Cgroup

	// CgroupPaths contains paths to all the cgroups setup for a container. Key is cgroup subsystem name
	// with the value as the path.
	CgroupPaths map[string]string

	// Resources represents the runtime resource constraints
	Resources specs.LinuxResources

	// CgroupPath is the OCI spec cgroup path
	CgroupPath string
}

type Manager struct {
	sync.Mutex
	mgr libcontcgroups.Manager
}

const (
	// file in the cgroup that contains the pids
	cgroupProcs = "cgroup.procs"
)

var (
	cgroupsLogger = logrus.WithField("source", "virtcontainers/pkg/cgroups")
)

// SetLogger sets up a logger for this pkg
func SetLogger(logger *logrus.Entry) {
	fields := cgroupsLogger.Data

	cgroupsLogger = logger.WithFields(fields)
}

// returns the list of devices that a hypervisor may need
func hypervisorDevices() []specs.LinuxDeviceCgroup {
	devices := []specs.LinuxDeviceCgroup{}

	// Processes running in a device-cgroup are constrained, they have acccess
	// only to the devices listed in the devices.list file.
	// In order to run Virtual Machines and create virtqueues, hypervisors
	// need access to certain character devices in the host, like kvm and vhost-net.
	hypervisorDevices := []string{
		"/dev/kvm",       // To run virtual machines
		"/dev/vhost-net", // To create virtqueues
		"/dev/vfio/vfio", // To access VFIO devices
	}

	for _, device := range hypervisorDevices {
		ldevice, err := DeviceToLinuxDevice(device)
		if err != nil {
			cgroupsLogger.WithError(err).Warnf("Could not get device information")
			continue
		}
		devices = append(devices, ldevice)
	}

	return devices
}

// New creates a new CgroupManager
func New(config *Config) (*Manager, error) {
	var err error

	devices := config.Resources.Devices
	devices = append(devices, hypervisorDevices()...)
	// Do not modify original devices
	config.Resources.Devices = devices

	newSpec := specs.Spec{
		Linux: &specs.Linux{
			Resources: &config.Resources,
		},
	}

	rootless := rootless.IsRootless()

	cgroups := config.Cgroups
	cgroupPaths := config.CgroupPaths

	// determine if we are utilizing systemd managed cgroups based on the path provided
	useSystemdCgroup := IsSystemdCgroup(config.CgroupPath)

	// Create a new cgroup if the current one is nil
	// this cgroups must be saved later
	if cgroups == nil {
		if config.CgroupPath == "" && !rootless {
			cgroupsLogger.Warn("cgroups have not been created and cgroup path is empty")
		}

		newSpec.Linux.CgroupsPath, err = ValidCgroupPath(config.CgroupPath, useSystemdCgroup)
		if err != nil {
			return nil, fmt.Errorf("Invalid cgroup path: %v", err)
		}

		if cgroups, err = specconv.CreateCgroupConfig(&specconv.CreateOpts{
			// cgroup name is taken from spec
			CgroupName:       "",
			UseSystemdCgroup: useSystemdCgroup,
			Spec:             &newSpec,
			RootlessCgroups:  rootless,
		}); err != nil {
			return nil, fmt.Errorf("Could not create cgroup config: %v", err)
		}
	}

	// Set cgroupPaths to nil when the map is empty, it can and will be
	// populated by `Manager.Apply()` when the runtime or any other process
	// is moved to the cgroup.
	if len(cgroupPaths) == 0 {
		cgroupPaths = nil
	}

	if useSystemdCgroup {
		systemdCgroupFunc, err := libcontcgroupssystemd.NewSystemdCgroupsManager()
		if err != nil {
			return nil, fmt.Errorf("Could not create systemd cgroup manager: %v", err)
		}
		libcontcgroupssystemd.UseSystemd()
		return &Manager{
			mgr: systemdCgroupFunc(cgroups, cgroupPaths),
		}, nil
	}

	return &Manager{
		mgr: &libcontcgroupsfs.Manager{
			Cgroups:  cgroups,
			Rootless: rootless,
			Paths:    cgroupPaths,
		},
	}, nil
}

// read all the pids in cgroupPath
func readPids(cgroupPath string) ([]int, error) {
	pids := []int{}
	f, err := os.Open(filepath.Join(cgroupPath, cgroupProcs))
	if err != nil {
		return nil, err
	}
	defer f.Close()
	buf := bufio.NewScanner(f)

	for buf.Scan() {
		if t := buf.Text(); t != "" {
			pid, err := strconv.Atoi(t)
			if err != nil {
				return nil, err
			}
			pids = append(pids, pid)
		}
	}
	return pids, nil
}

// write the pids into cgroup.procs
func writePids(pids []int, cgroupPath string) error {
	cgroupProcsPath := filepath.Join(cgroupPath, cgroupProcs)
	for _, pid := range pids {
		if err := ioutil.WriteFile(cgroupProcsPath,
			[]byte(strconv.Itoa(pid)),
			os.FileMode(0),
		); err != nil {
			return err
		}
	}
	return nil
}

func (m *Manager) logger() *logrus.Entry {
	return cgroupsLogger.WithField("source", "cgroup-manager")
}

// move all the processes in the current cgroup to the parent
func (m *Manager) moveToParent() error {
	m.Lock()
	defer m.Unlock()
	for _, cgroupPath := range m.mgr.GetPaths() {

		pids, err := readPids(cgroupPath)
		// possible that the cgroupPath doesn't exist. If so, skip:
		if os.IsNotExist(err) {
			// The cgroup is not present on the filesystem: no pids to move. The systemd cgroup
			// manager lists all of the subsystems, including those that are not actually being managed.
			continue
		}
		if err != nil {
			return err
		}

		if len(pids) == 0 {
			// no pids in this cgroup
			continue
		}

		cgroupParentPath := filepath.Dir(filepath.Clean(cgroupPath))
		if err = writePids(pids, cgroupParentPath); err != nil {
			if !strings.Contains(err.Error(), "no such process") {
				return err
			}
		}
	}
	return nil
}

// Add pid to cgroups
func (m *Manager) Add(pid int) error {
	if rootless.IsRootless() {
		m.logger().Debug("Unable to setup add pids to cgroup: running rootless")
		return nil
	}

	m.Lock()
	defer m.Unlock()
	return m.mgr.Apply(pid)
}

// Apply constraints
func (m *Manager) Apply() error {
	if rootless.IsRootless() {
		m.logger().Debug("Unable to apply constraints: running rootless")
		return nil
	}

	cgroups, err := m.GetCgroups()
	if err != nil {
		return err
	}

	m.Lock()
	defer m.Unlock()
	return m.mgr.Set(&configs.Config{
		Cgroups: cgroups,
	})
}

func (m *Manager) GetCgroups() (*configs.Cgroup, error) {
	m.Lock()
	defer m.Unlock()
	return m.mgr.GetCgroups()
}

func (m *Manager) GetPaths() map[string]string {
	m.Lock()
	defer m.Unlock()
	return m.mgr.GetPaths()
}

func (m *Manager) Destroy() error {
	// cgroup can't be destroyed if it contains running processes
	if err := m.moveToParent(); err != nil {
		// If the process migration to the parent cgroup fails, then
		// we expect the Destroy to fail as well. Let's log an error here
		// and attempt to execute the Destroy still to help cleanup the hosts' FS.
		m.logger().WithError(err).Error("Could not move processes into parent cgroup")
	}

	m.Lock()
	defer m.Unlock()
	return m.mgr.Destroy()
}

// AddDevice adds a device to the device cgroup
func (m *Manager) AddDevice(ctx context.Context, device string) error {
	cgroups, err := m.GetCgroups()
	if err != nil {
		return err
	}

	ld, err := DeviceToCgroupDevice(device)
	if err != nil {
		return err
	}

	m.Lock()
	cgroups.Devices = append(cgroups.Devices, ld)
	m.Unlock()

	return m.Apply()
}

// RemoceDevice removed a device from the device cgroup
func (m *Manager) RemoveDevice(device string) error {
	cgroups, err := m.GetCgroups()
	if err != nil {
		return err
	}

	m.Lock()
	for i, d := range cgroups.Devices {
		if d.Path == device {
			cgroups.Devices = append(cgroups.Devices[:i], cgroups.Devices[i+1:]...)
			m.Unlock()
			return m.Apply()
		}
	}
	m.Unlock()
	return fmt.Errorf("device %v not found in the cgroup", device)
}

func (m *Manager) SetCPUSet(cpuset, memset string) error {
	cgroups, err := m.GetCgroups()
	if err != nil {
		return err
	}

	m.Lock()
	cgroups.CpusetCpus = cpuset
	cgroups.CpusetMems = memset
	m.Unlock()

	return m.Apply()
}
