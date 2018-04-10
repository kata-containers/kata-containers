//
// Copyright (c) 2016 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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
	"github.com/stretchr/testify/assert"
)

const (
	containerID = "1"
)

var podAnnotations = map[string]string{
	"pod.foo":   "pod.bar",
	"pod.hello": "pod.world",
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

func newTestPodConfigNoop() PodConfig {
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

	podConfig := PodConfig{
		ID:               testPodID,
		HypervisorType:   MockHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType: NoopAgentType,

		Containers: []ContainerConfig{container},

		Annotations: podAnnotations,
	}

	return podConfig
}

func newTestPodConfigHyperstartAgent() PodConfig {
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

	podConfig := PodConfig{
		ID:               testPodID,
		HypervisorType:   MockHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   HyperstartAgent,
		AgentConfig: agentConfig,

		Containers:  []ContainerConfig{container},
		Annotations: podAnnotations,
	}

	return podConfig
}

func newTestPodConfigHyperstartAgentCNINetwork() PodConfig {
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

	podConfig := PodConfig{
		ID:               testPodID,
		HypervisorType:   MockHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   HyperstartAgent,
		AgentConfig: agentConfig,

		NetworkModel:  CNINetworkModel,
		NetworkConfig: netConfig,

		Containers:  []ContainerConfig{container},
		Annotations: podAnnotations,
	}

	return podConfig
}

func newTestPodConfigHyperstartAgentCNMNetwork() PodConfig {
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

	podConfig := PodConfig{
		ID:    testPodID,
		Hooks: hooks,

		HypervisorType:   MockHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   HyperstartAgent,
		AgentConfig: agentConfig,

		NetworkModel:  CNMNetworkModel,
		NetworkConfig: netConfig,

		Containers:  []ContainerConfig{container},
		Annotations: podAnnotations,
	}

	return podConfig
}

func newTestPodConfigKataAgent() PodConfig {
	// Sets the hypervisor configuration.
	hypervisorConfig := HypervisorConfig{
		KernelPath:     filepath.Join(testDir, testKernel),
		ImagePath:      filepath.Join(testDir, testImage),
		HypervisorPath: filepath.Join(testDir, testHypervisor),
	}

	podConfig := PodConfig{
		ID:               testPodID,
		HypervisorType:   MockHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType: KataContainersAgent,

		Annotations: podAnnotations,
	}

	return podConfig
}

func TestCreatePodNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
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

func TestCreatePodHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

	config := newTestPodConfigHyperstartAgent()

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

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}
}

func TestCreatePodKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

	config := newTestPodConfigKataAgent()

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

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}
}

func TestCreatePodFailing(t *testing.T) {
	cleanUp()

	config := PodConfig{}

	p, err := CreatePod(config)
	if p.(*Pod) != nil || err == nil {
		t.Fatal()
	}
}

func TestDeletePodNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}

	p, err = DeletePod(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(podDir)
	if err == nil {
		t.Fatal()
	}
}

func TestDeletePodHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

	config := newTestPodConfigHyperstartAgent()

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

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}

	p, err = DeletePod(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(podDir)
	if err == nil {
		t.Fatal(err)
	}
}

func TestDeletePodKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

	config := newTestPodConfigKataAgent()

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

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}

	p, err = DeletePod(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(podDir)
	if err == nil {
		t.Fatal(err)
	}
}

func TestDeletePodFailing(t *testing.T) {
	cleanUp()

	podDir := filepath.Join(configStoragePath, testPodID)
	os.Remove(podDir)

	p, err := DeletePod(testPodID)
	if p != nil || err == nil {
		t.Fatal()
	}
}

func TestStartPodNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	config := newTestPodConfigNoop()

	p, _, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStartPodHyperstartAgentSuccessful(t *testing.T) {
	cleanUp()

	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	config := newTestPodConfigHyperstartAgent()

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

	p, _, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Pod)
	assert.True(t, ok)

	bindUnmountAllRootfs(defaultSharedDir, *pImpl)
}

func TestStartPodKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

	config := newTestPodConfigKataAgent()

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

	p, _, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Pod)
	assert.True(t, ok)

	bindUnmountAllRootfs(defaultSharedDir, *pImpl)
}

func TestStartPodFailing(t *testing.T) {
	cleanUp()

	podDir := filepath.Join(configStoragePath, testPodID)
	os.Remove(podDir)

	p, err := StartPod(testPodID)
	if p != nil || err == nil {
		t.Fatal()
	}
}

func TestStopPodNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	config := newTestPodConfigNoop()

	p, _, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	vp, err := StopPod(p.ID())
	if vp == nil || err != nil {
		t.Fatal(err)
	}
}

func TestPauseThenResumePodNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	config := newTestPodConfigNoop()

	p, _, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contID := "100"
	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	p, err = PausePod(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Pod)
	assert.True(t, ok)

	expectedState := StatePaused

	assert.Equal(t, pImpl.state.State, expectedState, "unexpected paused pod state")

	for i, c := range p.GetAllContainers() {
		cImpl, ok := c.(*Container)
		assert.True(t, ok)

		assert.Equal(t, expectedState, cImpl.state.State,
			fmt.Sprintf("paused container %d has unexpected state", i))
	}

	p, err = ResumePod(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok = p.(*Pod)
	assert.True(t, ok)

	expectedState = StateRunning

	assert.Equal(t, pImpl.state.State, expectedState, "unexpected resumed pod state")

	for i, c := range p.GetAllContainers() {
		cImpl, ok := c.(*Container)
		assert.True(t, ok)

		assert.Equal(t, cImpl.state.State, expectedState,
			fmt.Sprintf("resumed container %d has unexpected state", i))
	}
}

func TestStopPodHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

	config := newTestPodConfigHyperstartAgent()

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

	p, _, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StopPod(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStopPodKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

	config := newTestPodConfigKataAgent()

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

	p, _, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StopPod(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStopPodFailing(t *testing.T) {
	cleanUp()

	podDir := filepath.Join(configStoragePath, testPodID)
	os.Remove(podDir)

	p, err := StopPod(testPodID)
	if p != nil || err == nil {
		t.Fatal()
	}
}

func TestRunPodNoopAgentSuccessful(t *testing.T) {
	cleanUp()

	config := newTestPodConfigNoop()

	p, err := RunPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}
}

func TestRunPodHyperstartAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

	config := newTestPodConfigHyperstartAgent()

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

	p, err := RunPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Pod)
	assert.True(t, ok)

	bindUnmountAllRootfs(defaultSharedDir, *pImpl)
}

func TestRunPodKataAgentSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

	config := newTestPodConfigKataAgent()

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

	p, err := RunPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Pod)
	assert.True(t, ok)

	bindUnmountAllRootfs(defaultSharedDir, *pImpl)
}

func TestRunPodFailing(t *testing.T) {
	cleanUp()

	config := PodConfig{}

	p, err := RunPod(config)
	if p != nil || err == nil {
		t.Fatal()
	}
}

func TestListPodSuccessful(t *testing.T) {
	cleanUp()

	os.RemoveAll(configStoragePath)

	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	_, err = ListPod()
	if err != nil {
		t.Fatal(err)
	}
}

func TestListPodNoPodDirectory(t *testing.T) {
	cleanUp()

	os.RemoveAll(configStoragePath)

	_, err := ListPod()
	if err != nil {
		t.Fatal(fmt.Sprintf("unexpected ListPod error from non-existent pod directory: %v", err))
	}
}

func TestStatusPodSuccessfulStateReady(t *testing.T) {
	cleanUp()

	config := newTestPodConfigNoop()
	hypervisorConfig := HypervisorConfig{
		KernelPath:        filepath.Join(testDir, testKernel),
		ImagePath:         filepath.Join(testDir, testImage),
		HypervisorPath:    filepath.Join(testDir, testHypervisor),
		DefaultVCPUs:      defaultVCPUs,
		DefaultMemSz:      defaultMemSzMiB,
		DefaultBridges:    defaultBridges,
		BlockDeviceDriver: defaultBlockDriver,
		DefaultMaxVCPUs:   defaultMaxQemuVCPUs,
	}

	expectedStatus := PodStatus{
		ID: testPodID,
		State: State{
			State: StateReady,
		},
		Hypervisor:       MockHypervisor,
		HypervisorConfig: hypervisorConfig,
		Agent:            NoopAgentType,
		Annotations:      podAnnotations,
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

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	status, err := StatusPod(p.ID())
	if err != nil {
		t.Fatal(err)
	}

	// Copy the start time as we can't pretend we know what that
	// value will be.
	expectedStatus.ContainersStatus[0].StartTime = status.ContainersStatus[0].StartTime

	if reflect.DeepEqual(status, expectedStatus) == false {
		t.Fatalf("Got pod status %v\n expecting %v", status, expectedStatus)
	}
}

func TestStatusPodSuccessfulStateRunning(t *testing.T) {
	cleanUp()

	config := newTestPodConfigNoop()
	hypervisorConfig := HypervisorConfig{
		KernelPath:        filepath.Join(testDir, testKernel),
		ImagePath:         filepath.Join(testDir, testImage),
		HypervisorPath:    filepath.Join(testDir, testHypervisor),
		DefaultVCPUs:      defaultVCPUs,
		DefaultMemSz:      defaultMemSzMiB,
		DefaultBridges:    defaultBridges,
		BlockDeviceDriver: defaultBlockDriver,
		DefaultMaxVCPUs:   defaultMaxQemuVCPUs,
	}

	expectedStatus := PodStatus{
		ID: testPodID,
		State: State{
			State: StateRunning,
		},
		Hypervisor:       MockHypervisor,
		HypervisorConfig: hypervisorConfig,
		Agent:            NoopAgentType,
		Annotations:      podAnnotations,
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

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StartPod(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	status, err := StatusPod(p.ID())
	if err != nil {
		t.Fatal(err)
	}

	// Copy the start time as we can't pretend we know what that
	// value will be.
	expectedStatus.ContainersStatus[0].StartTime = status.ContainersStatus[0].StartTime

	if reflect.DeepEqual(status, expectedStatus) == false {
		t.Fatalf("Got pod status %v\n expecting %v", status, expectedStatus)
	}
}

func TestStatusPodFailingFetchPodConfig(t *testing.T) {
	cleanUp()

	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	path := filepath.Join(configStoragePath, p.ID())
	os.RemoveAll(path)
	globalPodList.removePod(p.ID())

	_, err = StatusPod(p.ID())
	if err == nil {
		t.Fatal()
	}
}

func TestStatusPodPodFailingFetchPodState(t *testing.T) {
	cleanUp()

	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Pod)
	assert.True(t, ok)

	os.RemoveAll(pImpl.configPath)
	globalPodList.removePod(p.ID())

	_, err = StatusPod(p.ID())
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
	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(podDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}
}

func TestCreateContainerFailingNoPod(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = DeletePod(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
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
	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(podDir, contID)
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

func TestDeleteContainerFailingNoPod(t *testing.T) {
	cleanUp()

	podDir := filepath.Join(configStoragePath, testPodID)
	contID := "100"
	os.RemoveAll(podDir)

	c, err := DeleteContainer(testPodID, contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestDeleteContainerFailingNoContainer(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
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
	config := newTestPodConfigNoop()

	p, podDir, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}
	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(podDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err = StartContainer(p.ID(), contID)
	if c == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStartContainerFailingNoPod(t *testing.T) {
	cleanUp()

	podDir := filepath.Join(configStoragePath, testPodID)
	contID := "100"
	os.RemoveAll(podDir)

	c, err := StartContainer(testPodID, contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestStartContainerFailingNoContainer(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}

	c, err := StartContainer(p.ID(), contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestStartContainerFailingPodNotStarted(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(podDir, contID)
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
	config := newTestPodConfigNoop()

	p, podDir, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(podDir, contID)
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
	config := newTestPodConfigHyperstartAgent()

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

	p, podDir, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(podDir, contID)
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

	pImpl, ok := p.(*Pod)
	assert.True(t, ok)

	bindUnmountAllRootfs(defaultSharedDir, *pImpl)
}

func TestStartStopPodHyperstartAgentSuccessfulWithCNINetwork(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	cleanUp()

	config := newTestPodConfigHyperstartAgentCNINetwork()

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

	p, _, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StopPod(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = DeletePod(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStartStopPodHyperstartAgentSuccessfulWithCNMNetwork(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	config := newTestPodConfigHyperstartAgentCNMNetwork()

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

	p, _, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	v, err := StopPod(p.ID())
	if v == nil || err != nil {
		t.Fatal(err)
	}

	v, err = DeletePod(p.ID())
	if v == nil || err != nil {
		t.Fatal(err)
	}
}

func TestStopContainerFailingNoPod(t *testing.T) {
	cleanUp()

	podDir := filepath.Join(configStoragePath, testPodID)
	contID := "100"
	os.RemoveAll(podDir)

	c, err := StopContainer(testPodID, contID)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestStopContainerFailingNoContainer(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
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
	config := newTestPodConfigNoop()

	p, podDir, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(podDir, contID)
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
	config := newTestPodConfigNoop()

	p, podDir, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(podDir, contID)
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
	config := newTestPodConfigHyperstartAgent()

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

	p, podDir, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, _, err = CreateContainer(p.ID(), contConfig)
	if err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(podDir, contID)
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

	pImpl, ok := p.(*Pod)
	assert.True(t, ok)

	bindUnmountAllRootfs(defaultSharedDir, *pImpl)
}

func TestEnterContainerFailingNoPod(t *testing.T) {
	cleanUp()

	podDir := filepath.Join(configStoragePath, testPodID)
	contID := "100"
	os.RemoveAll(podDir)

	cmd := newBasicTestCmd()

	_, c, _, err := EnterContainer(testPodID, contID, cmd)
	if c != nil || err == nil {
		t.Fatal()
	}
}

func TestEnterContainerFailingNoContainer(t *testing.T) {
	cleanUp()

	contID := "100"
	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
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
	config := newTestPodConfigNoop()

	p, podDir, err := createAndStartPod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(podDir, contID)
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
	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(podDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	status, err := StatusContainer(p.ID(), contID)
	if err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Pod)
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
	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		t.Fatal(err)
	}

	contConfig := newTestContainerConfigNoop(contID)

	_, c, err := CreateContainer(p.ID(), contConfig)
	if c == nil || err != nil {
		t.Fatal(err)
	}

	contDir := filepath.Join(podDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	// fresh lookup
	p2, err := fetchPod(p.ID())
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
	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	p, err = StartPod(p.ID())
	if p == nil || err != nil {
		t.Fatal(err)
	}

	podDir := filepath.Join(configStoragePath, p.ID())
	_, err = os.Stat(podDir)
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

	contDir := filepath.Join(podDir, contID)
	_, err = os.Stat(contDir)
	if err != nil {
		t.Fatal(err)
	}

	// fresh lookup
	p2, err := fetchPod(p.ID())
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
	config := newTestPodConfigNoop()

	p, err := CreatePod(config)
	if p == nil || err != nil {
		t.Fatal(err)
	}

	pImpl, ok := p.(*Pod)
	assert.True(t, ok)

	os.RemoveAll(pImpl.configPath)
	globalPodList.removePod(p.ID())

	_, err = StatusContainer(p.ID(), contID)
	if err == nil {
		t.Fatal()
	}
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

	config := newTestPodConfigNoop()
	p, err := CreatePod(config)
	assert.NoError(err)
	assert.NotNil(p)

	pImpl, ok := p.(*Pod)
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
	// Pod not running, impossible to ps the container
	assert.Error(err)
}

/*
 * Benchmarks
 */

func createNewPodConfig(hType HypervisorType, aType AgentType, aConfig interface{}, netModel NetworkModel) PodConfig {
	hypervisorConfig := HypervisorConfig{
		KernelPath:     "/usr/share/kata-containers/vmlinux.container",
		ImagePath:      "/usr/share/kata-containers/kata-containers.img",
		HypervisorPath: "/usr/bin/qemu-system-x86_64",
	}

	netConfig := NetworkConfig{
		NumInterfaces: 1,
	}

	return PodConfig{
		ID:               testPodID,
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

// createAndStartPod handles the common test operation of creating and
// starting a pod.
func createAndStartPod(config PodConfig) (pod VCPod, podDir string,
	err error) {

	// Create pod
	pod, err = CreatePod(config)
	if pod == nil || err != nil {
		return nil, "", err
	}

	podDir = filepath.Join(configStoragePath, pod.ID())
	_, err = os.Stat(podDir)
	if err != nil {
		return nil, "", err
	}

	// Start pod
	pod, err = StartPod(pod.ID())
	if pod == nil || err != nil {
		return nil, "", err
	}

	return pod, podDir, nil
}

func createStartStopDeletePod(b *testing.B, podConfig PodConfig) {
	p, _, err := createAndStartPod(podConfig)
	if p == nil || err != nil {
		b.Fatalf("Could not create and start pod: %s", err)
	}

	// Stop pod
	_, err = StopPod(p.ID())
	if err != nil {
		b.Fatalf("Could not stop pod: %s", err)
	}

	// Delete pod
	_, err = DeletePod(p.ID())
	if err != nil {
		b.Fatalf("Could not delete pod: %s", err)
	}
}

func createStartStopDeleteContainers(b *testing.B, podConfig PodConfig, contConfigs []ContainerConfig) {
	// Create pod
	p, err := CreatePod(podConfig)
	if err != nil {
		b.Fatalf("Could not create pod: %s", err)
	}

	// Start pod
	_, err = StartPod(p.ID())
	if err != nil {
		b.Fatalf("Could not start pod: %s", err)
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

	// Stop pod
	_, err = StopPod(p.ID())
	if err != nil {
		b.Fatalf("Could not stop pod: %s", err)
	}

	// Delete pod
	_, err = DeletePod(p.ID())
	if err != nil {
		b.Fatalf("Could not delete pod: %s", err)
	}
}

func BenchmarkCreateStartStopDeletePodQemuHypervisorHyperstartAgentNetworkCNI(b *testing.B) {
	for i := 0; i < b.N; i++ {
		podConfig := createNewPodConfig(QemuHypervisor, HyperstartAgent, HyperConfig{}, CNINetworkModel)

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

		createStartStopDeletePod(b, podConfig)
	}
}

func BenchmarkCreateStartStopDeletePodQemuHypervisorNoopAgentNetworkCNI(b *testing.B) {
	for i := 0; i < b.N; i++ {
		podConfig := createNewPodConfig(QemuHypervisor, NoopAgentType, nil, CNINetworkModel)
		createStartStopDeletePod(b, podConfig)
	}
}

func BenchmarkCreateStartStopDeletePodQemuHypervisorHyperstartAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		podConfig := createNewPodConfig(QemuHypervisor, HyperstartAgent, HyperConfig{}, NoopNetworkModel)

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

		createStartStopDeletePod(b, podConfig)
	}
}

func BenchmarkCreateStartStopDeletePodQemuHypervisorNoopAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		podConfig := createNewPodConfig(QemuHypervisor, NoopAgentType, nil, NoopNetworkModel)
		createStartStopDeletePod(b, podConfig)
	}
}

func BenchmarkCreateStartStopDeletePodMockHypervisorNoopAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		podConfig := createNewPodConfig(MockHypervisor, NoopAgentType, nil, NoopNetworkModel)
		createStartStopDeletePod(b, podConfig)
	}
}

func BenchmarkStartStop1ContainerQemuHypervisorHyperstartAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		podConfig := createNewPodConfig(QemuHypervisor, HyperstartAgent, HyperConfig{}, NoopNetworkModel)
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

		createStartStopDeleteContainers(b, podConfig, contConfigs)
	}
}

func BenchmarkStartStop10ContainerQemuHypervisorHyperstartAgentNetworkNoop(b *testing.B) {
	for i := 0; i < b.N; i++ {
		podConfig := createNewPodConfig(QemuHypervisor, HyperstartAgent, HyperConfig{}, NoopNetworkModel)
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

		createStartStopDeleteContainers(b, podConfig, contConfigs)
	}
}
