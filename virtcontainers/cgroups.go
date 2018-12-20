// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/json"
	"fmt"

	"github.com/containerd/cgroups"
	"github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	specs "github.com/opencontainers/runtime-spec/specs-go"
)

const (
	vcpuGroupName       = "vcpu"
	defaultCgroupParent = "/kata"
)

type sandboxCgroups struct {
	commonParent cgroups.Cgroup
	sandboxSub   cgroups.Cgroup
	vcpuSub      cgroups.Cgroup
}

func (s *Sandbox) newCgroups() error {
	// New will still succeed when cgroup exists
	// create common parent for all kata-containers
	// e.g. /sys/fs/cgroup/cpu/vc
	parent, err := cgroups.New(cgroups.V1,
		cgroups.StaticPath(defaultCgroupParent), &specs.LinuxResources{})
	if err != nil {
		return fmt.Errorf("failed to create cgroup for %q", defaultCgroupParent)
	}

	// create sub-cgroup for each sandbox
	// e.g. /sys/fs/cgroup/cpu/vc/<sandbox>
	sandboxSub, err := parent.New(s.id, &specs.LinuxResources{})
	if err != nil {
		return fmt.Errorf("failed to create cgroup for %s/%s", defaultCgroupParent, s.id)
	}

	// create sub-cgroup for vcpu threads
	vcpuSub, err := sandboxSub.New(vcpuGroupName, &specs.LinuxResources{})
	if err != nil {
		return fmt.Errorf("failed to create cgroup for %s/%s/%s", defaultCgroupParent, s.id, vcpuGroupName)
	}

	s.cgroup = &sandboxCgroups{
		commonParent: parent,
		sandboxSub:   sandboxSub,
		vcpuSub:      vcpuSub,
	}
	return nil
}

func (s *Sandbox) destroyCgroups() error {
	if s.cgroup == nil {
		s.Logger().Warningf("cgroup is not initialized, no need to destroy")
		return nil
	}

	// first move all processes in subgroup to parent in case live process blocks
	// deletion of cgroup
	if err := s.cgroup.sandboxSub.MoveTo(s.cgroup.commonParent); err != nil {
		return fmt.Errorf("failed to clear cgroup processes")
	}

	return s.cgroup.sandboxSub.Delete()
}

func (s *Sandbox) setupCgroups() error {
	if s.cgroup == nil {
		return fmt.Errorf("failed to setup uninitialized cgroup for sandbox")
	}

	resource, err := s.mergeSpecResource()
	if err != nil {
		return err
	}

	if err := s.applyCPUCgroup(resource); err != nil {
		return err
	}
	return nil
}

func (s *Sandbox) applyCPUCgroup(rc *specs.LinuxResources) error {
	if s.cgroup == nil {
		return fmt.Errorf("failed to setup uninitialized cgroup for sandbox")
	}

	// apply cpu constraint to vcpu cgroup
	if err := s.cgroup.vcpuSub.Update(rc); err != nil {
		return err
	}

	// when new container joins, new CPU could be hotplugged, so we
	// have to query fresh vcpu info from hypervisor for every time.
	tids, err := s.hypervisor.getThreadIDs()
	if err != nil {
		return fmt.Errorf("failed to get thread ids from hypervisor: %v", err)
	}
	if tids == nil {
		// If there's no tid returned from the hypervisor, this is not
		// a bug. It simply means there is nothing to constrain, hence
		// let's return without any error from here.
		return nil
	}

	// use Add() to add vcpu thread to s.cgroup, it will write thread id to
	// `cgroup.procs` which will move all threads in qemu process to this cgroup
	// immediately as default behaviour.
	if len(tids.vcpus) > 0 {
		if err := s.cgroup.sandboxSub.Add(cgroups.Process{
			Pid: tids.vcpus[0],
		}); err != nil {
			return err
		}
	}

	for _, i := range tids.vcpus {
		if i <= 0 {
			continue
		}

		// In contrast, AddTask will write thread id to `tasks`
		// After this, vcpu threads are in "vcpu" sub-cgroup, other threads in
		// qemu will be left in parent cgroup untouched.
		if err := s.cgroup.vcpuSub.AddTask(cgroups.Process{
			Pid: i,
		}); err != nil {
			return err
		}
	}

	return nil
}

func (s *Sandbox) mergeSpecResource() (*specs.LinuxResources, error) {
	if s.config == nil {
		return nil, fmt.Errorf("sandbox config is nil")
	}

	resource := &specs.LinuxResources{
		CPU: &specs.LinuxCPU{},
	}

	for _, c := range s.config.Containers {
		config, ok := c.Annotations[annotations.ConfigJSONKey]
		if !ok {
			s.Logger().WithField("container", c.ID).Warningf("failed to find config from container annotations")
			continue
		}

		var spec specs.Spec
		if err := json.Unmarshal([]byte(config), &spec); err != nil {
			return nil, err
		}

		// TODO: how to handle empty/unlimited resource?
		// maybe we should add a default CPU/Memory delta when no
		// resource limit is given. -- @WeiZhang555
		if spec.Linux == nil || spec.Linux.Resources == nil {
			continue
		}
		// calculate cpu quota and period
		s.mergeCPUResource(resource, spec.Linux.Resources)
	}
	return resource, nil
}

func (s *Sandbox) mergeCPUResource(orig, rc *specs.LinuxResources) {
	if orig.CPU == nil {
		orig.CPU = &specs.LinuxCPU{}
	}

	if rc.CPU != nil && rc.CPU.Quota != nil && rc.CPU.Period != nil &&
		*rc.CPU.Quota > 0 && *rc.CPU.Period > 0 {
		if orig.CPU.Period == nil {
			orig.CPU.Period = rc.CPU.Period
			orig.CPU.Quota = rc.CPU.Quota
		} else {
			// this is an example to show how it works:
			// container A and `orig` has quota: 5000 and period 10000
			// here comes container B with quota 40 and period 100,
			// so use previous period 10000 as a baseline, container B
			// has proportional resource of quota 4000 and period 10000, calculated as
			// delta := 40 / 100 * 10000 = 4000
			// and final `*orig.CPU.Quota` = 5000 + 4000 = 9000
			delta := float64(*rc.CPU.Quota) / float64(*rc.CPU.Period) * float64(*orig.CPU.Period)
			*orig.CPU.Quota += int64(delta)
		}
	}
}
