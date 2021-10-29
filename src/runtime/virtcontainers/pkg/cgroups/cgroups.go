// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package cgroups

import (
	"path/filepath"
	"sync"

	"github.com/containerd/cgroups"
	v1 "github.com/containerd/cgroups/stats/v1"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

type Cgroup struct {
	cgroup  cgroups.Cgroup
	path    string
	cpusets *specs.LinuxCPU
	devices []specs.LinuxDeviceCgroup

	sync.Mutex
}

var (
	cgroupsLogger = logrus.WithField("source", "virtcontainers/pkg/cgroups")
)

// SetLogger sets up a logger for this pkg
func SetLogger(logger *logrus.Entry) {
	fields := cgroupsLogger.Data

	cgroupsLogger = logger.WithFields(fields)
}

func sandboxDevices() []specs.LinuxDeviceCgroup {
	devices := []specs.LinuxDeviceCgroup{}

	defaultDevices := []string{
		"/dev/null",
		"/dev/random",
		"/dev/full",
		"/dev/tty",
		"/dev/zero",
		"/dev/urandom",
		"/dev/console",
	}

	// Processes running in a device-cgroup are constrained, they have acccess
	// only to the devices listed in the devices.list file.
	// In order to run Virtual Machines and create virtqueues, hypervisors
	// need access to certain character devices in the host, like kvm and vhost-net.
	hypervisorDevices := []string{
		"/dev/kvm",       // To run virtual machines
		"/dev/vhost-net", // To create virtqueues
		"/dev/vfio/vfio", // To access VFIO devices
	}

	defaultDevices = append(defaultDevices, hypervisorDevices...)

	for _, device := range defaultDevices {
		ldevice, err := DeviceToLinuxDevice(device)
		if err != nil {
			cgroupsLogger.WithField("source", "cgroups").Warnf("Could not add %s to the devices cgroup", device)
			continue
		}
		devices = append(devices, ldevice)
	}

	wildcardMajor := int64(-1)
	wildcardMinor := int64(-1)
	ptsMajor := int64(136)
	tunMajor := int64(10)
	tunMinor := int64(200)

	wildcardDevices := []specs.LinuxDeviceCgroup{
		// allow mknod for any device
		{
			Allow:  true,
			Type:   "c",
			Major:  &wildcardMajor,
			Minor:  &wildcardMinor,
			Access: "m",
		},
		{
			Allow:  true,
			Type:   "b",
			Major:  &wildcardMajor,
			Minor:  &wildcardMinor,
			Access: "m",
		},
		// /dev/pts/ - pts namespaces are "coming soon"
		{
			Allow:  true,
			Type:   "c",
			Major:  &ptsMajor,
			Minor:  &wildcardMinor,
			Access: "rwm",
		},
		// tuntap
		{
			Allow:  true,
			Type:   "c",
			Major:  &tunMajor,
			Minor:  &tunMinor,
			Access: "rwm",
		},
	}

	devices = append(devices, wildcardDevices...)

	return devices
}

func NewCgroup(path string, resources *specs.LinuxResources) (*Cgroup, error) {
	var err error
	var cgroup cgroups.Cgroup

	cgroupPath, err := ValidCgroupPath(path, IsSystemdCgroup(path))
	if err != nil {
		return nil, err
	}

	if !IsSystemdCgroup(path) {
		cgroup, err = cgroups.New(cgroups.V1, cgroups.StaticPath(cgroupPath), resources)
	} else {
		cgroup, err = cgroups.New(cgroups.Systemd, cgroups.Slice(getSliceName(cgroupPath), getUnitName(cgroupPath)), resources)
	}
	if err != nil {
		return nil, err
	}

	return &Cgroup{
		path:    cgroupPath,
		devices: resources.Devices,
		cpusets: resources.CPU,
		cgroup:  cgroup,
	}, nil
}

func NewSandboxCgroup(path string, resources *specs.LinuxResources) (*Cgroup, error) {
	sandboxResources := *resources
	sandboxResources.Devices = append(sandboxResources.Devices, sandboxDevices()...)

	return NewCgroup(path, &sandboxResources)
}

func Load(path string) (*Cgroup, error) {
	var err error
	var cgroup cgroups.Cgroup

	if !IsSystemdCgroup(path) {
		cgroup, err = cgroups.Load(cgroups.V1, cgroups.StaticPath(path))
	} else {
		cgroup, err = cgroups.Load(cgroups.Systemd, cgroups.Slice(getSliceName(path), getUnitName(path)))
	}
	if err != nil {
		return nil, err
	}

	return &Cgroup{
		path:   path,
		cgroup: cgroup,
	}, nil
}

func (c *Cgroup) Logger() *logrus.Entry {
	return cgroupsLogger.WithField("source", "cgroups")
}

func (c *Cgroup) Delete() error {
	return c.cgroup.Delete()
}

func (c *Cgroup) Stat() (*v1.Metrics, error) {
	return c.cgroup.Stat(cgroups.ErrorHandler(cgroups.IgnoreNotExist))
}

func (c *Cgroup) AddProcess(pid int, subsystems ...string) error {
	return c.cgroup.Add(cgroups.Process{Pid: pid})
}

func (c *Cgroup) AddTask(pid int, subsystems ...string) error {
	return c.cgroup.AddTask(cgroups.Process{Pid: pid})
}

func (c *Cgroup) Update(resources *specs.LinuxResources) error {
	return c.cgroup.Update(resources)
}

func (c *Cgroup) MoveTo(path string) error {
	var err error
	var newCgroup cgroups.Cgroup

	if !IsSystemdCgroup(path) {
		newCgroup, err = cgroups.Load(cgroups.V1, cgroups.StaticPath(path))
	} else {
		newCgroup, err = cgroups.Load(cgroups.Systemd, cgroups.Slice(getSliceName(path), getUnitName(path)))
	}
	if err != nil {
		return err
	}

	return c.cgroup.MoveTo(newCgroup)
}

func (c *Cgroup) MoveToParent() error {
	parentPath := filepath.Dir(c.path)

	return c.MoveTo(parentPath)
}

func (c *Cgroup) AddDevice(deviceHostPath string) error {
	deviceResource, err := DeviceToLinuxDevice(deviceHostPath)
	if err != nil {
		return err
	}

	c.Lock()
	defer c.Unlock()

	c.devices = append(c.devices, deviceResource)

	if err := c.cgroup.Update(&specs.LinuxResources{
		Devices: c.devices,
	}); err != nil {
		return err
	}

	return nil
}

func (c *Cgroup) RemoveDevice(deviceHostPath string) error {
	deviceResource, err := DeviceToLinuxDevice(deviceHostPath)
	if err != nil {
		return err
	}

	c.Lock()
	defer c.Unlock()

	for i, d := range c.devices {
		if d.Type == deviceResource.Type &&
			d.Major == deviceResource.Major &&
			d.Minor == deviceResource.Minor {
			c.devices = append(c.devices[:i], c.devices[i+1:]...)
		}
	}

	if err := c.cgroup.Update(&specs.LinuxResources{
		Devices: c.devices,
	}); err != nil {
		return err
	}

	return nil
}

func (c *Cgroup) UpdateCpuSet(cpuset, memset string) error {
	c.Lock()
	defer c.Unlock()

	if len(cpuset) > 0 {
		// If we didn't have a cpuset defined, let's create:
		if c.cpusets == nil {
			c.cpusets = &specs.LinuxCPU{}
		}

		c.cpusets.Cpus = cpuset
	}

	if len(memset) > 0 {
		// If we didn't have a cpuset defined, let's now create:
		if c.cpusets == nil {
			c.cpusets = &specs.LinuxCPU{}
		}

		c.cpusets.Mems = memset
	}

	return c.cgroup.Update(&specs.LinuxResources{
		CPU: c.cpusets,
	})
}

func (c *Cgroup) Path() string {
	return c.path
}
