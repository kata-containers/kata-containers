// Copyright (c) 2024 Kata Containers Authors
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"syscall"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
	vctypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

// --- state command ---

func TestKataStateToOCI(t *testing.T) {
	assert := assert.New(t)
	assert.Equal("created", kataStateToOCI(vctypes.StateReady))
	assert.Equal("running", kataStateToOCI(vctypes.StateRunning))
	assert.Equal("paused", kataStateToOCI(vctypes.StatePaused))
	assert.Equal("stopped", kataStateToOCI(vctypes.StateStopped))
	assert.Equal("stopped", kataStateToOCI("unknown"))
}

func TestRunStateCommandSuccess(t *testing.T) {
	assert := assert.New(t)
	ctx := context.Background()

	sandbox := &vcmock.Sandbox{}
	sandbox.StatusContainerFunc = func(cid string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{
			ID:          "mycontainer",
			PID:         1234,
			Annotations: map[string]string{"test-key": "test-value"},
			State:       vctypes.ContainerState{State: vctypes.StateRunning},
		}, nil
	}
	sandbox.ReleaseFunc = func() error { return nil }

	testingImpl.FetchSandboxFunc = func(_ context.Context, id string) (vc.VCSandbox, error) {
		assert.Equal("mycontainer", id)
		return sandbox, nil
	}
	defer func() { testingImpl.FetchSandboxFunc = nil }()

	// Capture stdout.
	old := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w

	err := runStateCommand(ctx, "mycontainer")
	w.Close()
	os.Stdout = old

	assert.NoError(err)

	buf := make([]byte, 4096)
	n, _ := r.Read(buf)
	var got ociState
	assert.NoError(json.Unmarshal(buf[:n], &got))
	assert.Equal("mycontainer", got.ID)
	assert.Equal("running", got.Status)
	assert.Equal(1234, got.PID)
}

func TestRunStateCommandNoID(t *testing.T) {
	err := runStateCommand(context.Background(), "")
	assert.Error(t, err)
}

func TestRunStateCommandFetchError(t *testing.T) {
	testingImpl.FetchSandboxFunc = func(_ context.Context, id string) (vc.VCSandbox, error) {
		return nil, fmt.Errorf("not found")
	}
	defer func() { testingImpl.FetchSandboxFunc = nil }()

	err := runStateCommand(context.Background(), "missing")
	assert.Error(t, err)
}

// --- kill command ---

func TestParseSignal(t *testing.T) {
	assert := assert.New(t)

	sig, err := parseSignal("SIGTERM")
	assert.NoError(err)
	assert.Equal(syscall.SIGTERM, sig)

	sig, err = parseSignal("TERM")
	assert.NoError(err)
	assert.Equal(syscall.SIGTERM, sig)

	sig, err = parseSignal("15")
	assert.NoError(err)
	assert.Equal(syscall.Signal(15), sig)

	_, err = parseSignal("SIGNOTEXIST")
	assert.Error(err)
}

func TestRunKillCommandSuccess(t *testing.T) {
	assert := assert.New(t)
	killed := false

	sandbox := &vcmock.Sandbox{}
	sandbox.KillContainerFunc = func(cid string, sig syscall.Signal, all bool) error {
		assert.Equal("mycontainer", cid)
		assert.Equal(syscall.SIGTERM, sig)
		killed = true
		return nil
	}
	sandbox.ReleaseFunc = func() error { return nil }

	testingImpl.FetchSandboxFunc = func(_ context.Context, id string) (vc.VCSandbox, error) {
		return sandbox, nil
	}
	defer func() { testingImpl.FetchSandboxFunc = nil }()

	err := runKillCommand(context.Background(), "mycontainer", syscall.SIGTERM, false)
	assert.NoError(err)
	assert.True(killed)
}

func TestRunKillCommandFetchError(t *testing.T) {
	testingImpl.FetchSandboxFunc = func(_ context.Context, id string) (vc.VCSandbox, error) {
		return nil, fmt.Errorf("not found")
	}
	defer func() { testingImpl.FetchSandboxFunc = nil }()

	err := runKillCommand(context.Background(), "missing", syscall.SIGTERM, false)
	assert.Error(t, err)
}

// --- delete command ---

func TestRunDeleteCommandSuccess(t *testing.T) {
	assert := assert.New(t)
	cleaned := false

	testingImpl.CleanupContainerFunc = func(_ context.Context, sandboxID, containerID string, force bool) error {
		assert.Equal("mycontainer", sandboxID)
		assert.Equal("mycontainer", containerID)
		cleaned = true
		return nil
	}
	defer func() { testingImpl.CleanupContainerFunc = nil }()

	err := runDeleteCommand(context.Background(), "mycontainer", false)
	assert.NoError(err)
	assert.True(cleaned)
}

func TestRunDeleteCommandForceIgnoresError(t *testing.T) {
	testingImpl.CleanupContainerFunc = func(_ context.Context, sandboxID, containerID string, force bool) error {
		return fmt.Errorf("cleanup failed")
	}
	defer func() { testingImpl.CleanupContainerFunc = nil }()

	err := runDeleteCommand(context.Background(), "mycontainer", true)
	assert.NoError(t, err)
}

// --- start command ---

func TestRunStartCommandSuccess(t *testing.T) {
	assert := assert.New(t)
	started := false

	sandbox := &vcmock.Sandbox{}
	sandbox.StartContainerFunc = func(cid string) (vc.VCContainer, error) {
		assert.Equal("mycontainer", cid)
		started = true
		return &vcmock.Container{}, nil
	}
	sandbox.ReleaseFunc = func() error { return nil }

	testingImpl.FetchSandboxFunc = func(_ context.Context, id string) (vc.VCSandbox, error) {
		return sandbox, nil
	}
	defer func() { testingImpl.FetchSandboxFunc = nil }()

	err := runStartCommand(context.Background(), "mycontainer")
	assert.NoError(err)
	assert.True(started)
}

func TestRunStartCommandFetchError(t *testing.T) {
	testingImpl.FetchSandboxFunc = func(_ context.Context, id string) (vc.VCSandbox, error) {
		return nil, fmt.Errorf("not found")
	}
	defer func() { testingImpl.FetchSandboxFunc = nil }()

	err := runStartCommand(context.Background(), "missing")
	assert.Error(t, err)
}

// --- pause / resume ---

func TestRunPauseCommandSuccess(t *testing.T) {
	assert := assert.New(t)
	paused := false

	sandbox := &vcmock.Sandbox{}
	sandbox.PauseContainerFunc = func(cid string) error {
		paused = true
		return nil
	}
	sandbox.ReleaseFunc = func() error { return nil }

	testingImpl.FetchSandboxFunc = func(_ context.Context, id string) (vc.VCSandbox, error) {
		return sandbox, nil
	}
	defer func() { testingImpl.FetchSandboxFunc = nil }()

	err := runPauseCommand(context.Background(), "mycontainer")
	assert.NoError(err)
	assert.True(paused)
}

func TestRunResumeCommandSuccess(t *testing.T) {
	assert := assert.New(t)
	resumed := false

	sandbox := &vcmock.Sandbox{}
	sandbox.ResumeContainerFunc = func(cid string) error {
		resumed = true
		return nil
	}
	sandbox.ReleaseFunc = func() error { return nil }

	testingImpl.FetchSandboxFunc = func(_ context.Context, id string) (vc.VCSandbox, error) {
		return sandbox, nil
	}
	defer func() { testingImpl.FetchSandboxFunc = nil }()

	err := runResumeCommand(context.Background(), "mycontainer")
	assert.NoError(err)
	assert.True(resumed)
}

// --- create command (bundle path validation) ---

func TestRunCreateCommandMissingBundle(t *testing.T) {
	err := runCreateCommand(context.Background(), "mycontainer", "/nonexistent/bundle", "", oci.RuntimeConfig{})
	assert.Error(t, err)
}

func TestCreateTestBundle(t *testing.T) {
	dir := t.TempDir()
	rootfs := filepath.Join(dir, "rootfs")
	assert.NoError(t, os.MkdirAll(rootfs, 0755))

	spec := map[string]interface{}{
		"ociVersion": "1.0.0",
		"root":       map[string]interface{}{"path": "rootfs"},
		"process": map[string]interface{}{
			"args": []string{"/bin/sh"},
			"cwd":  "/",
		},
	}
	b, _ := json.Marshal(spec)
	assert.NoError(t, os.WriteFile(filepath.Join(dir, "config.json"), b, 0644))
	assert.FileExists(t, filepath.Join(dir, "config.json"))
}
