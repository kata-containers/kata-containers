//go:build linux

// Copyright (c) 2021-2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"

	"github.com/containerd/cgroups"
	cgroupsv2 "github.com/containerd/cgroups/v2"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

const (
	// prepend a kata specific string to oci cgroup path to
	// form a different cgroup path, thus cAdvisor couldn't
	// find kata containers cgroup path on host to prevent it
	// from grabbing the stats data.
	CgroupKataPrefix = "kata"

	// cgroup v2 mount point
	unifiedMountpoint = "/sys/fs/cgroup"
)

func RenameCgroupPath(path string) (string, error) {
	if path == "" {
		path = DefaultResourceControllerID
	}

	cgroupPathDir := filepath.Dir(path)
	cgroupPathName := fmt.Sprintf("%s_%s", CgroupKataPrefix, filepath.Base(path))
	return filepath.Join(cgroupPathDir, cgroupPathName), nil
}

type LinuxCgroup struct {
	cgroup  interface{}
	path    string
	cpusets *specs.LinuxCPU
	devices []specs.LinuxDeviceCgroup

	sync.Mutex
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
		"/dev/kvm",         // To run virtual machines with KVM
		"/dev/mshv",        // To run virtual machines with Hyper-V
		"/dev/vhost-net",   // To create virtqueues
		"/dev/vfio/vfio",   // To access VFIO devices
		"/dev/vhost-vsock", // To interact with vsock if
	}

	defaultDevices = append(defaultDevices, hypervisorDevices...)

	for _, device := range defaultDevices {
		ldevice, err := DeviceToLinuxDevice(device)
		if err != nil {
			controllerLogger.WithField("source", "cgroups").Warnf("Could not add %s to the devices cgroup", device)
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

func NewResourceController(path string, resources *specs.LinuxResources) (ResourceController, error) {
	var err error
	var cgroup interface{}
	var cgroupPath string

	if cgroups.Mode() == cgroups.Legacy || cgroups.Mode() == cgroups.Hybrid {
		cgroupPath, err = ValidCgroupPath(path, false, IsSystemdCgroup(path))
		if err != nil {
			return nil, err
		}
		cgroup, err = cgroups.New(cgroups.V1, cgroups.StaticPath(cgroupPath), resources)
		if err != nil {
			return nil, err
		}
	} else if cgroups.Mode() == cgroups.Unified {
		cgroupPath, err = ValidCgroupPath(path, true, IsSystemdCgroup(path))
		if err != nil {
			return nil, err
		}
		cgroup, err = cgroupsv2.NewManager(unifiedMountpoint, cgroupPath, cgroupsv2.ToResources(resources))
		if err != nil {
			return nil, err
		}
	} else {
		return nil, ErrCgroupMode
	}

	return &LinuxCgroup{
		path:    cgroupPath,
		devices: resources.Devices,
		cpusets: resources.CPU,
		cgroup:  cgroup,
	}, nil
}

func NewSandboxResourceController(path string, resources *specs.LinuxResources, sandboxCgroupOnly bool) (ResourceController, error) {
	sandboxResources := *resources
	sandboxResources.Devices = append(sandboxResources.Devices, sandboxDevices()...)

	// Currently we know to handle systemd cgroup path only when it's the only cgroup (no overhead group), hence,
	// if sandboxCgroupOnly is not true we treat it as cgroupfs path as it used to be, although it may be incorrect.
	if !IsSystemdCgroup(path) || !sandboxCgroupOnly {
		return NewResourceController(path, &sandboxResources)
	}

	var cgroup interface{}

	slice, unit, err := getSliceAndUnit(path)
	if err != nil {
		return nil, err
	}

	//github.com/containerd/cgroups doesn't support creating a scope unit with
	//v1 and v2 cgroups against systemd, the following interacts directly with systemd
	//to create the cgroup and then load it using containerd's api.
	//adding runtime process, it makes calling setupCgroups redundant
	if createCgroupsSystemd(slice, unit, os.Getpid()); err != nil {
		return nil, err
	}

	// Create systemd cgroup
	if cgroups.Mode() == cgroups.Legacy || cgroups.Mode() == cgroups.Hybrid {
		cgHierarchy, cgPath, err := cgroupHierarchy(path)
		if err != nil {
			return nil, err
		}

		// load created cgroup and update with resources
		cg, err := cgroups.Load(cgHierarchy, cgPath)
		if err != nil {
			if cg.Update(&sandboxResources); err != nil {
				return nil, err
			}
		}
		cgroup = cg
	} else if cgroups.Mode() == cgroups.Unified {
		// load created cgroup and update with resources
		cg, err := cgroupsv2.LoadSystemd(slice, unit)
		if err != nil {
			if cg.Update(cgroupsv2.ToResources(&sandboxResources)); err != nil {
				return nil, err
			}
		}
		cgroup = cg
	} else {
		return nil, ErrCgroupMode
	}

	return &LinuxCgroup{
		path:    path,
		devices: sandboxResources.Devices,
		cpusets: sandboxResources.CPU,
		cgroup:  cgroup,
	}, nil
}

func LoadResourceController(path string) (ResourceController, error) {
	var err error
	var cgroup interface{}

	// load created cgroup and update with resources
	if cgroups.Mode() == cgroups.Legacy || cgroups.Mode() == cgroups.Hybrid {
		cgHierarchy, cgPath, err := cgroupHierarchy(path)
		if err != nil {
			return nil, err
		}

		cgroup, err = cgroups.Load(cgHierarchy, cgPath)
		if err != nil {
			return nil, err
		}
	} else if cgroups.Mode() == cgroups.Unified {
		if IsSystemdCgroup(path) {
			slice, unit, err := getSliceAndUnit(path)
			if err != nil {
				return nil, err
			}
			cgroup, err = cgroupsv2.LoadSystemd(slice, unit)
			if err != nil {
				return nil, err
			}
		} else {
			cgroup, err = cgroupsv2.LoadManager(unifiedMountpoint, path)
			if err != nil {
				return nil, err
			}
		}
	} else {
		return nil, ErrCgroupMode
	}

	return &LinuxCgroup{
		path:   path,
		cgroup: cgroup,
	}, nil
}

func (c *LinuxCgroup) Logger() *logrus.Entry {
	return controllerLogger.WithField("source", "cgroups")
}

func (c *LinuxCgroup) Delete() error {
	switch cg := c.cgroup.(type) {
	case cgroups.Cgroup:
		return cg.Delete()
	case *cgroupsv2.Manager:
		if IsSystemdCgroup(c.ID()) {
			if err := cg.DeleteSystemd(); err != nil {
				return err
			}
		}
		return cg.Delete()
	default:
		return ErrCgroupMode
	}
}

func (c *LinuxCgroup) Stat() (interface{}, error) {
	switch cg := c.cgroup.(type) {
	case cgroups.Cgroup:
		return cg.Stat(cgroups.IgnoreNotExist)
	case *cgroupsv2.Manager:
		return cg.Stat()
	default:
		return nil, ErrCgroupMode
	}
}

func (c *LinuxCgroup) AddProcess(pid int, subsystems ...string) error {
	switch cg := c.cgroup.(type) {
	case cgroups.Cgroup:
		return cg.AddProc(uint64(pid))
	case *cgroupsv2.Manager:
		return cg.AddProc(uint64(pid))
	default:
		return ErrCgroupMode
	}
}

func (c *LinuxCgroup) AddThread(pid int, subsystems ...string) error {
	switch cg := c.cgroup.(type) {
	case cgroups.Cgroup:
		return cg.AddTask(cgroups.Process{Pid: pid})
	case *cgroupsv2.Manager:
		return cg.AddProc(uint64(pid))
	default:
		return ErrCgroupMode
	}
}

func (c *LinuxCgroup) Update(resources *specs.LinuxResources) error {
	switch cg := c.cgroup.(type) {
	case cgroups.Cgroup:
		return cg.Update(resources)
	case *cgroupsv2.Manager:
		return cg.Update(cgroupsv2.ToResources(resources))
	default:
		return ErrCgroupMode
	}
}

func (c *LinuxCgroup) MoveTo(path string) error {
	switch cg := c.cgroup.(type) {
	case cgroups.Cgroup:
		cgHierarchy, cgPath, err := cgroupHierarchy(path)
		if err != nil {
			return err
		}
		newCgroup, err := cgroups.Load(cgHierarchy, cgPath)
		if err != nil {
			return err
		}
		return cg.MoveTo(newCgroup)
	case *cgroupsv2.Manager:
		newCgroup, err := cgroupsv2.LoadManager(unifiedMountpoint, path)
		if err != nil {
			return err
		}
		return cg.MoveTo(newCgroup)
	default:
		return ErrCgroupMode
	}
}

func (c *LinuxCgroup) AddDevice(deviceHostPath string) error {
	deviceResource, err := DeviceToLinuxDevice(deviceHostPath)
	if err != nil {
		return err
	}

	c.Lock()
	defer c.Unlock()

	c.devices = append(c.devices, deviceResource)

	switch cg := c.cgroup.(type) {
	case cgroups.Cgroup:
		if err := cg.Update(&specs.LinuxResources{
			Devices: c.devices,
		}); err != nil {
			return err
		}
	case *cgroupsv2.Manager:
		if err := cg.Update(cgroupsv2.ToResources(&specs.LinuxResources{
			Devices: c.devices,
		})); err != nil {
			return err
		}
	default:
		return ErrCgroupMode
	}

	return nil
}

func (c *LinuxCgroup) RemoveDevice(deviceHostPath string) error {
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

	switch cg := c.cgroup.(type) {
	case cgroups.Cgroup:
		if err := cg.Update(&specs.LinuxResources{
			Devices: c.devices,
		}); err != nil {
			return err
		}
	case *cgroupsv2.Manager:
		if err := cg.Update(cgroupsv2.ToResources(&specs.LinuxResources{
			Devices: c.devices,
		})); err != nil {
			return err
		}
	default:
		return ErrCgroupMode
	}

	return nil
}

func (c *LinuxCgroup) UpdateCpuSet(cpuset, memset string) error {
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

	switch cg := c.cgroup.(type) {
	case cgroups.Cgroup:
		return cg.Update(&specs.LinuxResources{
			CPU: c.cpusets,
		})
	case *cgroupsv2.Manager:
		return cg.Update(cgroupsv2.ToResources(&specs.LinuxResources{
			CPU: c.cpusets,
		}))
	default:
		return ErrCgroupMode
	}
}

func (c *LinuxCgroup) Type() ResourceControllerType {
	return LinuxCgroups
}

func (c *LinuxCgroup) ID() string {
	return c.path
}

func (c *LinuxCgroup) Parent() string {
	return filepath.Dir(c.path)
}
