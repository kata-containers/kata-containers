// Copyright (c) 2026 Kata Containers contributors
// SPDX-License-Identifier: Apache-2.0

package katautils

import (
	"testing"

	"github.com/BurntSushi/toml"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/stretchr/testify/require"
)

func TestGuestAppArmorAgentConfig(t *testing.T) {
	for _, enabled := range []bool{false, true} {
		input := "[agent.kata]\nenable_guest_apparmor = false\n"
		if enabled {
			input = "[agent.kata]\nenable_guest_apparmor = true\n"
		}
		var parsed tomlConfig
		_, err := toml.Decode(input, &parsed)
		require.NoError(t, err)
		var runtime oci.RuntimeConfig
		require.NoError(t, updateRuntimeConfigAgent("fixture", parsed, &runtime))
		require.Equal(t, enabled, runtime.AgentConfig.EnableGuestAppArmor)
	}
}

func TestGuestAppArmorReservedKernelParameter(t *testing.T) {
	for _, value := range []string{"true", "false", ""} {
		config := oci.RuntimeConfig{HypervisorConfig: vc.HypervisorConfig{KernelParams: []vc.Param{{Key: vc.GuestAppArmorKernelParam, Value: value}}}}
		require.ErrorContains(t, rejectGuestAppArmorKernelOverride(config.HypervisorConfig.KernelParams), "reserved")
	}
}
