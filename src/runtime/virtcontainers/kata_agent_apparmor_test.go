// Copyright (c) 2026 Kata Containers contributors
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

import (
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/require"
)

func TestCmdToKataProcessPreservesAppArmorRequest(t *testing.T) {
	for _, profile := range []string{"", "kata-apparmor-test", "localhost/kata-apparmor-test", "unconfined", "different-profile"} {
		t.Run(profile, func(t *testing.T) {
			process, err := cmdToKataProcess(types.Cmd{User: "0", PrimaryGroup: "0", ApparmorProfile: profile})
			require.NoError(t, err)
			require.Equal(t, profile, process.ApparmorProfile)
		})
	}
}
