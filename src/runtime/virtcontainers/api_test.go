// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	resCtrl "github.com/kata-containers/kata-containers/src/runtime/pkg/resourcecontrol"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/fs"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/mock"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

const (
	containerID = "1"
)

var newMockAgent = NewMockAgent

var sandboxAnnotations = map[string]string{
	"sandbox.foo":   "sandbox.bar",
	"sandbox.hello": "sandbox.world",
}

var containerAnnotations = map[string]string{
	"container.foo":   "container.bar",
	"container.hello": "container.world",
}

func init() {
	rootless.IsRootless = func() bool { return false }
}

func newEmptySpec() *specs.Spec {
	return &specs.Spec{
		Linux: &specs.Linux{
			Resources:   &specs.LinuxResources{},
			CgroupsPath: resCtrl.DefaultResourceControllerID,
		},
		Process: &specs.Process{
			Capabilities: &specs.LinuxCapabilities{},
		},
	}
}

func newBasicTestCmd() types.Cmd {
	envs := []types.EnvVar{
		{
			Var:   "PATH",
			Value: "/bin:/usr/bin:/sbin:/usr/sbin",
		},
	}

	cmd := types.Cmd{
		Args:    strings.Split("/bin/sh", " "),
		Envs:    envs,
		WorkDir: "/",
	}

	return cmd
}

func newTestSandboxConfigNoop() SandboxConfig {
	bundlePath := filepath.Join(testDir, testBundle)
	containerAnnotations[annotations.BundlePathKey] = bundlePath
	containerAnnotations[annotations.ContainerTypeKey] = "pod_sandbox"

	emptySpec := newEmptySpec()

	// Define the container command and bundle.
	container := ContainerConfig{
		ID:          containerID,
		RootFs:      RootFs{Target: bundlePath, Mounted: true},
		Cmd:         newBasicTestCmd(),
		Annotations: containerAnnotations,
		CustomSpec:  emptySpec,
	}

	// Sets the hypervisor configuration.
	hypervisorConfig := HypervisorConfig{
		KernelPath:     filepath.Join(testDir, testKernel),
		ImagePath:      filepath.Join(testDir, testImage),
		HypervisorPath: filepath.Join(testDir, testHypervisor),
	}

	sandboxConfig := SandboxConfig{
		ID:               testSandboxID,
		HypervisorType:   MockHypervisor,
		HypervisorConfig: hypervisorConfig,

		Containers: []ContainerConfig{container},

		Annotations: sandboxAnnotations,

		AgentConfig: KataAgentConfig{},
	}

	configFile := filepath.Join(bundlePath, "config.json")
	f, err := os.OpenFile(configFile, os.O_RDWR|os.O_CREATE, 0644)
	if err != nil {
		return SandboxConfig{}
	}
	defer f.Close()

	if err := json.NewEncoder(f).Encode(emptySpec); err != nil {
		return SandboxConfig{}
	}

	return sandboxConfig
}

func newTestSandboxConfigKataAgent() SandboxConfig {
	sandboxConfig := newTestSandboxConfigNoop()
	sandboxConfig.Containers = nil

	return sandboxConfig
}

func TestCreateSandboxNoopAgentSuccessful(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}
	defer cleanUp()

	// Pre-create the directory path to avoid panic error. Without this change, ff the test is run as a non-root user,
	// this test will fail because of permission denied error in chown syscall in the utils.MkdirAllWithInheritedOwner() method
	err := os.MkdirAll(fs.MockRunStoragePath(), DirMode)
	assert.NoError(err)

	config := newTestSandboxConfigNoop()

	ctx := WithNewAgentFunc(context.Background(), newMockAgent)
	p, err := CreateSandbox(ctx, config, nil, nil)
	assert.NoError(err)
	assert.NotNil(p)

	s, ok := p.(*Sandbox)
	assert.True(ok)
	assert.NotNil(s)

	sandboxDir := filepath.Join(s.store.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)
}

func TestCreateSandboxKataAgentSuccessful(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigKataAgent()

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	ctx := WithNewAgentFunc(context.Background(), newMockAgent)
	p, err := CreateSandbox(ctx, config, nil, nil)
	assert.NoError(err)
	assert.NotNil(p)

	s, ok := p.(*Sandbox)
	assert.True(ok)
	sandboxDir := filepath.Join(s.store.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)
}

func TestCreateSandboxFailing(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}
	defer cleanUp()
	assert := assert.New(t)

	config := SandboxConfig{}

	ctx := WithNewAgentFunc(context.Background(), newMockAgent)
	p, err := CreateSandbox(ctx, config, nil, nil)
	assert.Error(err)
	assert.Nil(p.(*Sandbox))
}

/*
 * Benchmarks
 */

func newTestContainerConfigNoop(contID string) ContainerConfig {
	// Define the container command and bundle.
	container := ContainerConfig{
		ID:          contID,
		RootFs:      RootFs{Target: filepath.Join(testDir, testBundle), Mounted: true},
		Cmd:         newBasicTestCmd(),
		Annotations: containerAnnotations,
		CustomSpec:  newEmptySpec(),
	}

	return container
}

// createAndStartSandbox handles the common test operation of creating and
// starting a sandbox.
func createAndStartSandbox(ctx context.Context, config SandboxConfig) (sandbox VCSandbox, sandboxDir string,
	err error) {

	// Create sandbox
	sandbox, err = CreateSandbox(ctx, config, nil, nil)
	if sandbox == nil || err != nil {
		return nil, "", err
	}

	s, ok := sandbox.(*Sandbox)
	if !ok {
		return nil, "", fmt.Errorf("Could not get Sandbox")
	}
	sandboxDir = filepath.Join(s.store.RunStoragePath(), sandbox.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		return nil, "", err
	}

	// Start sandbox
	err = sandbox.Start(ctx)
	if err != nil {
		return nil, "", err
	}

	return sandbox, sandboxDir, nil
}

func TestReleaseSandbox(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	ctx := WithNewAgentFunc(context.Background(), newMockAgent)
	s, err := CreateSandbox(ctx, config, nil, nil)
	assert.NoError(t, err)
	assert.NotNil(t, s)

	err = s.Release(ctx)
	assert.Nil(t, err, "sandbox release failed: %v", err)
}

func TestCleanupContainer(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	config := newTestSandboxConfigNoop()
	assert := assert.New(t)

	ctx := WithNewAgentFunc(context.Background(), newMockAgent)

	p, _, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contIDs := []string{"100", "101", "102", "103", "104"}
	for _, contID := range contIDs {
		contConfig := newTestContainerConfigNoop(contID)

		c, err := p.CreateContainer(ctx, contConfig)
		if c == nil || err != nil {
			t.Fatal(err)
		}

		c, err = p.StartContainer(context.Background(), c.ID())
		if c == nil || err != nil {
			t.Fatal(err)
		}
	}

	for _, c := range p.GetAllContainers() {
		CleanupContainer(ctx, p.ID(), c.ID(), true)
	}

	s, ok := p.(*Sandbox)
	assert.True(ok)
	sandboxDir := filepath.Join(s.store.RunStoragePath(), p.ID())

	_, err = os.Stat(sandboxDir)
	if err == nil {
		t.Fatal("sandbox dir should be deleted")
	}
}
