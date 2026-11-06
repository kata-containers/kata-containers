// Copyright (c) 2026 Kata Containers contributors
// SPDX-License-Identifier: Apache-2.0

package containerdshim

import (
	"testing"

	"github.com/containerd/containerd/protobuf"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/require"
)

func TestNewExecPreservesAppArmorRequest(t *testing.T) {
	for _, profile := range []string{"", "kata-apparmor-test", "localhost/kata-apparmor-test", "unconfined", "different-profile"} {
		t.Run(profile, func(t *testing.T) {
			request, err := protobuf.MarshalAnyToProto(&specs.Process{
				Args: []string{"/bin/true"}, ApparmorProfile: profile,
			})
			require.NoError(t, err)
			exec, err := newExec(&container{}, "", "", "", false, request)
			require.NoError(t, err)
			require.Equal(t, profile, exec.cmds.ApparmorProfile)
		})
	}
}
