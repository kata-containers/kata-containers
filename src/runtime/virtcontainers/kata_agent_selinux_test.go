// Copyright (c) 2026 Kata Containers contributors
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

import (
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/require"
)

func TestCmdToKataProcessPreservesSelinuxRequest(t *testing.T) {
	for _, label := range []string{
		"",
		"system_u:system_r:container_t:s0:c42,c99",
		"system_u:system_r:container_kvm_t:s0:c42,c99",
		"system_u:system_r:spc_t:s0",
	} {
		t.Run(label, func(t *testing.T) {
			process, err := cmdToKataProcess(types.Cmd{User: "0", PrimaryGroup: "0", SelinuxLabel: label})
			require.NoError(t, err)
			require.Equal(t, label, process.SelinuxLabel)
		})
	}
}

func TestNormalizeSelinuxProcessLabel(t *testing.T) {
	t.Run("empty stays empty", func(t *testing.T) {
		label, err := normalizeSelinuxProcessLabel("", "")
		require.NoError(t, err)
		require.Equal(t, "", label)
	})

	// A manager authors the context for the host KVM process; the guest must see the
	// container domain instead, with the sensitivity and categories preserved untouched.
	t.Run("kvm type is retyped to container", func(t *testing.T) {
		label, err := normalizeSelinuxProcessLabel("system_u:system_r:container_kvm_t:s0:c42,c99", "")
		require.NoError(t, err)
		require.Equal(t, "system_u:system_r:container_t:s0:c42,c99", label)
	})

	t.Run("configured guest label wins", func(t *testing.T) {
		label, err := normalizeSelinuxProcessLabel(
			"system_u:system_r:container_kvm_t:s0:c42,c99",
			"system_u:system_r:my_app_t:s0",
		)
		require.NoError(t, err)
		require.Equal(t, "system_u:system_r:my_app_t:s0", label)
	})

	t.Run("malformed context is rejected", func(t *testing.T) {
		_, err := normalizeSelinuxProcessLabel("not-a-selinux-context", "")
		require.Error(t, err)
	})
}

func TestExecSelinuxLabel(t *testing.T) {
	const (
		kvmLabel = "system_u:system_r:container_kvm_t:s0:c42,c99"
		invalid  = "not-a-selinux-context"
	)

	// With guest SELinux disabled the request is blanked before anything is parsed, matching the
	// create path: a context that only the host cares about must not make Exec fail there.
	for _, label := range []string{"", kvmLabel, invalid} {
		t.Run("disabled blanks without parsing: "+label, func(t *testing.T) {
			got, err := execSelinuxLabel(label, "", true)
			require.NoError(t, err)
			require.Equal(t, "", got)
		})
	}

	for _, test := range []struct {
		name  string
		label string
		guest string
		want  string
	}{
		{"empty stays empty", "", "", ""},
		{"kvm type is retyped to container", kvmLabel, "", "system_u:system_r:container_t:s0:c42,c99"},
		{"configured guest label wins", kvmLabel, "system_u:system_r:my_app_t:s0", "system_u:system_r:my_app_t:s0"},
	} {
		t.Run(test.name, func(t *testing.T) {
			got, err := execSelinuxLabel(test.label, test.guest, false)
			require.NoError(t, err)
			require.Equal(t, test.want, got)
		})
	}

	t.Run("enabled rejects a malformed context", func(t *testing.T) {
		_, err := execSelinuxLabel(invalid, "", false)
		require.Error(t, err)
	})
}
