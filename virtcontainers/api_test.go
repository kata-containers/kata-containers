// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"syscall"
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/pkg/mock"
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

func newBasicTestCmd() Cmd {
	envs := []EnvVar{
		{
			Var:   "PATH",
			Value: "/bin:/usr/bin:/sbin:/usr/sbin",
		},
	}

	cmd := Cmd{
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
	}

	return sandboxConfig
}

func newTestSandboxConfigHyperstartAgentCNINetwork() SandboxConfig {
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

	netConfig := NetworkConfig{
		NumInterfaces: 1,
	}

	sandboxConfig := SandboxConfig{
		ID:               testSandboxID,
		HypervisorType:   MockHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   HyperstartAgent,
		AgentConfig: agentConfig,

		NetworkModel:  CNINetworkModel,
		NetworkConfig: netConfig,

		Containers:  []ContainerConfig{container},
		Annotations: sandboxAnnotations,
	}

	return sandboxConfig
}

func newTestSandboxConfigHyperstartAgentCNMNetwork() SandboxConfig {
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

	hooks := Hooks{
		PreStartHooks: []Hook{
			{
				Path: getMockHookBinPath(),
				Args: []string{testKeyHook, testContainerIDHook, testControllerIDHook},
			},
		},
		PostStartHooks: []Hook{},
		PostStopHooks:  []Hook{},
	}

	netConfig := NetworkConfig{
		NumInterfaces: len(hooks.PreStartHooks),
	}

	sandboxConfig := SandboxConfig{
		ID:    testSandboxID,
		Hooks: hooks,

		HypervisorType:   MockHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   HyperstartAgent,
		AgentConfig: agentConfig,

		NetworkModel:  CNMNetworkModel,
		NetworkConfig: netConfig,

		Containers:  []ContainerConfig{container},
		Annotations: sandboxAnnotations,
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
	}

	return sandboxConfig
}

func TestCreateSandboxNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
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

	cleanUp()

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

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}
}

func TestCreateSandboxKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

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

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}
}

func TestCreateSandboxFailing(t *testing.T) {
	cleanUp()

	config := SandboxConfig{}

	p, err := CreateSandbox(config, nil)
	if p.(*Sandbox) != nil || err == nil {
		t.Fatal()
	}
}

func TestDeleteSandboxNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	p, err = DeleteSandbox(p.ID())
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

	cleanUp()

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

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	p, err = DeleteSandbox(p.ID())
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

	cleanUp()

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

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	p, err = DeleteSandbox(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(sandboxDir)
	if err == nil {
		t.Fatal(err)
	}
}

func TestDeleteSandboxFailing(t *testing.T) {
	cleanUp()

	sandboxDir := filepath.Join(configStoragePath, testSandboxID)
	os.Remove(sandboxDir)

	p, err := DeleteSandbox(testSandboxID)
	if p != nil || err == nil {
		t.Fatal()
	}
}

func TestStartSandboxNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	config := newTestSandboxConfigNoop()

	p, _, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStartSandboxHyperstartAgentSuccessful(t *testing.T) {
	cleanUp()

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

	p, _, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	bindUnmountAllRootfs(defaultSharedDir, pImpl)
}

func TestStartSandboxKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

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

	p, _, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	bindUnmountAllRootfs(defaultSharedDir, pImpl)
}

func TestStartSandboxFailing(t *testing.T) {
	cleanUp()

	sandboxDir := filepath.Join(configStoragePath, testSandboxID)
	os.Remove(sandboxDir)

	p, err := StartSandbox(testSandboxID)
	if p != nil || err == nil {
		t.Fatal()
	}
}

func TestStopSandboxNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	config := newTestSandboxConfigNoop()

	p, _, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	vp, err := StopSandbox(p.ID())
	if vp == nil || err != nil {
		t.Fatal(err)
	}
}

func TestPauseThenResumeSandboxNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	config := newTestSandboxConfigNoop()

	p, _, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contID := "100"
	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	p, err = PauseSandbox(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	expectedState := StatePaused

	assert.Equal(t, pImpl.state.State, expectedState, "unexpected paused sandbox state")

	for i, c := range p.GetAllContainers() {
		cImpl, ok := c.(*Container)
		assert.True(t, ok)

		assert.Equal(t, expectedState, cImpl.state.State,
			fmt.Sprintf("paused container %d has unexpected state", i))
	}

	p, err = ResumeSandbox(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok = p.(*Sandbox)
	assert.True(t, ok)

	expectedState = StateRunning

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

	cleanUp()

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

	p, _, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StopSandbox(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStopSandboxKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

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

	p, _, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StopSandbox(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStopSandboxFailing(t *testing.T) {
	cleanUp()

	sandboxDir := filepath.Join(configStoragePath, testSandboxID)
	os.Remove(sandboxDir)

	p, err := StopSandbox(testSandboxID)
	if p != nil || err == nil {
		t.Fatal()
	}
}

func TestRunSandboxNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	config := newTestSandboxConfigNoop()

	p, err := RunSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}
}

func TestRunSandboxHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

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

	p, err := RunSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	bindUnmountAllRootfs(defaultSharedDir, pImpl)
}

func TestRunSandboxKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

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

	p, err := RunSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	bindUnmountAllRootfs(defaultSharedDir, pImpl)
}

func TestRunSandboxFailing(t *testing.T) {
	cleanUp()

	config := SandboxConfig{}

	p, err := RunSandbox(config, nil)
	if p != nil || err == nil {
		t.Fatal()
	}
}

func TestListSandboxSuccessful(t *testing.T) {
	cleanUp()

	os.RemoveAll(configStoragePath)

	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	_, err = ListSandbox()
	if err != nil {
		t.Fatal(err)
	}
}

func TestListSandboxNoSandboxDirectory(t *testing.T) {
	cleanUp()

	os.RemoveAll(configStoragePath)

	_, err := ListSandbox()
	if err != nil {
		t.Fatal(fmt.Sprintf("unexpected ListSandbox error from non-existent sandbox directory: %v", err))
	}
}

func TestStatusSandboxSuccessfulStateReady(t *testing.T) {
	cleanUp()

	config := newTestSandboxConfigNoop()
	hypervisorConfig := HypervisorConfig{
		KernelPath:        filepath.Join(testDir, testKernel),
		ImagePath:         filepath.Join(testDir, testImage),
		HypervisorPath:    filepath.Join(testDir, testHypervisor),
		DefaultVCPUs:      defaultVCPUs,
		DefaultMemSz:      defaultMemSzMiB,
		DefaultBridges:    defaultBridges,
		BlockDeviceDriver: defaultBlockDriver,
		DefaultMaxVCPUs:   defaultMaxQemuVCPUs,
		Msize9p:           defaultMsize9p,
	}

	expectedStatus := SandboxStatus{
		ID: testSandboxID,
		State: State{
			State: StateReady,
		},
		Hypervisor:       MockHypervisor,
		HypervisorConfig: hypervisorConfig,
		Agent:            NoopAgentType,
		Annotations:      sandboxAnnotations,
		ContainersStatus: []ContainerStatus{
			{
				ID: containerID,
				State: State{
					State: StateReady,
				},
				PID:         0,
				RootFs:      filepath.Join(testDir, testBundle),
				Annotations: containerAnnotations,
			},
		},
	}

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	status, err := StatusSandbox(p.ID())
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
	cleanUp()

	config := newTestSandboxConfigNoop()
	hypervisorConfig := HypervisorConfig{
		KernelPath:        filepath.Join(testDir, testKernel),
		ImagePath:         filepath.Join(testDir, testImage),
		HypervisorPath:    filepath.Join(testDir, testHypervisor),
		DefaultVCPUs:      defaultVCPUs,
		DefaultMemSz:      defaultMemSzMiB,
		DefaultBridges:    defaultBridges,
		BlockDeviceDriver: defaultBlockDriver,
		DefaultMaxVCPUs:   defaultMaxQemuVCPUs,
		Msize9p:           defaultMsize9p,
	}

	expectedStatus := SandboxStatus{
		ID: testSandboxID,
		State: State{
			State: StateRunning,
		},
		Hypervisor:       MockHypervisor,
		HypervisorConfig: hypervisorConfig,
		Agent:            NoopAgentType,
		Annotations:      sandboxAnnotations,
		ContainersStatus: []ContainerStatus{
			{
				ID: containerID,
				State: State{
					State: StateRunning,
				},
				PID:         0,
				RootFs:      filepath.Join(testDir, testBundle),
				Annotations: containerAnnotations,
			},
		},
	}

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StartSandbox(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	status, err := StatusSandbox(p.ID())
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
	cleanUp()

	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	path := filepath.Join(configStoragePath, p.ID())
	os.RemoveAll(path)
	globalSandboxList.removeSandbox(p.ID())

	_, err = StatusSandbox(p.ID())
	if err == nil {
		t.Fatal()
	}
}

func TestStatusPodSandboxFailingFetchSandboxState(t *testing.T) {
	cleanUp()

	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	os.RemoveAll(pImpl.configPath)
	globalSandboxList.removeSandbox(p.ID())

	_, err = StatusSandbox(p.ID())
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
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
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
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = DeleteSandbox(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err == nil {
		t.Fatal()
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c != nil || err == nil {
		t.Fatal(err)
	}
}

func TestDeleteContainerSuccessful(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err = DeleteContainer(p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(contDir)
	if err == nil {
		t.Fatal()
	}
}

func TestDeleteContainerFailingNoSandbox(t *testing.T) {
	cleanUp()

	sandboxDir := filepath.Join(configStoragePath, testSandboxID)
	contID := "100"
	os.RemoveAll(sandboxDir)

	c, err := DeleteContainer(testSandboxID, contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestDeleteContainerFailingNoContainer(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err := DeleteContainer(p.ID(), contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestStartContainerNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, sandboxDir, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}
	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err = StartContainer(p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStartContainerFailingNoSandbox(t *testing.T) {
	cleanUp()

	sandboxDir := filepath.Join(configStoragePath, testSandboxID)
	contID := "100"
	os.RemoveAll(sandboxDir)

	c, err := StartContainer(testSandboxID, contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestStartContainerFailingNoContainer(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err := StartContainer(p.ID(), contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestStartContainerFailingSandboxNotStarted(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	_, err = StartContainer(p.ID(), contID)
	if err == nil {
		t.Fatal("Function should have failed")
	}
}

func TestStopContainerNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, sandboxDir, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err = StartContainer(p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	c, err = StopContainer(p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStartStopContainerHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

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

	p, sandboxDir, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err = StartContainer(p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	c, err = StopContainer(p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	bindUnmountAllRootfs(defaultSharedDir, pImpl)
}

func TestStartStopSandboxHyperstartAgentSuccessfulWithCNINetwork(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

	config := newTestSandboxConfigHyperstartAgentCNINetwork()

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

	p, _, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StopSandbox(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = DeleteSandbox(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStartStopSandboxHyperstartAgentSuccessfulWithCNMNetwork(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	config := newTestSandboxConfigHyperstartAgentCNMNetwork()

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

	p, _, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	v, err := StopSandbox(p.ID())
	if v == nil || err != nil {
		t.Fatal(err)
	}

	v, err = DeleteSandbox(p.ID())
	if v == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStopContainerFailingNoSandbox(t *testing.T) {
	cleanUp()

	sandboxDir := filepath.Join(configStoragePath, testSandboxID)
	contID := "100"
	os.RemoveAll(sandboxDir)

	c, err := StopContainer(testSandboxID, contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestStopContainerFailingNoContainer(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err := StopContainer(p.ID(), contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func testKillContainerFromContReadySuccessful(t *testing.T, signal syscall.Signal) {
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, sandboxDir, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	if err := KillContainer(p.ID(), contID, signal, false); err != nil {
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
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, sandboxDir, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err = StartContainer(p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	cmd := newBasicTestCmd()

	_, c, _, err = EnterContainer(p.ID(), contID, cmd)
	if c == nil || err != nil {
		t.Fatal(err)
	}
}

func TestEnterContainerHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

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

	p, sandboxDir, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, _, err = CreateContainer(p.ID(), contConfig)
	if err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	_, err = StartContainer(p.ID(), contID)
	if err != nil {
		t.Fatal(err)
	}

	cmd := newBasicTestCmd()

	_, _, _, err = EnterContainer(p.ID(), contID, cmd)
	if err != nil {
		t.Fatal(err)
	}

	_, err = StopContainer(p.ID(), contID)
	if err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	bindUnmountAllRootfs(defaultSharedDir, pImpl)
}

func TestEnterContainerFailingNoSandbox(t *testing.T) {
	cleanUp()

	sandboxDir := filepath.Join(configStoragePath, testSandboxID)
	contID := "100"
	os.RemoveAll(sandboxDir)

	cmd := newBasicTestCmd()

	_, c, _, err := EnterContainer(testSandboxID, contID, cmd)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestEnterContainerFailingNoContainer(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	cmd := newBasicTestCmd()

	_, c, _, err := EnterContainer(p.ID(), contID, cmd)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestEnterContainerFailingContNotStarted(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, sandboxDir, err := createAndStartSandbox(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	cmd := newBasicTestCmd()

	_, c, _, err = EnterContainer(p.ID(), contID, cmd)
	if c == nil || err != nil {
		t.Fatal()
	}
}

func TestStatusContainerSuccessful(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	status, err := StatusContainer(p.ID(), contID)
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
	cleanUp()

	// (homage to a great album! ;)
	contID := "101"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	// fresh lookup
	p2, err := fetchSandbox(p.ID())
	if err != nil {
		t.Fatal(err)
	}

	expectedStatus := ContainerStatus{
		ID: contID,
		State: State{
			State: StateReady,
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
	cleanUp()

	// (homage to a great album! ;)
	contID := "101"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StartSandbox(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	sandboxDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	c, err = StartContainer(p.ID(), c.ID())
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	// fresh lookup
	p2, err := fetchSandbox(p.ID())
	if err != nil {
		t.Fatal(err)
	}

	expectedStatus := ContainerStatus{
		ID: contID,
		State: State{
			State: StateRunning,
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
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	os.RemoveAll(pImpl.configPath)
	globalSandboxList.removeSandbox(p.ID())

	_, err = StatusContainer(p.ID(), contID)
	if err == nil {
		t.Fatal()
	}
}

func TestStatsContainerFailing(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config, nil)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(t, ok)

	os.RemoveAll(pImpl.configPath)
	globalSandboxList.removeSandbox(p.ID())

	_, err = StatsContainer(p.ID(), contID)
	if err == nil {
		t.Fatal()
	}
}

func TestStatsContainer(t *testing.T) {
	cleanUp()

	assert := assert.New(t)
	contID := "100"

	_, err := StatsContainer("", "")
	assert.Error(err)

	_, err = StatsContainer("abc", "")
	assert.Error(err)

	_, err = StatsContainer("abc", "abc")
	assert.Error(err)

	config := newTestSandboxConfigNoop()
	p, err := CreateSandbox(config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	p, err = StartSandbox(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Sandbox)
	assert.True(ok)
	defer os.RemoveAll(pImpl.configPath)

	contConfig := newTestContainerConfigNoop(contID)
	_, c, err := CreateContainer(p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	_, err = StatsContainer(pImpl.id, "xyz")
	assert.Error(err)

	_, err = StatsContainer("xyz", contID)
	assert.Error(err)

	stats, err := StatsContainer(pImpl.id, contID)
	assert.NoError(err)
	assert.Equal(stats, ContainerStats{})
}

func TestProcessListContainer(t *testing.T) {
	cleanUp()

	assert := assert.New(t)

	contID := "abc"
	options := ProcessListOptions{
		Format: "json",
		Args:   []string{"-ef"},
	}

	_, err := ProcessListContainer("", "", options)
	assert.Error(err)

	_, err = ProcessListContainer("xyz", "", options)
	assert.Error(err)

	_, err = ProcessListContainer("xyz", "xyz", options)
	assert.Error(err)

	config := newTestSandboxConfigNoop()
	p, err := CreateSandbox(config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	pImpl, ok := p.(*Sandbox)
	assert.True(ok)
	defer os.RemoveAll(pImpl.configPath)

	contConfig := newTestContainerConfigNoop(contID)
	_, c, err := CreateContainer(p.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	_, err = ProcessListContainer(pImpl.id, "xyz", options)
	assert.Error(err)

	_, err = ProcessListContainer("xyz", contID, options)
	assert.Error(err)

	_, err = ProcessListContainer(pImpl.id, contID, options)
	// Sandbox not running, impossible to ps the container
	assert.Error(err)
}

/*
 * Benchmarks
 */

func createNewSandboxConfig(hType HypervisorType, aType AgentType, aConfig interface{}, netModel NetworkModel) SandboxConfig {
	hypervisorConfig := HypervisorConfig{
		KernelPath:     "/usr/share/kata-containers/vmlinux.container",
		ImagePath:      "/usr/share/kata-containers/kata-containers.img",
		HypervisorPath: "/usr/bin/qemu-system-x86_64",
	}

	netConfig := NetworkConfig{
		NumInterfaces: 1,
	}

	return SandboxConfig{
		ID:               testSandboxID,
		HypervisorType:   hType,
		HypervisorConfig: hypervisorConfig,

		AgentType:   aType,
		AgentConfig: aConfig,

		NetworkModel:  netModel,
		NetworkConfig: netConfig,
	}
}

func createNewContainerConfigs(numOfContainers int) []ContainerConfig {
	var contConfigs []ContainerConfig

	envs := []EnvVar{
		{
			Var:   "PATH",
			Value: "/bin:/usr/bin:/sbin:/usr/sbin",
		},
	}

	cmd := Cmd{
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
func createAndStartSandbox(config SandboxConfig) (sandbox VCSandbox, sandboxDir string,
	err error) {

	// Create sandbox
	sandbox, err = CreateSandbox(config, nil)
	if sandbox == nil || err != nil {
		return nil, "", err
	}

	sandboxDir = filepath.Join(configStoragePath, sandbox.ID())
	_, err = os.Stat(sandboxDir)
	if err != nil {
		return nil, "", err
	}

	// Start sandbox
	sandbox, err = StartSandbox(sandbox.ID())
	if sandbox == nil || err != nil {
		return nil, "", err
	}

	return sandbox, sandboxDir, nil
}

func createStartStopDeleteSandbox(b *testing.B, sandboxConfig SandboxConfig) {
	p, _, err := createAndStartSandbox(sandboxConfig)
	if p == nil || err != nil {
		b.Fatalf("Could not create and start sandbox: %s", err)
	}

	// Stop sandbox
	_, err = StopSandbox(p.ID())
	if err != nil {
		b.Fatalf("Could not stop sandbox: %s", err)
	}

	// Delete sandbox
	_, err = DeleteSandbox(p.ID())
	if err != nil {
		b.Fatalf("Could not delete sandbox: %s", err)
	}
}

func createStartStopDeleteContainers(b *testing.B, sandboxConfig SandboxConfig, contConfigs []ContainerConfig) {
	// Create sandbox
	p, err := CreateSandbox(sandboxConfig, nil)
	if err != nil {
		b.Fatalf("Could not create sandbox: %s", err)
	}

	// Start sandbox
	_, err = StartSandbox(p.ID())
	if err != nil {
		b.Fatalf("Could not start sandbox: %s", err)
	}

	// Create containers
	for _, contConfig := range contConfigs {
		_, _, err := CreateContainer(p.ID(), contConfig)
		if err != nil {
			b.Fatalf("Could not create container %s: %s", contConfig.ID, err)
		}
	}

	// Start containers
	for _, contConfig := range contConfigs {
		_, err := StartContainer(p.ID(), contConfig.ID)
		if err != nil {
			b.Fatalf("Could not start container %s: %s", contConfig.ID, err)
		}
	}

	// Stop containers
	for _, contConfig := range contConfigs {
		_, err := StopContainer(p.ID(), contConfig.ID)
		if err != nil {
			b.Fatalf("Could not stop container %s: %s", contConfig.ID, err)
		}
	}

	// Delete containers
	for _, contConfig := range contConfigs {
		_, err := DeleteContainer(p.ID(), contConfig.ID)
		if err != nil {
			b.Fatalf("Could not delete container %s: %s", contConfig.ID, err)
		}
	}

	// Stop sandbox
	_, err = StopSandbox(p.ID())
	if err != nil {
		b.Fatalf("Could not stop sandbox: %s", err)
	}

	// Delete sandbox
	_, err = DeleteSandbox(p.ID())
	if err != nil {
		b.Fatalf("Could not delete sandbox: %s", err)
	}
}

func BenchmarkCreateStartStopDeleteSandboxQemuHypervisorHyperstartAgentNetworkCNI(b *testing.B) {
	for i := 0; i < b.N; i++ {
		sandboxConfig := createNewSandboxConfig(QemuHypervisor, HyperstartAgent, HyperConfig{}, CNINetworkModel)

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

func BenchmarkCreateStartStopDeleteSandboxQemuHypervisorNoopAgentNetworkCNI(b *testing.B) {
	for i := 0; i < b.N; i++ {
		sandboxConfig := createNewSandboxConfig(QemuHypervisor, NoopAgentType, nil, CNINetworkModel)
		createStartStopDeleteSandbox(b, sandboxConfig)
	}
}

func BenchmarkCreateStartStopDeleteSandboxQemuHypervisorHyperstartAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		sandboxConfig := createNewSandboxConfig(QemuHypervisor, HyperstartAgent, HyperConfig{}, NoopNetworkModel)

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
		sandboxConfig := createNewSandboxConfig(QemuHypervisor, NoopAgentType, nil, NoopNetworkModel)
		createStartStopDeleteSandbox(b, sandboxConfig)
	}
}

func BenchmarkCreateStartStopDeleteSandboxMockHypervisorNoopAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		sandboxConfig := createNewSandboxConfig(MockHypervisor, NoopAgentType, nil, NoopNetworkModel)
		createStartStopDeleteSandbox(b, sandboxConfig)
	}
}

func BenchmarkStartStop1ContainerQemuHypervisorHyperstartAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		sandboxConfig := createNewSandboxConfig(QemuHypervisor, HyperstartAgent, HyperConfig{}, NoopNetworkModel)
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
		sandboxConfig := createNewSandboxConfig(QemuHypervisor, HyperstartAgent, HyperConfig{}, NoopNetworkModel)
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
	cleanUp()

	config := newTestSandboxConfigNoop()

	s, err := CreateSandbox(config, nil)
	if s == nil || err != nil {
		t.Fatal(err)
	}

	fetched, err := FetchSandbox(s.ID())
	assert.Nil(t, err, "%v", err)
	assert.True(t, fetched == s, "fetched sandboxed do not match")
}

func TestFetchNonExistingSandbox(t *testing.T) {
	cleanUp()

	_, err := FetchSandbox("some-non-existing-sandbox-name")
	assert.NotNil(t, err, "fetch non-existing sandbox should fail")
}

func TestReleaseSandbox(t *testing.T) {
	cleanUp()

	config := newTestSandboxConfigNoop()

	s, err := CreateSandbox(config, nil)
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

	cleanUp()

	period := uint64(1000)
	quota := int64(2000)
	assert := assert.New(t)
	resources := specs.LinuxResources{
		CPU: &specs.LinuxCPU{
			Period: &period,
			Quota:  &quota,
		},
	}
	err := UpdateContainer("", "", resources)
	assert.Error(err)

	err = UpdateContainer("abc", "", resources)
	assert.Error(err)

	contID := "100"
	config := newTestSandboxConfigNoop()

	s, sandboxDir, err := createAndStartSandbox(config)
	assert.NoError(err)
	assert.NotNil(s)

	contConfig := newTestContainerConfigNoop(contID)
	_, c, err := CreateContainer(s.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	_, err = StartContainer(s.ID(), contID)
	assert.NoError(err)

	err = UpdateContainer(s.ID(), contID, resources)
	assert.NoError(err)
}

func TestPauseResumeContainer(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

	assert := assert.New(t)
	err := PauseContainer("", "")
	assert.Error(err)

	err = PauseContainer("abc", "")
	assert.Error(err)

	contID := "100"
	config := newTestSandboxConfigNoop()

	s, sandboxDir, err := createAndStartSandbox(config)
	assert.NoError(err)
	assert.NotNil(s)

	contConfig := newTestContainerConfigNoop(contID)
	_, c, err := CreateContainer(s.ID(), contConfig)
	assert.NoError(err)
	assert.NotNil(c)

	contDir := filepath.Join(sandboxDir, contID)
	_, err = os.Stat(contDir)
	assert.NoError(err)

	_, err = StartContainer(s.ID(), contID)
	assert.NoError(err)

	err = PauseContainer(s.ID(), contID)
	assert.NoError(err)

	err = ResumeContainer(s.ID(), contID)
	assert.NoError(err)
}
