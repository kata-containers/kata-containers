// Copyright (c) 2026 Kata Containers contributors
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

import (
	"fmt"
	"os"
	"strconv"
	"strings"

	grpc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
)

// GuestAppArmorKernelParam is owned by the administrator's agent configuration,
// never by a workload's kernel-parameter annotation.
const GuestAppArmorKernelParam = "agent.enable_guest_apparmor"

// ValidateGuestAppArmorConfig is also called after annotation processing, before
// creating a VM. Attaching a policy extension does not itself enable enforcement.
func ValidateGuestAppArmorConfig(kind HypervisorType, hypervisor HypervisorConfig, agent KataAgentConfig) error {
	seen := false
	for _, param := range hypervisor.KernelParams {
		if param.Key != GuestAppArmorKernelParam {
			continue
		}
		if seen || param.Value != strconv.FormatBool(agent.EnableGuestAppArmor) {
			return fmt.Errorf("duplicate or conflicting reserved parameter %s", GuestAppArmorKernelParam)
		}
		seen = true
	}
	if !agent.EnableGuestAppArmor {
		return nil
	}
	if kind != QemuHypervisor || strings.TrimSpace(hypervisor.ImagePath) == "" || hypervisor.InitrdPath != "" {
		return fmt.Errorf("guest AppArmor requires a QEMU systemd image guest")
	}
	count := 0
	for _, extension := range hypervisor.GuestExtensionImages {
		if extension.Name != "apparmor" {
			continue
		}
		count++
		info, err := os.Stat(extension.Path)
		if err != nil {
			return fmt.Errorf("guest AppArmor extension: %w", err)
		}
		if !info.Mode().IsRegular() {
			return fmt.Errorf("guest AppArmor extension must be a regular file")
		}
		file, err := os.Open(extension.Path)
		if err != nil {
			return fmt.Errorf("guest AppArmor extension: %w", err)
		}
		if err := file.Close(); err != nil {
			return err
		}
	}
	if count != 1 {
		return fmt.Errorf("enable_guest_apparmor requires exactly one apparmor guest extension (found %d)", count)
	}
	return nil
}

func validateGuestAppArmorDetails(expected bool, details *grpc.GuestDetailsResponse) error {
	actual := false
	if details != nil && details.AgentDetails != nil {
		actual = details.AgentDetails.GuestApparmorEnabled
	}
	if actual != expected {
		return fmt.Errorf("guest AppArmor mode mismatch: runtime enabled=%t, agent enabled=%t (an older agent cannot satisfy enable_guest_apparmor)", expected, actual)
	}
	return nil
}
