// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"syscall"
	"testing"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/mock"
	vcTypes "github.com/kata-containers/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/runtime/virtcontainers/store"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/kata-containers/runtime/virtcontainers/utils"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

const (
	containerID = "1"
)

var sandboxAnnotations = map[string]string{
	"sandbox.foo":             "sandbox.bar",
	"sandbox.hello":           "sandbox.world",
	annotations.ConfigJSONKey: `{"linux":{"resources":{}}}`,
}

var containerAnnotations = map[string]string{
	"container.foo":           "container.bar",
	"container.hello":         "container.world",
	annotations.ConfigJSONKey: `{"linux":{"resources":{}}}`,
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
	// Define the container command and bundle.
	container := ContainerConfig{
		ID:          containerID,
		RootFs:      filepath.Join(testDir, testBundle),
		Cmd:         newBasicTestCmd(),
		Annotations: containerAnnotations,
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

	return sandboxConfig
}

func newTestSandboxConfigHyperstartAgent() SandboxConfig {
	// Define the container command and bundle.
	container := ContainerConfig{
		ID:          containerID,
		RootFs:      filepath.Join(testDir, testBundle),
		Cmd:         newBasicTestCmd(),
		Annotations: containerAnnotations,
	}

	// Sets the hypervisor configuration.
	hypervisorConfig := HypervisorConfig{
		KernelPath:     filepath.Join(testDir, testKernel),
		ImagePath:      filepath.Join(testDir, testImage),
		HypervisorPath: filepath.Join(testDir, testHypervisor),
	}

	agentConfig := HyperConfig{
		SockCtlName: testHyperstartCtlSocket,
		SockTtyName: testHyperstartTtySocket,
	}

	sandboxConfig := SandboxConfig{
		ID:               testSandboxID,
		HypervisorType:   MockHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   HyperstartAgent,
		AgentConfig: agentConfig,

		Containers:  []ContainerConfig{container},
		Annotations: sandboxAnnotations,

		ProxyType: NoopProxyType,
	}

	return sandboxConfig
}

func newTestSandboxConfigHyperstartAgentDefaultNetwork() SandboxConfig {
	// Define the container command and bundle.
	container := ContainerConfig{
		ID:          containerID,
		RootFs:      filepath.Join(testDir, testBundle),
		Cmd:         newBasicTestCmd(),
		Annotations: containerAnnotations,
	}

	// Sets the hypervisor configuration.
	hypervisorConfig := HypervisorConfig{
		KernelPath:     filepath.Join(testDir, testKernel),
		ImagePath:      filepath.Join(testDir, testImage),
		HypervisorPath: filepath.Join(testDir, testHypervisor),
	}

	agentConfig := HyperConfig{
		SockCtlName: testHyperstartCtlSocket,
		SockTtyName: testHyperstartTtySocket,
	}

	netConfig := NetworkConfig{}

	sandboxConfig := SandboxConfig{
		ID: testSandboxID,

		HypervisorType:   MockHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   HyperstartAgent,
		AgentConfig: agentConfig,

		NetworkConfig: netConfig,

		Containers:  []ContainerConfig{container},
		Annotations: sandboxAnnotations,

		ProxyType: NoopProxyType,
	}

	return sandboxConfig
}

func newTestSandboxConfigKataAgent() SandboxConfig {
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

		AgentType: KataContainersAgent,

		Annotations: sandboxAnnotations,

		ProxyType: NoopProxyType,
	}

	return sandboxConfig
}

func TestCreateSandboxNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(context.Background(), config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}
}

var testCCProxySockPathTempl = "%s/cc-proxy-test.sock"
var testCCProxyURLUnixScheme = "unix://"

func testGenerateCCProxySockDir() (string, error) {
	dir, err := ioutil.TempDir("", "cc-proxy-test")
	if err != nil {
		return "", err
	}

	return dir, nil
}

func TestCreateSandboxHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigHyperstartAgent()

	sockDir, err := testGenerateCCProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testCCProxySockPath := fmt.Sprintf(testCCProxySockPathTempl, sockDir)
	noopProxyURL = testCCProxyURLUnixScheme + testCCProxySockPath
	proxy := mock.NewCCProxyMock(t, testCCProxySockPath)
	proxy.Start()
	defer proxy.Stop()

	p, err := CreateSandbox(context.Background(), config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}
}

func TestCreateSandboxKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigKataAgent()

	sockDir, err := testGenerateKataProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	noopProxyURL = testKataProxyURL

	impl := &gRPCProxy{}

	kataProxyMock := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}
	if err := kataProxyMock.Start(testKataProxyURL); err != nil {
		t.Fatal(err)
	}
	defer kataProxyMock.Stop()

	p, err := CreateSandbox(context.Background(), config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}
}

func TestCreateSandboxFailing(t *testing.T) {
	defer cleanUp()

	config := SandboxConfig{}

	p, err := CreateSandbox(context.Background(), config, nil)
	if p.(*Sandbox) != nil || err == nil {
		t.Fatal()
	}
}

func TestDeleteSandboxNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()

	ctx := context.Background()
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	p, err = DeleteSandbox(ctx, p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(sandboxDir)
	if err == nil {
		t.Fatal()
	}
}

func TestDeleteSandboxHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigHyperstartAgent()

	sockDir, err := testGenerateCCProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testCCProxySockPath := fmt.Sprintf(testCCProxySockPathTempl, sockDir)
	noopProxyURL = testCCProxyURLUnixScheme + testCCProxySockPath
	proxy := mock.NewCCProxyMock(t, testCCProxySockPath)
	proxy.Start()
	defer proxy.Stop()

	ctx := context.Background()

	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	p, err = DeleteSandbox(ctx, p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(sandboxDir)
	if err == nil {
		t.Fatal(err)
	}
}

func TestDeleteSandboxKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigKataAgent()

	sockDir, err := testGenerateKataProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	noopProxyURL = testKataProxyURL

	impl := &gRPCProxy{}

	kataProxyMock := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}
	if err := kataProxyMock.Start(testKataProxyURL); err != nil {
		t.Fatal(err)
	}
	defer kataProxyMock.Stop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	p, err = DeleteSandbox(ctx, p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(sandboxDir)
	if err == nil {
		t.Fatal(err)
	}
}

func TestDeleteSandboxFailing(t *testing.T) {
	defer cleanUp()

	sandboxDir := store.SandboxConfigurationRootPath(testSandboxID)
	os.Remove(sandboxDir)

	p, err := DeleteSandbox(context.Background(), testSandboxID)
	if p != nil || err == nil {
		t.Fatal()
	}
}

func TestStartSandboxNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	p, _, err := createAndStartSandbox(context.Background(), config)
	if p == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStartSandboxHyperstartAgentSuccessful(t *testing.T) {
	defer cleanUp()

	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	config := newTestSandboxConfigHyperstartAgent()

	sockDir, err := testGenerateCCProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testCCProxySockPath := fmt.Sprintf(testCCProxySockPathTempl, sockDir)
	noopProxyURL = testCCProxyURLUnixScheme + testCCProxySockPath
	proxy := mock.NewCCProxyMock(t, testCCProxySockPath)
	proxy.Start()
	defer proxy.Stop()

	hyperConfig := config.AgentConfig.(HyperConfig)
	config.AgentConfig = hyperConfig

	ctx := context.Background()
	p, _, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	bindUnmountAllRootfs(ctx, defaultSharedDir, pImpl)
}

func TestStartSandboxKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigKataAgent()

	sockDir, err := testGenerateKataProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	noopProxyURL = testKataProxyURL

	impl := &gRPCProxy{}

	kataProxyMock := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}
	if err := kataProxyMock.Start(testKataProxyURL); err != nil {
		t.Fatal(err)
	}
	defer kataProxyMock.Stop()

	ctx := context.Background()
	p, _, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	bindUnmountAllRootfs(ctx, defaultSharedDir, pImpl)
}

func TestStartSandboxFailing(t *testing.T) {
	defer cleanUp()

	sandboxDir := store.SandboxConfigurationRootPath(testSandboxID)
	os.Remove(sandboxDir)

	p, err := StartSandbox(context.Background(), testSandboxID)
	if p != nil || err == nil {
		t.Fatal()
	}
}

func TestStopSandboxNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, _, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	vp, err := StopSandbox(ctx, p.ID())
	if vp == nil || err != nil {
		t.Fatal(err)
	}
}

func TestPauseThenResumeSandboxNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	p, _, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contID := "100"
	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	p, err = PauseSandbox(ctx, p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	expectedState := types.StatePaused

	assert.Equal(t, pImpl.state.State, expectedState, "unexpected paused sandbox state")

	for i, c := range p.GetAllContainers() {
		cImpl, ok := c.(*Container)
		assert.True(t, ok)

		assert.Equal(t, expectedState, cImpl.state.State,
			fmt.Sprintf("paused container %d has unexpected state", i))
	}

	p, err = ResumeSandbox(ctx, p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok = p.(*Sandbox)
	assert.True(t, ok)

	expectedState = types.StateRunning

	assert.Equal(t, pImpl.state.State, expectedState, "unexpected resumed sandbox state")

	for i, c := range p.GetAllContainers() {
		cImpl, ok := c.(*Container)
		assert.True(t, ok)

		assert.Equal(t, cImpl.state.State, expectedState,
			fmt.Sprintf("resumed container %d has unexpected state", i))
	}
}

func TestStopSandboxHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigHyperstartAgent()

	sockDir, err := testGenerateCCProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testCCProxySockPath := fmt.Sprintf(testCCProxySockPathTempl, sockDir)
	noopProxyURL = testCCProxyURLUnixScheme + testCCProxySockPath
	proxy := mock.NewCCProxyMock(t, testCCProxySockPath)
	proxy.Start()
	defer proxy.Stop()

	hyperConfig := config.AgentConfig.(HyperConfig)
	config.AgentConfig = hyperConfig

	ctx := context.Background()
	p, _, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StopSandbox(ctx, p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStopSandboxKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigKataAgent()

	sockDir, err := testGenerateKataProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	noopProxyURL = testKataProxyURL

	impl := &gRPCProxy{}

	kataProxyMock := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}
	if err := kataProxyMock.Start(testKataProxyURL); err != nil {
		t.Fatal(err)
	}
	defer kataProxyMock.Stop()

	ctx := context.Background()
	p, _, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StopSandbox(ctx, p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStopSandboxFailing(t *testing.T) {
	defer cleanUp()

	sandboxDir := store.SandboxConfigurationRootPath(testSandboxID)
	os.Remove(sandboxDir)

	p, err := StopSandbox(context.Background(), testSandboxID)
	if p != nil || err == nil {
		t.Fatal()
	}
}

func TestRunSandboxNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	p, err := RunSandbox(context.Background(), config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}
}

func TestRunSandboxHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigHyperstartAgent()

	sockDir, err := testGenerateCCProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testCCProxySockPath := fmt.Sprintf(testCCProxySockPathTempl, sockDir)
	noopProxyURL = testCCProxyURLUnixScheme + testCCProxySockPath
	proxy := mock.NewCCProxyMock(t, testCCProxySockPath)
	proxy.Start()
	defer proxy.Stop()

	hyperConfig := config.AgentConfig.(HyperConfig)
	config.AgentConfig = hyperConfig

	ctx := context.Background()
	p, err := RunSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	bindUnmountAllRootfs(ctx, defaultSharedDir, pImpl)
}

func TestRunSandboxKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigKataAgent()

	sockDir, err := testGenerateKataProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	noopProxyURL = testKataProxyURL

	impl := &gRPCProxy{}

	kataProxyMock := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}
	if err := kataProxyMock.Start(testKataProxyURL); err != nil {
		t.Fatal(err)
	}
	defer kataProxyMock.Stop()

	ctx := context.Background()
	p, err := RunSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	bindUnmountAllRootfs(ctx, defaultSharedDir, pImpl)
}

func TestRunSandboxFailing(t *testing.T) {
	defer cleanUp()

	config := SandboxConfig{}

	p, err := RunSandbox(context.Background(), config, nil)
	if p != nil || err == nil {
		t.Fatal()
	}
}

func TestListSandboxSuccessful(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	_, err = ListSandbox(ctx)
	if err != nil {
		t.Fatal(err)
	}
}

func TestListSandboxNoSandboxDirectory(t *testing.T) {
	defer cleanUp()

	_, err := ListSandbox(context.Background())
	if err != nil {
		t.Fatal(fmt.Sprintf("unexpected ListSandbox error from non-existent sandbox directory: %v", err))
	}
}

func TestStatusSandboxSuccessfulStateReady(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()
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
		State: types.State{
			State: types.StateReady,
		},
		Hypervisor:       MockHypervisor,
		HypervisorConfig: hypervisorConfig,
		Agent:            NoopAgentType,
		Annotations:      sandboxAnnotations,
		ContainersStatus: []ContainerStatus{
			{
				ID: containerID,
				State: types.State{
					State:      types.StateReady,
					CgroupPath: utils.DefaultCgroupPath,
				},
				PID:         0,
				RootFs:      filepath.Join(testDir, testBundle),
				Annotations: containerAnnotations,
			},
		},
	}

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	status, err := StatusSandbox(ctx, p.ID())
	if err != nil {
		t.Fatal(err)
	}

	// Copy the start time as we can't pretend we know what that
	// value will be.
	expectedStatus.ContainersStatus[0].StartTime = status.ContainersStatus[0].StartTime

	if reflect.DeepEqual(status, expectedStatus) == false {
		t.Fatalf("Got sandbox status %v\n expecting %v", status, expectedStatus)
	}
}

func TestStatusSandboxSuccessfulStateRunning(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()
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
		State: types.State{
			State: types.StateRunning,
		},
		Hypervisor:       MockHypervisor,
		HypervisorConfig: hypervisorConfig,
		Agent:            NoopAgentType,
		Annotations:      sandboxAnnotations,
		ContainersStatus: []ContainerStatus{
			{
				ID: containerID,
				State: types.State{
					State:      types.StateRunning,
					CgroupPath: utils.DefaultCgroupPath,
				},
				PID:         0,
				RootFs:      filepath.Join(testDir, testBundle),
				Annotations: containerAnnotations,
			},
		},
	}

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StartSandbox(ctx, p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	status, err := StatusSandbox(ctx, p.ID())
	if err != nil {
		t.Fatal(err)
	}

	// Copy the start time as we can't pretend we know what that
	// value will be.
	expectedStatus.ContainersStatus[0].StartTime = status.ContainersStatus[0].StartTime

	if reflect.DeepEqual(status, expectedStatus) == false {
		t.Fatalf("Got sandbox status %v\n expecting %v", status, expectedStatus)
	}
}

func TestStatusSandboxFailingFetchSandboxConfig(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	store.DeleteAll()
	globalSandboxList.removeSandbox(p.ID())

	_, err = StatusSandbox(ctx, p.ID())
	if err == nil {
		t.Fatal()
	}
}

func TestStatusPodSandboxFailingFetchSandboxState(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	store.DeleteAll()
	globalSandboxList.removeSandbox(p.ID())

	_, err = StatusSandbox(ctx, p.ID())
	if err == nil {
		t.Fatal()
	}
}

func newTestContainerConfigNoop(contID string) ContainerConfig {
	// Define the container command and bundle.
	container := ContainerConfig{
		ID:          contID,
		RootFs:      filepath.Join(testDir, testBundle),
		Cmd:         newBasicTestCmd(),
		Annotations: containerAnnotations,
	}

	return container
}

func TestCreateContainerSuccessful(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}
}

func TestCreateContainerFailingNoSandbox(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = DeleteSandbox(ctx, p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err == nil {
		t.Fatal()
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c != nil || err == nil {
		t.Fatal(err)
	}
}

func TestDeleteContainerSuccessful(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err = DeleteContainer(ctx, p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(contDir)
	if err == nil {
		t.Fatal()
	}
}

func TestDeleteContainerFailingNoSandbox(t *testing.T) {
	defer cleanUp()

	contID := "100"
	c, err := DeleteContainer(context.Background(), testSandboxID, contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestDeleteContainerFailingNoContainer(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err := DeleteContainer(ctx, p.ID(), contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestStartContainerNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	p, sandboxDir, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}
	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err = StartContainer(ctx, p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStartContainerFailingNoSandbox(t *testing.T) {
	defer cleanUp()

	contID := "100"
	c, err := StartContainer(context.Background(), testSandboxID, contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestStartContainerFailingNoContainer(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err := StartContainer(ctx, p.ID(), contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestStartContainerFailingSandboxNotStarted(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	_, err = StartContainer(ctx, p.ID(), contID)
	if err == nil {
		t.Fatal("Function should have failed")
	}
}

func TestStopContainerNoopAgentSuccessful(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	p, sandboxDir, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err = StartContainer(ctx, p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	c, err = StopContainer(ctx, p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStartStopContainerHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigHyperstartAgent()

	sockDir, err := testGenerateCCProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testCCProxySockPath := fmt.Sprintf(testCCProxySockPathTempl, sockDir)
	noopProxyURL = testCCProxyURLUnixScheme + testCCProxySockPath
	proxy := mock.NewCCProxyMock(t, testCCProxySockPath)
	proxy.Start()
	defer proxy.Stop()

	hyperConfig := config.AgentConfig.(HyperConfig)
	config.AgentConfig = hyperConfig

	ctx := context.Background()

	p, sandboxDir, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err = StartContainer(ctx, p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	c, err = StopContainer(ctx, p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	bindUnmountAllRootfs(ctx, defaultSharedDir, pImpl)
}

func TestStartStopSandboxHyperstartAgentSuccessfulWithDefaultNetwork(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigHyperstartAgentDefaultNetwork()

	n, err := ns.NewNS()
	if err != nil {
		t.Fatal(err)
	}
	defer n.Close()

	config.NetworkConfig.NetNSPath = n.Path()
	config.NetworkConfig.NetNsCreated = true

	sockDir, err := testGenerateCCProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testCCProxySockPath := fmt.Sprintf(testCCProxySockPathTempl, sockDir)
	noopProxyURL = testCCProxyURLUnixScheme + testCCProxySockPath
	proxy := mock.NewCCProxyMock(t, testCCProxySockPath)
	proxy.Start()
	defer proxy.Stop()

	hyperConfig := config.AgentConfig.(HyperConfig)
	config.AgentConfig = hyperConfig

	ctx := context.Background()

	p, _, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	v, err := StopSandbox(ctx, p.ID())
	if v == nil || err != nil {
		t.Fatal(err)
	}

	v, err = DeleteSandbox(ctx, p.ID())
	if v == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStopContainerFailingNoSandbox(t *testing.T) {
	defer cleanUp()

	contID := "100"
	c, err := StopContainer(context.Background(), testSandboxID, contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestStopContainerFailingNoContainer(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err := StopContainer(ctx, p.ID(), contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func testKillContainerFromContReadySuccessful(t *testing.T, signal syscall.Signal) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	p, sandboxDir, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	if err := KillContainer(ctx, p.ID(), contID, signal, false); err != nil {
		t.Fatal()
	}
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

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	p, sandboxDir, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err = StartContainer(ctx, p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	cmd := newBasicTestCmd()

	_, c, _, err = EnterContainer(ctx, p.ID(), contID, cmd)
	if c == nil || err != nil {
		t.Fatal(err)
	}
}

func TestEnterContainerHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigHyperstartAgent()

	sockDir, err := testGenerateCCProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testCCProxySockPath := fmt.Sprintf(testCCProxySockPathTempl, sockDir)
	noopProxyURL = testCCProxyURLUnixScheme + testCCProxySockPath
	proxy := mock.NewCCProxyMock(t, testCCProxySockPath)
	proxy.Start()
	defer proxy.Stop()

	hyperConfig := config.AgentConfig.(HyperConfig)
	config.AgentConfig = hyperConfig

	ctx := context.Background()

	p, sandboxDir, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, _, err = CreateContainer(ctx, p.ID(), contConfig)
	if err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	_, err = StartContainer(ctx, p.ID(), contID)
	if err != nil {
		t.Fatal(err)
	}

	cmd := newBasicTestCmd()

	_, _, _, err = EnterContainer(ctx, p.ID(), contID, cmd)
	if err != nil {
		t.Fatal(err)
	}

	_, err = StopContainer(ctx, p.ID(), contID)
	if err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	bindUnmountAllRootfs(ctx, defaultSharedDir, pImpl)
}

func TestEnterContainerFailingNoSandbox(t *testing.T) {
	defer cleanUp()
	contID := "100"
	cmd := newBasicTestCmd()

	_, c, _, err := EnterContainer(context.Background(), testSandboxID, contID, cmd)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestEnterContainerFailingNoContainer(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	cmd := newBasicTestCmd()

	_, c, _, err := EnterContainer(ctx, p.ID(), contID, cmd)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestEnterContainerFailingContNotStarted(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	p, sandboxDir, err := createAndStartSandbox(ctx, config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	cmd := newBasicTestCmd()

	_, c, _, err = EnterContainer(ctx, p.ID(), contID, cmd)
	if c == nil || err != nil {
		t.Fatal()
	}
}

func TestStatusContainerSuccessful(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	status, err := StatusContainer(ctx, p.ID(), contID)
	if err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	cImpl, ok := c.(*Container)
	assert.True(t, ok)

	if status.StartTime.Equal(cImpl.process.StartTime) == false {
		t.Fatalf("Got container start time %v, expecting %v", status.StartTime, cImpl.process.StartTime)
	}

	if reflect.DeepEqual(pImpl.config.Containers[0].Annotations, status.Annotations) == false {
		t.Fatalf("Got annotations %v\n expecting %v", status.Annotations, pImpl.config.Containers[0].Annotations)
	}
}

func TestStatusContainerStateReady(t *testing.T) {
	defer cleanUp()

	// (homage to a great album! ;)
	contID := "101"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	// fresh lookup
	p2, err := fetchSandbox(ctx, p.ID())
	if err != nil {
		t.Fatal(err)
	}
	defer p2.releaseStatelessSandbox()

	expectedStatus := ContainerStatus{
		ID: contID,
		State: types.State{
			State:      types.StateReady,
			CgroupPath: utils.DefaultCgroupPath,
		},
		PID:         0,
		RootFs:      filepath.Join(testDir, testBundle),
		Annotations: containerAnnotations,
	}

	defer p2.wg.Wait()

	status, err := statusContainer(p2, contID)
	if err != nil {
		t.Fatal(err)
	}

	// Copy the start time as we can't pretend we know what that
	// value will be.
	expectedStatus.StartTime = status.StartTime

	if reflect.DeepEqual(status, expectedStatus) == false {
		t.Fatalf("Got container status %v, expected %v", status, expectedStatus)
	}
}

func TestStatusContainerStateRunning(t *testing.T) {
	defer cleanUp()

	// (homage to a great album! ;)
	contID := "101"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StartSandbox(ctx, p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := store.SandboxConfigurationRootPath(p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	c, err = StartContainer(ctx, p.ID(), c.ID())
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	// fresh lookup
	p2, err := fetchSandbox(ctx, p.ID())
	if err != nil {
		t.Fatal(err)
	}
	defer p2.releaseStatelessSandbox()

	expectedStatus := ContainerStatus{
		ID: contID,
		State: types.State{
			State:      types.StateRunning,
			CgroupPath: utils.DefaultCgroupPath,
		},
		PID:         0,
		RootFs:      filepath.Join(testDir, testBundle),
		Annotations: containerAnnotations,
	}

	defer p2.wg.Wait()

	status, err := statusContainer(p2, contID)
	if err != nil {
		t.Fatal(err)
	}

	// Copy the start time as we can't pretend we know what that
	// value will be.
	expectedStatus.StartTime = status.StartTime

	if reflect.DeepEqual(status, expectedStatus) == false {
		t.Fatalf("Got container status %v, expected %v", status, expectedStatus)
	}
}

func TestStatusContainerFailing(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	store.DeleteAll()
	globalSandboxList.removeSandbox(p.ID())

	_, err = StatusContainer(ctx, p.ID(), contID)
	if err == nil {
		t.Fatal()
	}
}

func TestStatsContainerFailing(t *testing.T) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	store.DeleteAll()
	globalSandboxList.removeSandbox(p.ID())

	_, err = StatsContainer(ctx, p.ID(), contID)
	if err == nil {
		t.Fatal()
	}
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
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(ok)
	defer store.DeleteAll()

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
	defer store.DeleteAll()

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

func createNewContainerConfigs(numOfContainers int) []ContainerConfig {
	var contConfigs []ContainerConfig

	envs := []types.EnvVar{
		{
			Var:   "PATH",
			Value: "/bin:/usr/bin:/sbin:/usr/sbin",
		},
	}

	cmd := types.Cmd{
		Args:    strings.Split("/bin/ps -A", " "),
		Envs:    envs,
		WorkDir: "/",
	}

	_, thisFile, _, ok := runtime.Caller(0)
	if ok == false {
		return nil
	}

	rootFs := filepath.Dir(thisFile) + "/utils/supportfiles/bundles/busybox/"

	for i := 0; i < numOfContainers; i++ {
		contConfig := ContainerConfig{
			ID:     fmt.Sprintf("%d", i),
			RootFs: rootFs,
			Cmd:    cmd,
		}

		contConfigs = append(contConfigs, contConfig)
	}

	return contConfigs
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

	sandboxDir = store.SandboxConfigurationRootPath(sandbox.ID())
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
	_, err = StopSandbox(ctx, p.ID())
	if err != nil {
		b.Fatalf("Could not stop sandbox: %s", err)
	}

	// Delete sandbox
	_, err = DeleteSandbox(ctx, p.ID())
	if err != nil {
		b.Fatalf("Could not delete sandbox: %s", err)
	}
}

func createStartStopDeleteContainers(b *testing.B, sandboxConfig SandboxConfig, contConfigs []ContainerConfig) {
	ctx := context.Background()

	// Create sandbox
	p, err := CreateSandbox(ctx, sandboxConfig, nil)
	if err != nil {
		b.Fatalf("Could not create sandbox: %s", err)
	}

	// Start sandbox
	_, err = StartSandbox(ctx, p.ID())
	if err != nil {
		b.Fatalf("Could not start sandbox: %s", err)
	}

	// Create containers
	for _, contConfig := range contConfigs {
		_, _, err := CreateContainer(ctx, p.ID(), contConfig)
		if err != nil {
			b.Fatalf("Could not create container %s: %s", contConfig.ID, err)
		}
	}

	// Start containers
	for _, contConfig := range contConfigs {
		_, err := StartContainer(ctx, p.ID(), contConfig.ID)
		if err != nil {
			b.Fatalf("Could not start container %s: %s", contConfig.ID, err)
		}
	}

	// Stop containers
	for _, contConfig := range contConfigs {
		_, err := StopContainer(ctx, p.ID(), contConfig.ID)
		if err != nil {
			b.Fatalf("Could not stop container %s: %s", contConfig.ID, err)
		}
	}

	// Delete containers
	for _, contConfig := range contConfigs {
		_, err := DeleteContainer(ctx, p.ID(), contConfig.ID)
		if err != nil {
			b.Fatalf("Could not delete container %s: %s", contConfig.ID, err)
		}
	}

	// Stop sandbox
	_, err = StopSandbox(ctx, p.ID())
	if err != nil {
		b.Fatalf("Could not stop sandbox: %s", err)
	}

	// Delete sandbox
	_, err = DeleteSandbox(ctx, p.ID())
	if err != nil {
		b.Fatalf("Could not delete sandbox: %s", err)
	}
}

func BenchmarkCreateStartStopDeleteSandboxQemuHypervisorHyperstartAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		sandboxConfig := createNewSandboxConfig(QemuHypervisor, HyperstartAgent, HyperConfig{})

		sockDir, err := testGenerateCCProxySockDir()
		if err != nil {
			b.Fatal(err)
		}
		defer os.RemoveAll(sockDir)

		var t testing.T
		testCCProxySockPath := fmt.Sprintf(testCCProxySockPathTempl, sockDir)
		noopProxyURL = testCCProxyURLUnixScheme + testCCProxySockPath
		proxy := mock.NewCCProxyMock(&t, testCCProxySockPath)
		proxy.Start()
		defer proxy.Stop()

		createStartStopDeleteSandbox(b, sandboxConfig)
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

func BenchmarkStartStop1ContainerQemuHypervisorHyperstartAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		sandboxConfig := createNewSandboxConfig(QemuHypervisor, HyperstartAgent, HyperConfig{})
		contConfigs := createNewContainerConfigs(1)

		sockDir, err := testGenerateCCProxySockDir()
		if err != nil {
			b.Fatal(err)
		}
		defer os.RemoveAll(sockDir)

		var t testing.T
		testCCProxySockPath := fmt.Sprintf(testCCProxySockPathTempl, sockDir)
		noopProxyURL = testCCProxyURLUnixScheme + testCCProxySockPath
		proxy := mock.NewCCProxyMock(&t, testCCProxySockPath)
		proxy.Start()
		defer proxy.Stop()

		createStartStopDeleteContainers(b, sandboxConfig, contConfigs)
	}
}

func BenchmarkStartStop10ContainerQemuHypervisorHyperstartAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		sandboxConfig := createNewSandboxConfig(QemuHypervisor, HyperstartAgent, HyperConfig{})
		contConfigs := createNewContainerConfigs(10)

		sockDir, err := testGenerateCCProxySockDir()
		if err != nil {
			b.Fatal(err)
		}
		defer os.RemoveAll(sockDir)

		var t testing.T
		testCCProxySockPath := fmt.Sprintf(testCCProxySockPathTempl, sockDir)
		noopProxyURL = testCCProxyURLUnixScheme + testCCProxySockPath
		proxy := mock.NewCCProxyMock(&t, testCCProxySockPath)
		proxy.Start()
		defer proxy.Stop()

		createStartStopDeleteContainers(b, sandboxConfig, contConfigs)
	}
}

func TestFetchSandbox(t *testing.T) {
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	ctx := context.Background()

	s, err := CreateSandbox(ctx, config, nil)
	if s == nil || err != nil {
		t.Fatal(err)
	}

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
	if s == nil || err != nil {
		t.Fatal(err)
	}

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
	if s == nil || err != nil {
		t.Fatal(err)
	}
	err = s.Release()
	assert.Nil(t, err, "sandbox release failed: %v", err)
}

func TestUpdateContainer(t *testing.T) {
	if os.Geteuid() != 0 {
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
	if os.Geteuid() != 0 {
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
	if os.Geteuid() != 0 {
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
