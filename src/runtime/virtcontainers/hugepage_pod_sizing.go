// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"errors"
	"fmt"
)

func hypervisorBacksGuestRAMWithHugePages(hypervisorType HypervisorType) bool {
	switch hypervisorType {
	case QemuHypervisor, ClhHypervisor:
		return true
	default:
		return false
	}
}

func hugePagePodSized(hc *HypervisorConfig) bool {
	return hc.HugePages && hc.HugePagesPodSizing
}

// CheckHugePagesPodSizing runs when the configuration loads and again for each
// sandbox, because annotations can change the settings it depends on.
func CheckHugePagesPodSizing(hypervisorType HypervisorType, staticResourceMgmt, sandboxCgroupOnly, vmFactory bool) error {
	switch {
	case !hypervisorBacksGuestRAMWithHugePages(hypervisorType):
		return fmt.Errorf("enable_hugepages_pod_sizing needs a hypervisor that maps guest RAM from the huge page pool (qemu or cloud-hypervisor), and this sandbox uses %s", hypervisorType)
	case !staticResourceMgmt:
		return errors.New("enable_hugepages_pod_sizing needs static_sandbox_resource_mgmt")
	case !sandboxCgroupOnly:
		return errors.New("enable_hugepages_pod_sizing needs sandbox_cgroup_only: the hypervisor's huge pages are charged to the pod's cgroup")
	case vmFactory:
		return errors.New("enable_hugepages_pod_sizing sizes each VM from its pod, so it cannot use VM templates or the VM cache")
	}
	return nil
}
