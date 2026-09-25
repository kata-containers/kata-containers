// Copyright (c) 2023 IBM Corporation
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

import (
	"context"
	"net"
	"os"
	"path/filepath"
	"testing"
	"time"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/pkg/hypervisors"
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

func TestRemoteHypervisorSaveLoad(t *testing.T) {
	assert := assert.New(t)

	// Create a remote hypervisor with state
	rh := &remoteHypervisor{
		sandboxID:       remoteHypervisorSandboxID("test-sandbox-123"),
		agentSocketPath: "/run/peerpod/pods/test-sandbox-123/agent.sock",
		config:          newRemoteConfig(),
	}

	// Save the state
	savedState := rh.Save()

	// Verify saved state contains expected values
	assert.Equal("/run/peerpod/pods/test-sandbox-123/agent.sock", savedState.AgentSocketPath)

	// Create a new remote hypervisor (simulating restart)
	rh2 := &remoteHypervisor{
		config: newRemoteConfig(),
	}

	// Verify initial state is empty
	assert.Empty(rh2.agentSocketPath)

	// Load the saved state
	rh2.Load(savedState)

	// Verify state was restored
	assert.Equal("/run/peerpod/pods/test-sandbox-123/agent.sock", rh2.agentSocketPath)
}

func TestCreateVMReconnectsWithRestoredState(t *testing.T) {
	assert := assert.New(t)

	rh := &remoteHypervisor{
		config: newRemoteConfig(),
	}

	rh.Load(persistapi.HypervisorState{
		AgentSocketPath: "/run/peerpod/pods/test-sandbox-123/agent.sock",
	})

	config := newRemoteConfig()
	err := rh.CreateVM(context.Background(), "test-sandbox-123", nil, &config)
	assert.NoError(err)

	assert.Equal("/run/peerpod/pods/test-sandbox-123/agent.sock", rh.agentSocketPath)
	assert.Equal(remoteHypervisorSandboxID("test-sandbox-123"), rh.sandboxID)
}

func TestOpenRemoteServiceRetry(t *testing.T) {
	assert := assert.New(t)

	// Test 1: Non-existent socket should fail after retries
	t.Run("fails after retries on non-existent socket", func(t *testing.T) {
		start := time.Now()
		_, err := openRemoteServiceWithRetry("/nonexistent/path/to/socket.sock", 3, 10*time.Millisecond)
		elapsed := time.Since(start)

		assert.Error(err)
		assert.Contains(err.Error(), "failed to connect to remote hypervisor socket")
		assert.True(elapsed >= 10*time.Millisecond, "should have retried with delay, elapsed: %v", elapsed)
		assert.True(elapsed < 1*time.Second, "should complete quickly with short base delay, elapsed: %v", elapsed)
	})

	// Test 2: Valid socket connects successfully
	t.Run("connects to valid socket", func(t *testing.T) {
		tmpDir, err := os.MkdirTemp("", "remote-test-*")
		assert.NoError(err)
		defer os.RemoveAll(tmpDir)

		socketPath := filepath.Join(tmpDir, "test.sock")
		listener, err := net.Listen("unix", socketPath)
		assert.NoError(err)
		defer listener.Close()

		svc, err := openRemoteServiceWithRetry(socketPath, 3, 10*time.Millisecond)
		assert.NoError(err)
		assert.NotNil(svc)
		assert.NotNil(svc.conn)
		assert.NotNil(svc.client)

		err = svc.Close()
		assert.NoError(err)
	})
}
