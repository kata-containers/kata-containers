// Copyright (c) 2026 Kata Containers contributors
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

import (
	"os"
	"path/filepath"
	"testing"

	grpc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	"github.com/stretchr/testify/require"
	"google.golang.org/protobuf/proto"
)

func TestGuestAppArmorPreflight(t *testing.T) {
	path := filepath.Join(t.TempDir(), "policy.img")
	require.NoError(t, os.WriteFile(path, []byte("policy image fixture"), 0600))
	extension := GuestExtensionImage{Name: "apparmor", Path: path}
	for _, tt := range []struct {
		name       string
		enabled    bool
		kind       HypervisorType
		hypervisor HypervisorConfig
		wantError  bool
	}{
		{name: "ordinary default", kind: QemuHypervisor},
		{name: "extension does not enable feature", kind: QemuHypervisor, hypervisor: HypervisorConfig{GuestExtensionImages: []GuestExtensionImage{extension}}},
		{name: "enabled without extension", enabled: true, kind: QemuHypervisor, wantError: true},
		{name: "enabled with extension", enabled: true, kind: QemuHypervisor, hypervisor: HypervisorConfig{GuestExtensionImages: []GuestExtensionImage{extension}}},
		{name: "missing guest image", enabled: true, kind: QemuHypervisor, hypervisor: HypervisorConfig{GuestExtensionImages: []GuestExtensionImage{extension}}, wantError: true},
		{name: "duplicate extension", enabled: true, kind: QemuHypervisor, hypervisor: HypervisorConfig{GuestExtensionImages: []GuestExtensionImage{extension, extension}}, wantError: true},
		{name: "missing path", enabled: true, kind: QemuHypervisor, hypervisor: HypervisorConfig{GuestExtensionImages: []GuestExtensionImage{{Name: "apparmor", Path: path + ".missing"}}}, wantError: true},
		{name: "initrd unsupported", enabled: true, kind: QemuHypervisor, hypervisor: HypervisorConfig{InitrdPath: "initrd", GuestExtensionImages: []GuestExtensionImage{extension}}, wantError: true},
		{name: "other hypervisor unsupported", enabled: true, kind: ClhHypervisor, hypervisor: HypervisorConfig{GuestExtensionImages: []GuestExtensionImage{extension}}, wantError: true},
		{name: "conflicting annotation", enabled: true, kind: QemuHypervisor, hypervisor: HypervisorConfig{KernelParams: []Param{{Key: GuestAppArmorKernelParam, Value: "false"}}, GuestExtensionImages: []GuestExtensionImage{extension}}, wantError: true},
		{name: "duplicate reserved parameter", kind: QemuHypervisor, hypervisor: HypervisorConfig{KernelParams: []Param{{Key: GuestAppArmorKernelParam, Value: "false"}, {Key: GuestAppArmorKernelParam, Value: "false"}}}, wantError: true},
	} {
		t.Run(tt.name, func(t *testing.T) {
			if tt.name != "missing guest image" {
				tt.hypervisor.ImagePath = path
			}
			err := ValidateGuestAppArmorConfig(tt.kind, tt.hypervisor, KataAgentConfig{EnableGuestAppArmor: tt.enabled})
			if tt.wantError {
				require.Error(t, err)
			} else {
				require.NoError(t, err)
			}
		})
	}
}

func TestGuestAppArmorHandshake(t *testing.T) {
	for _, expected := range []bool{false, true} {
		for _, actual := range []bool{false, true} {
			response := &grpc.GuestDetailsResponse{AgentDetails: &grpc.AgentDetails{GuestApparmorEnabled: actual}}
			err := validateGuestAppArmorDetails(expected, response)
			if expected == actual {
				require.NoError(t, err)
			} else {
				require.ErrorContains(t, err, "mode mismatch")
			}
		}
	}
	require.Error(t, validateGuestAppArmorDetails(true, nil))
	require.Error(t, validateGuestAppArmorDetails(true, &grpc.GuestDetailsResponse{}))
	require.NoError(t, validateGuestAppArmorDetails(false, nil))
	for _, enabled := range []bool{false, true} {
		count := 0
		for _, p := range KataAgentKernelParams(KataAgentConfig{EnableGuestAppArmor: enabled}) {
			if p.Key == GuestAppArmorKernelParam {
				count++
			}
		}
		require.Equal(t, 1, count)
	}
}

func TestGuestAppArmorHandshakeWireCompatibility(t *testing.T) {
	for _, enabled := range []bool{false, true} {
		wire, err := proto.Marshal(&grpc.GuestDetailsResponse{AgentDetails: &grpc.AgentDetails{GuestApparmorEnabled: enabled}})
		require.NoError(t, err)
		var decoded grpc.GuestDetailsResponse
		require.NoError(t, proto.Unmarshal(wire, &decoded))
		require.NoError(t, validateGuestAppArmorDetails(enabled, &decoded))
	}
	// An old agent omits field 7 entirely. It cannot satisfy an enabled runtime.
	var oldAgent grpc.GuestDetailsResponse
	require.NoError(t, proto.Unmarshal([]byte{0x12, 0x00}, &oldAgent))
	require.Error(t, validateGuestAppArmorDetails(true, &oldAgent))
	require.NoError(t, validateGuestAppArmorDetails(false, &oldAgent))
}
