// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package cgroups

import (
	"bufio"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"

	"github.com/kata-containers/runtime/virtcontainers/pkg/rootless"
	libcontcgroups "github.com/opencontainers/runc/libcontainer/cgroups"
	libcontcgroupsfs "github.com/opencontainers/runc/libcontainer/cgroups/fs"
	libcontcgroupssystemd "github.com/opencontainers/runc/libcontainer/cgroups/systemd"
	"github.com/opencontainers/runc/libcontainer/configs"
	"github.com/opencontainers/runc/libcontainer/specconv"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
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
	// If set to true, expects cgroupsPath to be of form "slice:prefix:name", otherwise cgroups creation will fail
	systemdCgroup *bool

	cgroupsLogger = logrus.WithField("source", "virtcontainers/pkg/cgroups")
)

func EnableSystemdCgroup() {
	systemd := true
	systemdCgroup = &systemd
}

func UseSystemdCgroup() bool {
	if systemdCgroup != nil {
		return *systemdCgroup
	}
	return false
}

// returns the list of devices that a hypervisor may need
func hypervisorDevices() []specs.LinuxDeviceCgroup {
	wildcard := int64(-1)
	devicemapperMajor := int64(253)

	devices := []specs.LinuxDeviceCgroup{}

	devices = append(devices,
		// hypervisor needs access to all devicemapper devices,
		// since they can be hotplugged in the VM.
		specs.LinuxDeviceCgroup{
			Allow:  true,
			Type:   "b",
			Major:  &devicemapperMajor,
			Minor:  &wildcard,
			Access: "rwm",
		})

	// Processes running in a device-cgroup are constrained, they have acccess
	// only to the devices listed in the devices.list file.
	// In order to run Virtual Machines and create virtqueues, hypervisors
	// need access to certain character devices in the host, like kvm and vhost-net.
	hypervisorDevices := []string{
		"/dev/kvm",       // To run virtual machines
		"/dev/vhost-net", // To create virtqueues
	}

	for _, device := range hypervisorDevices {
		var st unix.Stat_t
		linuxDevice := specs.LinuxDeviceCgroup{
			Allow:  true,
			Access: "rwm",
		}

		if err := unix.Stat(device, &st); err != nil {
			cgroupsLogger.WithError(err).WithField("device", device).Warn("Could not get device information")
			continue
		}

		switch st.Mode & unix.S_IFMT {
		case unix.S_IFCHR:
			linuxDevice.Type = "c"
		case unix.S_IFBLK:
			linuxDevice.Type = "b"
		}

		major := int64(unix.Major(st.Rdev))
		minor := int64(unix.Minor(st.Rdev))
		linuxDevice.Major = &major
		linuxDevice.Minor = &minor

		devices = append(devices, linuxDevice)
	}

	return devices
}

// New creates a new CgroupManager
func New(config *Config) (*Manager, error) {
	var err error
	useSystemdCgroup := UseSystemdCgroup()

	devices := []specs.LinuxDeviceCgroup{}
	copy(devices, config.Resources.Devices)
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
		return fmt.Errorf("Could not move processes into parent cgroup: %v", err)
	}

	m.Lock()
	defer m.Unlock()
	return m.mgr.Destroy()
}
