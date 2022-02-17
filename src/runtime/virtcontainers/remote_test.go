// Copyright (c) 2023 IBM Corporation
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

import (
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

func newRemoteConfig() HypervisorConfig {
	return HypervisorConfig{
		RemoteHypervisorSocket:  "/run/peerpod/hypervisor.sock",
		RemoteHypervisorTimeout: 600,
		DisableGuestSeLinux:     true,
		EnableAnnotations:       []string{},
	}
}

func TestRemoteHypervisorGenerateSocket(t *testing.T) {
	assert := assert.New(t)

	remoteHypervisor := remoteHypervisor{
		config: newRemoteConfig(),
	}
	id := "sandboxId"

	// No socketPath should error
	_, err := remoteHypervisor.GenerateSocket(id)
	assert.Error(err)

	socketPath := "socketPath"
	remoteHypervisor.agentSocketPath = socketPath

	result, err := remoteHypervisor.GenerateSocket(id)
	assert.NoError(err)

	expected := types.RemoteSock{
		SandboxID:        id,
		TunnelSocketPath: socketPath,
	}
	assert.Equal(result, expected)
}
