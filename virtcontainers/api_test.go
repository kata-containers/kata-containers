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
	"syscall"
	"testing"

	ktu "github.com/kata-containers/runtime/pkg/katatestutils"
	"github.com/kata-containers/runtime/virtcontainers/persist"
	"github.com/kata-containers/runtime/virtcontainers/persist/fs"
	"github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/mock"
	"github.com/kata-containers/runtime/virtcontainers/pkg/rootless"
	vcTypes "github.com/kata-containers/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/runtime/virtcontainers/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

const (
	containerID = "1"
)

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
			CgroupsPath: defaultCgroupPath,
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

func rmSandboxDir(sid string) error {
	store, err := persist.GetDriver("fs")
	if err != nil {
		return fmt.Errorf("failed to get fs persist driver: %v", err)
	}

	store.Destroy(sid)
	return nil
}

func newTestSandboxConfigNoop() SandboxConfig {
	bundlePath := filepath.Join(testDir, testBundle)
	containerAnnotations[annotations.BundlePathKey] = bundlePath
	// containerAnnotations["com.github.containers.virtcontainers.pkg.oci.container_type"] = "pod_sandbox"

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

		AgentType: NoopAgentType,

		Containers: []ContainerConfig{container},

		Annotations: sandboxAnnotations,

		ProxyType: NoopProxyType,
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
	sandboxConfig.AgentType = KataContainersAgent
	sandboxConfig.AgentConfig = KataAgentConfig{}
	sandboxConfig.Containers = nil

	return sandboxConfig
}

func TestCreateSandboxNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(context.Background(), config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
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

	sockDir, err := testGenerateKataProxySockDir()
	assert.NoError(err)

	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	noopProxyURL = testKataProxyURL

	impl := &gRPCProxy{}

	kataProxyMock := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}
	err = kataProxyMock.Start(testKataProxyURL)
	assert.NoError(err)
	defer kataProxyMock.Stop()

	p, err := CreateSandbox(context.Background(), config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)
}

func TestCreateSandboxFailing(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	config := SandboxConfig{}

	p, err := CreateSandbox(context.Background(), config, nil)
	assert.Error(err)
	assert.Nil(p.(*Sandbox))
}

func TestDeleteSandboxNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	ctx := context.Background()
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	p, err = DeleteSandbox(ctx, p.ID())
	assert.NoError(err)
	assert.NotNil(p)

	_, err = os.Stat(sandboxDir)
	assert.Error(err)
}

func TestDeleteSandboxKataAgentSuccessful(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigKataAgent()

	sockDir, err := testGenerateKataProxySockDir()
	assert.NoError(err)

	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	noopProxyURL = testKataProxyURL

	impl := &gRPCProxy{}

	kataProxyMock := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}
	err = kataProxyMock.Start(testKataProxyURL)
	assert.NoError(err)
	defer kataProxyMock.Stop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	p, err = DeleteSandbox(ctx, p.ID())
	assert.NoError(err)
	assert.NotNil(p)

	_, err = os.Stat(sandboxDir)
	assert.Error(err)
}

func TestDeleteSandboxFailing(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	sandboxDir := filepath.Join(fs.RunStoragePath(), testSandboxID)
	os.Remove(sandboxDir)

	p, err := DeleteSandbox(context.Background(), testSandboxID)
	assert.Error(err)
	assert.Nil(p)
}

func TestStartSandboxNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	config := newTestSandboxConfigNoop()

	p, _, err := createAndStartSandbox(context.Background(), config)
	assert.NoError(err)
	assert.NotNil(p)
}

func TestStartSandboxKataAgentSuccessful(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigKataAgent()

	sockDir, err := testGenerateKataProxySockDir()
	assert.NoError(err)
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	noopProxyURL = testKataProxyURL

	impl := &gRPCProxy{}

	kataProxyMock := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}
	err = kataProxyMock.Start(testKataProxyURL)
	assert.NoError(err)
	defer kataProxyMock.Stop()

	ctx := context.Background()
	p, _, err := createAndStartSandbox(ctx, config)
	assert.NoError(err)
	assert.NotNil(p)

	pImpl, ok := p.(*Sandbox)
	assert.True(ok)

	// TODO: defaultSharedDir is a hyper var = /run/hyper/shared/sandboxes
	// do we need to unmount sandboxes and containers?
	err = bindUnmountAllRootfs(ctx, testDir, pImpl)
	assert.NoError(err)
}

func TestStartSandboxFailing(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	sandboxDir := filepath.Join(fs.RunStoragePath(), testSandboxID)
	os.Remove(sandboxDir)

	p, err := StartSandbox(context.Background(), testSandboxID)
	assert.Error(err)
	assert.Nil(p)
}

func TestStopSandboxNoopAgentSuccessful(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}
	defer cleanUp()
	assert := assert.New(t)

	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, _, err := createAndStartSandbox(ctx, config)
	assert.NoError(err)
	assert.NotNil(p)

	vp, err := StopSandbox(ctx, p.ID(), false)
	assert.NoError(err)
	assert.NotNil(vp)
}

func TestStopSandboxKataAgentSuccessful(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigKataAgent()

	sockDir, err := testGenerateKataProxySockDir()
	assert.NoError(err)
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	noopProxyURL = testKataProxyURL

	impl := &gRPCProxy{}

	kataProxyMock := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}
	err = kataProxyMock.Start(testKataProxyURL)
	assert.NoError(err)
	defer kataProxyMock.Stop()

	ctx := context.Background()
	p, _, err := createAndStartSandbox(ctx, config)
	assert.NoError(err)
	assert.NotNil(p)

	p, err = StopSandbox(ctx, p.ID(), false)
	assert.NoError(err)
	assert.NotNil(p)
}

func TestStopSandboxFailing(t *testing.T) {
	defer cleanUp()

	sandboxDir := filepath.Join(fs.RunStoragePath(), testSandboxID)
	os.Remove(sandboxDir)

	p, err := StopSandbox(context.Background(), testSandboxID, false)
	assert.Error(t, err)
	assert.Nil(t, p)
}

func TestRunSandboxNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	config := newTestSandboxConfigNoop()

	p, err := RunSandbox(context.Background(), config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)
}

func TestRunSandboxKataAgentSuccessful(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigKataAgent()

	sockDir, err := testGenerateKataProxySockDir()
	assert.NoError(err)

	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	noopProxyURL = testKataProxyURL

	impl := &gRPCProxy{}

	kataProxyMock := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}
	err = kataProxyMock.Start(testKataProxyURL)
	assert.NoError(err)
	defer kataProxyMock.Stop()

	ctx := context.Background()
	p, err := RunSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	pImpl, ok := p.(*Sandbox)
	assert.True(ok)

	err = bindUnmountAllRootfs(ctx, testDir, pImpl)
	assert.NoError(err)
}

func TestRunSandboxFailing(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	config := SandboxConfig{}

	p, err := RunSandbox(context.Background(), config, nil)
	assert.Error(err)
	assert.Nil(p)
}

func TestListSandboxSuccessful(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	_, err = ListSandbox(ctx)
	assert.NoError(err)
}

func TestListSandboxNoSandboxDirectory(t *testing.T) {
	defer cleanUp()

	_, err := ListSandbox(context.Background())
	assert.NoError(t, err)
}

func TestStatusSandboxSuccessfulStateReady(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	config := newTestSandboxConfigNoop()
	cgroupPath, err := renameCgroupPath(defaultCgroupPath)
	assert.NoError(err)

	hypervisorConfig := HypervisorConfig{
		KernelPath:        filepath.Join(testDir, testKernel),
		ImagePath:         filepath.Join(testDir, testImage),
		HypervisorPath:    filepath.Join(testDir, testHypervisor),
		NumVCPUs:          defaultVCPUs,
		MemorySize:        defaultMemSzMiB,
		DefaultBridges:    defaultBridges,
		BlockDeviceDriver: defaultBlockDriver,
		DefaultMaxVCPUs:   defaultMaxQemuVCPUs,
		Msize9p:           defaultMsize9p,
	}

	expectedStatus := SandboxStatus{
		ID: testSandboxID,
		State: types.SandboxState{
			State:          types.StateReady,
			PersistVersion: 2,
		},
		Hypervisor:       MockHypervisor,
		HypervisorConfig: hypervisorConfig,
		Agent:            NoopAgentType,
		ContainersStatus: []ContainerStatus{
			{
				ID: containerID,
				State: types.ContainerState{
					State:      types.StateReady,
					CgroupPath: cgroupPath,
				},
				PID:         0,
				RootFs:      filepath.Join(testDir, testBundle),
				Annotations: containerAnnotations,
				Spec:        newEmptySpec(),
			},
		},
	}

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	status, err := StatusSandbox(ctx, p.ID())
	assert.NoError(err)

	// Copy the start time as we can't pretend we know what that
	// value will be.
	expectedStatus.ContainersStatus[0].StartTime = status.ContainersStatus[0].StartTime

	assert.Equal(status, expectedStatus)
}

func TestStatusSandboxSuccessfulStateRunning(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	config := newTestSandboxConfigNoop()
	cgroupPath, err := renameCgroupPath(defaultCgroupPath)
	assert.NoError(err)

	hypervisorConfig := HypervisorConfig{
		KernelPath:        filepath.Join(testDir, testKernel),
		ImagePath:         filepath.Join(testDir, testImage),
		HypervisorPath:    filepath.Join(testDir, testHypervisor),
		NumVCPUs:          defaultVCPUs,
		MemorySize:        defaultMemSzMiB,
		DefaultBridges:    defaultBridges,
		BlockDeviceDriver: defaultBlockDriver,
		DefaultMaxVCPUs:   defaultMaxQemuVCPUs,
		Msize9p:           defaultMsize9p,
	}

	expectedStatus := SandboxStatus{
		ID: testSandboxID,
		State: types.SandboxState{
			State:          types.StateRunning,
			PersistVersion: 2,
		},
		Hypervisor:       MockHypervisor,
		HypervisorConfig: hypervisorConfig,
		Agent:            NoopAgentType,
		ContainersStatus: []ContainerStatus{
			{
				ID: containerID,
				State: types.ContainerState{
					State:      types.StateRunning,
					CgroupPath: cgroupPath,
				},
				PID:         0,
				RootFs:      filepath.Join(testDir, testBundle),
				Annotations: containerAnnotations,
				Spec:        newEmptySpec(),
			},
		},
	}

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	p, err = StartSandbox(ctx, p.ID())
	assert.NoError(err)
	assert.NotNil(p)

	status, err := StatusSandbox(ctx, p.ID())
	assert.NoError(err)

	// Copy the start time as we can't pretend we know what that
	// value will be.
	expectedStatus.ContainersStatus[0].StartTime = status.ContainersStatus[0].StartTime

	assert.Exactly(status, expectedStatus)
}

func TestStatusSandboxFailingFetchSandboxConfig(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	rmSandboxDir(p.ID())
	globalSandboxList.removeSandbox(p.ID())

	_, err = StatusSandbox(ctx, p.ID())
	assert.Error(err)
}

func TestStatusPodSandboxFailingFetchSandboxState(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	rmSandboxDir(p.ID())
	globalSandboxList.removeSandbox(p.ID())

	_, err = StatusSandbox(ctx, p.ID())
	assert.Error(err)
}

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

func TestCreateContainerSuccessful(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)
}

func TestCreateContainerFailingNoSandbox(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	p, err = DeleteSandbox(ctx, p.ID())
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.Error(err)

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.Error(err)
	assert.Nil(c)
}

func TestDeleteContainerSuccessful(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	c, err = DeleteContainer(ctx, p.ID(), contID)
	assert.NoError(err)
	assert.NotNil(c)

	_, err = os.Stat(contDir)
	assert.Error(err)
}

func TestDeleteContainerFailingNoSandbox(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	c, err := DeleteContainer(context.Background(), testSandboxID, contID)
	assert.Error(err)
	assert.Nil(c)
}

func TestDeleteContainerFailingNoContainer(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	c, err := DeleteContainer(ctx, p.ID(), contID)
	assert.Error(err)
	assert.Nil(c)
}

func TestStartContainerNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	p, sandboxDir, err := createAndStartSandbox(ctx, config)
	assert.NoError(err)
	assert.NotNil(p)
	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	c, err = StartContainer(ctx, p.ID(), contID)
	assert.NoError(err)
	assert.NotNil(c)
}

func TestStartContainerFailingNoSandbox(t *testing.T) {
	defer cleanUp()

	contID := "100"
	c, err := StartContainer(context.Background(), testSandboxID, contID)
	assert.Error(t, err)
	assert.Nil(t, c)
}

func TestStartContainerFailingNoContainer(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	c, err := StartContainer(ctx, p.ID(), contID)
	assert.Error(err)
	assert.Nil(c)
}

func TestStartContainerFailingSandboxNotStarted(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	_, err = StartContainer(ctx, p.ID(), contID)
	assert.Error(err)
}

func TestStopContainerNoopAgentSuccessful(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	p, sandboxDir, err := createAndStartSandbox(ctx, config)
	assert.NoError(err)
	assert.NotNil(p)

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	c, err = StartContainer(ctx, p.ID(), contID)
	assert.NoError(err)
	assert.NotNil(c)

	c, err = StopContainer(ctx, p.ID(), contID)
	assert.NoError(err)
	assert.NotNil(c)
}

func TestStopContainerFailingNoSandbox(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}
	defer cleanUp()

	contID := "100"
	c, err := StopContainer(context.Background(), testSandboxID, contID)
	assert.Error(t, err)
	assert.Nil(t, c)
}

func TestStopContainerFailingNoContainer(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	c, err := StopContainer(ctx, p.ID(), contID)
	assert.Error(err)
	assert.Nil(c)
}

func testKillContainerFromContReadySuccessful(t *testing.T, signal syscall.Signal) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	p, sandboxDir, err := createAndStartSandbox(ctx, config)
	assert.NoError(err)
	assert.NotNil(p)

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	err = KillContainer(ctx, p.ID(), contID, signal, false)
	assert.NoError(err)
}

func TestKillContainerFromContReadySuccessful(t *testing.T) {
	// SIGUSR1
	testKillContainerFromContReadySuccessful(t, syscall.SIGUSR1)
	// SIGUSR2
	testKillContainerFromContReadySuccessful(t, syscall.SIGUSR2)
	// SIGKILL
	testKillContainerFromContReadySuccessful(t, syscall.SIGKILL)
	// SIGTERM
	testKillContainerFromContReadySuccessful(t, syscall.SIGTERM)
}

func TestEnterContainerNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	p, sandboxDir, err := createAndStartSandbox(ctx, config)
	assert.NoError(err)
	assert.NotNil(p)

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	c, err = StartContainer(ctx, p.ID(), contID)
	assert.NoError(err)
	assert.NotNil(c)

	cmd := newBasicTestCmd()

	_, c, _, err = EnterContainer(ctx, p.ID(), contID, cmd)
	assert.NoError(err)
	assert.NotNil(c)
}

func TestEnterContainerFailingNoSandbox(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)
	contID := "100"
	cmd := newBasicTestCmd()

	_, c, _, err := EnterContainer(context.Background(), testSandboxID, contID, cmd)
	assert.Error(err)
	assert.Nil(c)
}

func TestEnterContainerFailingNoContainer(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	cmd := newBasicTestCmd()

	_, c, _, err := EnterContainer(ctx, p.ID(), contID, cmd)
	assert.Error(err)
	assert.Nil(c)
}

func TestEnterContainerFailingContNotStarted(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	p, sandboxDir, err := createAndStartSandbox(ctx, config)
	assert.NoError(err)
	assert.NotNil(p)

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	cmd := newBasicTestCmd()

	_, c, _, err = EnterContainer(ctx, p.ID(), contID, cmd)
	assert.NoError(err)
	assert.NotNil(c)
}

func TestStatusContainerSuccessful(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	status, err := StatusContainer(ctx, p.ID(), contID)
	assert.NoError(err)

	pImpl, ok := p.(*Sandbox)
	assert.True(ok)

	cImpl, ok := c.(*Container)
	assert.True(ok)

	assert.True(status.StartTime.Equal(cImpl.process.StartTime))
	assert.Exactly(pImpl.config.Containers[0].Annotations, status.Annotations)
}

func TestStatusContainerStateReady(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	// (homage to a great album! ;)
	contID := "101"

	config := newTestSandboxConfigNoop()
	cgroupPath, err := renameCgroupPath(defaultCgroupPath)
	assert.NoError(err)

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	// fresh lookup
	p2, err := fetchSandbox(ctx, p.ID())
	assert.NoError(err)
	defer p2.releaseStatelessSandbox()

	expectedStatus := ContainerStatus{
		ID: contID,
		State: types.ContainerState{
			State:      types.StateReady,
			CgroupPath: cgroupPath,
		},
		PID:         0,
		RootFs:      filepath.Join(testDir, testBundle),
		Annotations: containerAnnotations,
		Spec:        newEmptySpec(),
	}

	defer p2.wg.Wait()

	status, err := statusContainer(p2, contID)
	assert.NoError(err)

	// Copy the start time as we can't pretend we know what that
	// value will be.
	expectedStatus.StartTime = status.StartTime

	assert.Exactly(status, expectedStatus)
}

func TestStatusContainerStateRunning(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	// (homage to a great album! ;)
	contID := "101"

	config := newTestSandboxConfigNoop()
	cgroupPath, err := renameCgroupPath(defaultCgroupPath)
	assert.NoError(err)

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	p, err = StartSandbox(ctx, p.ID())
	assert.NoError(err)
	assert.NotNil(p)

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	c, err = StartContainer(ctx, p.ID(), c.ID())
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	// fresh lookup
	p2, err := fetchSandbox(ctx, p.ID())
	assert.NoError(err)
	defer p2.releaseStatelessSandbox()

	expectedStatus := ContainerStatus{
		ID: contID,
		State: types.ContainerState{
			State:      types.StateRunning,
			CgroupPath: cgroupPath,
		},
		PID:         0,
		RootFs:      filepath.Join(testDir, testBundle),
		Annotations: containerAnnotations,
		Spec:        newEmptySpec(),
	}

	defer p2.wg.Wait()

	status, err := statusContainer(p2, contID)
	assert.NoError(err)

	// Copy the start time as we can't pretend we know what that
	// value will be.
	expectedStatus.StartTime = status.StartTime

	assert.Exactly(status, expectedStatus)
}

func TestStatusContainerFailing(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	rmSandboxDir(p.ID())
	globalSandboxList.removeSandbox(p.ID())

	_, err = StatusContainer(ctx, p.ID(), contID)
	assert.Error(err)
}

func TestStatsContainerFailing(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	rmSandboxDir(p.ID())
	globalSandboxList.removeSandbox(p.ID())

	_, err = StatsContainer(ctx, p.ID(), contID)
	assert.Error(err)
}

func TestStatsContainer(t *testing.T) {
	defer cleanUp()

	assert := assert.New(t)
	contID := "100"

	ctx := context.Background()
	_, err := StatsContainer(ctx, "", "")
	assert.Error(err)

	_, err = StatsContainer(ctx, "abc", "")
	assert.Error(err)

	_, err = StatsContainer(ctx, "abc", "abc")
	assert.Error(err)

	config := newTestSandboxConfigNoop()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	p, err = StartSandbox(ctx, p.ID())
	assert.NoError(err)
	assert.NotNil(p)

	pImpl, ok := p.(*Sandbox)
	assert.True(ok)

	contConfig := newTestContainerConfigNoop(contID)
	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	_, err = StatsContainer(ctx, pImpl.id, "xyz")
	assert.Error(err)

	_, err = StatsContainer(ctx, "xyz", contID)
	assert.Error(err)

	stats, err := StatsContainer(ctx, pImpl.id, contID)
	assert.NoError(err)
	assert.Equal(stats, ContainerStats{})
}

func TestProcessListContainer(t *testing.T) {
	defer cleanUp()

	assert := assert.New(t)

	contID := "abc"
	options := ProcessListOptions{
		Format: "json",
		Args:   []string{"-ef"},
	}

	ctx := context.Background()
	_, err := ProcessListContainer(ctx, "", "", options)
	assert.Error(err)

	_, err = ProcessListContainer(ctx, "xyz", "", options)
	assert.Error(err)

	_, err = ProcessListContainer(ctx, "xyz", "xyz", options)
	assert.Error(err)

	config := newTestSandboxConfigNoop()
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	pImpl, ok := p.(*Sandbox)
	assert.True(ok)

	contConfig := newTestContainerConfigNoop(contID)
	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	_, err = ProcessListContainer(ctx, pImpl.id, "xyz", options)
	assert.Error(err)

	_, err = ProcessListContainer(ctx, "xyz", contID, options)
	assert.Error(err)

	_, err = ProcessListContainer(ctx, pImpl.id, contID, options)
	// Sandbox not running, impossible to ps the container
	assert.Error(err)
}

/*
 * Benchmarks
 */

func createNewSandboxConfig(hType HypervisorType, aType AgentType, aConfig interface{}) SandboxConfig {
	hypervisorConfig := HypervisorConfig{
		KernelPath:     "/usr/share/kata-containers/vmlinux.container",
		ImagePath:      "/usr/share/kata-containers/kata-containers.img",
		HypervisorPath: "/usr/bin/qemu-system-x86_64",
	}

	netConfig := NetworkConfig{}

	return SandboxConfig{
		ID:               testSandboxID,
		HypervisorType:   hType,
		HypervisorConfig: hypervisorConfig,

		AgentType:   aType,
		AgentConfig: aConfig,

		NetworkConfig: netConfig,

		ProxyType: NoopProxyType,
	}
}

// createAndStartSandbox handles the common test operation of creating and
// starting a sandbox.
func createAndStartSandbox(ctx context.Context, config SandboxConfig) (sandbox VCSandbox, sandboxDir string,
	err error) {

	// Create sandbox
	sandbox, err = CreateSandbox(ctx, config, nil)
	if sandbox == nil || err != nil {
		return nil, "", err
	}

	sandboxDir = filepath.Join(fs.RunStoragePath(), sandbox.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		return nil, "", err
	}

	// Start sandbox
	sandbox, err = StartSandbox(ctx, sandbox.ID())
	if sandbox == nil || err != nil {
		return nil, "", err
	}

	return sandbox, sandboxDir, nil
}

func createStartStopDeleteSandbox(b *testing.B, sandboxConfig SandboxConfig) {
	ctx := context.Background()

	p, _, err := createAndStartSandbox(ctx, sandboxConfig)
	if p == nil || err != nil {
		b.Fatalf("Could not create and start sandbox: %s", err)
	}

	// Stop sandbox
	_, err = StopSandbox(ctx, p.ID(), false)
	if err != nil {
		b.Fatalf("Could not stop sandbox: %s", err)
	}

	// Delete sandbox
	_, err = DeleteSandbox(ctx, p.ID())
	if err != nil {
		b.Fatalf("Could not delete sandbox: %s", err)
	}
}

func BenchmarkCreateStartStopDeleteSandboxQemuHypervisorNoopAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		sandboxConfig := createNewSandboxConfig(QemuHypervisor, NoopAgentType, nil)
		createStartStopDeleteSandbox(b, sandboxConfig)
	}
}

func BenchmarkCreateStartStopDeleteSandboxMockHypervisorNoopAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		sandboxConfig := createNewSandboxConfig(MockHypervisor, NoopAgentType, nil)
		createStartStopDeleteSandbox(b, sandboxConfig)
	}
}

func TestFetchSandbox(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	s, err := CreateSandbox(ctx, config, nil)
	assert.NoError(t, err)
	assert.NotNil(t, s)

	fetched, err := FetchSandbox(ctx, s.ID())
	assert.Nil(t, err, "%v", err)
	assert.True(t, fetched != s, "fetched stateless sandboxes should not match")
}

func TestFetchStatefulSandbox(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	config.Stateful = true

	ctx := context.Background()

	s, err := CreateSandbox(ctx, config, nil)
	assert.NoError(t, err)
	assert.NotNil(t, s)

	fetched, err := FetchSandbox(ctx, s.ID())
	assert.Nil(t, err, "%v", err)
	assert.Equal(t, fetched, s, "fetched stateful sandboxed should match")
}

func TestFetchNonExistingSandbox(t *testing.T) {
	defer cleanUp()

	_, err := FetchSandbox(context.Background(), "some-non-existing-sandbox-name")
	assert.NotNil(t, err, "fetch non-existing sandbox should fail")
}

func TestReleaseSandbox(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	s, err := CreateSandbox(context.Background(), config, nil)
	assert.NoError(t, err)
	assert.NotNil(t, s)

	err = s.Release()
	assert.Nil(t, err, "sandbox release failed: %v", err)
}

func TestUpdateContainer(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	ctx := context.Background()

	period := uint64(1000)
	quota := int64(2000)
	memoryLimit := int64(1073741824)
	memorySwap := int64(1073741824)
	assert := assert.New(t)
	resources := specs.LinuxResources{
		CPU: &specs.LinuxCPU{
			Period: &period,
			Quota:  &quota,
		},
		Memory: &specs.LinuxMemory{
			Limit: &memoryLimit,
			Swap:  &memorySwap,
		},
	}
	err := UpdateContainer(ctx, "", "", resources)
	assert.Error(err)

	err = UpdateContainer(ctx, "abc", "", resources)
	assert.Error(err)

	contID := "100"
	config := newTestSandboxConfigNoop()

	s, sandboxDir, err := createAndStartSandbox(ctx, config)
	assert.NoError(err)
	assert.NotNil(s)

	contConfig := newTestContainerConfigNoop(contID)
	_, c, err := CreateContainer(ctx, s.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	_, err = StartContainer(ctx, s.ID(), contID)
	assert.NoError(err)

	err = UpdateContainer(ctx, s.ID(), contID, resources)
	assert.NoError(err)
}

func TestPauseResumeContainer(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	ctx := context.Background()

	assert := assert.New(t)
	err := PauseContainer(ctx, "", "")
	assert.Error(err)

	err = PauseContainer(ctx, "abc", "")
	assert.Error(err)

	contID := "100"
	config := newTestSandboxConfigNoop()

	s, sandboxDir, err := createAndStartSandbox(ctx, config)
	assert.NoError(err)
	assert.NotNil(s)

	contConfig := newTestContainerConfigNoop(contID)
	_, c, err := CreateContainer(ctx, s.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	_, err = StartContainer(ctx, s.ID(), contID)
	assert.NoError(err)

	err = PauseContainer(ctx, s.ID(), contID)
	assert.NoError(err)

	err = ResumeContainer(ctx, s.ID(), contID)
	assert.NoError(err)
}

func TestNetworkOperation(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	assert := assert.New(t)
	inf := &vcTypes.Interface{
		Name:   "eno1",
		Mtu:    1500,
		HwAddr: "02:00:ca:fe:00:48",
	}
	ip := vcTypes.IPAddress{
		Family:  0,
		Address: "192.168.0.101",
		Mask:    "24",
	}
	inf.IPAddresses = append(inf.IPAddresses, &ip)

	ctx := context.Background()

	_, err := AddInterface(ctx, "", inf)
	assert.Error(err)

	_, err = AddInterface(ctx, "abc", inf)
	assert.Error(err)

	netNSPath, err := createNetNS()
	assert.NoError(err)
	defer deleteNetNS(netNSPath)

	config := newTestSandboxConfigNoop()
	config.NetworkConfig = NetworkConfig{
		NetNSPath: netNSPath,
	}

	s, _, err := createAndStartSandbox(ctx, config)
	assert.NoError(err)
	assert.NotNil(s)

	_, err = AddInterface(ctx, s.ID(), inf)
	assert.Error(err)

	_, err = RemoveInterface(ctx, s.ID(), inf)
	assert.NoError(err)

	_, err = ListInterfaces(ctx, s.ID())
	assert.NoError(err)

	_, err = UpdateRoutes(ctx, s.ID(), nil)
	assert.NoError(err)

	_, err = ListRoutes(ctx, s.ID())
	assert.NoError(err)
}

func TestCleanupContainer(t *testing.T) {
	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	p, _, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contIDs := []string{"100", "101", "102", "103", "104"}
	for _, contID := range contIDs {
		contConfig := newTestContainerConfigNoop(contID)

		c, err := p.CreateContainer(contConfig)
		if c == nil || err != nil {
			t.Fatal(err)
		}

		c, err = p.StartContainer(c.ID())
		if c == nil || err != nil {
			t.Fatal(err)
		}
	}

	for _, c := range p.GetAllContainers() {
		CleanupContainer(ctx, p.ID(), c.ID(), true)
	}

	sandboxDir := filepath.Join(fs.RunStoragePath(), p.ID())

	_, err = os.Stat(sandboxDir)
	if err == nil {
		t.Fatal(err)
	}
}
