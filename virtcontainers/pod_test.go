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
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"reflect"
	"sync"
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/stretchr/testify/assert"
)

func newHypervisorConfig(kernelParams []Param, hParams []Param) HypervisorConfig {
	return HypervisorConfig{
		KernelPath:       filepath.Join(testDir, testKernel),
		ImagePath:        filepath.Join(testDir, testImage),
		HypervisorPath:   filepath.Join(testDir, testHypervisor),
		KernelParams:     kernelParams,
		HypervisorParams: hParams,
	}

}

func testCreatePod(t *testing.T, id string,
	htype HypervisorType, hconfig HypervisorConfig, atype AgentType,
	nmodel NetworkModel, nconfig NetworkConfig, containers []ContainerConfig,
	volumes []Volume) (*Pod, error) {

	config := PodConfig{
		ID:               id,
		HypervisorType:   htype,
		HypervisorConfig: hconfig,
		AgentType:        atype,
		NetworkModel:     nmodel,
		NetworkConfig:    nconfig,
		Volumes:          volumes,
		Containers:       containers,
	}

	pod, err := createPod(config)
	if err != nil {
		return nil, fmt.Errorf("Could not create pod: %s", err)
	}

	if err := pod.agent.startPod(*pod); err != nil {
		return nil, err
	}

	if err := pod.createContainers(); err != nil {
		return nil, err
	}

	if pod.id == "" {
		return pod, fmt.Errorf("Invalid empty pod ID")
	}

	if id != "" && pod.id != id {
		return pod, fmt.Errorf("Invalid ID %s vs %s", id, pod.id)
	}

	return pod, nil
}

func TestCreateEmtpyPod(t *testing.T) {
	_, err := testCreatePod(t, testPodID, MockHypervisor, HypervisorConfig{}, NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	if err == nil {
		t.Fatalf("VirtContainers should not allow empty pods")
	}
	defer cleanUp()
}

func TestCreateEmtpyHypervisorPod(t *testing.T) {
	_, err := testCreatePod(t, testPodID, QemuHypervisor, HypervisorConfig{}, NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	if err == nil {
		t.Fatalf("VirtContainers should not allow pods with empty hypervisors")
	}
	defer cleanUp()
}

func TestCreateMockPod(t *testing.T) {
	hConfig := newHypervisorConfig(nil, nil)

	_, err := testCreatePod(t, testPodID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()
}

func TestCreatePodEmtpyID(t *testing.T) {
	hConfig := newHypervisorConfig(nil, nil)

	p, err := testCreatePod(t, "", MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	if err == nil {
		t.Fatalf("Expected pod with empty ID to fail, but got pod %v", p)
	}
	defer cleanUp()
}

func testPodStateTransition(t *testing.T, state stateString, newState stateString) error {
	hConfig := newHypervisorConfig(nil, nil)

	p, err := testCreatePod(t, testPodID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	if err != nil {
		return err
	}
	defer cleanUp()

	p.state = State{
		State: state,
	}

	return p.state.validTransition(state, newState)
}

func TestPodStateReadyRunning(t *testing.T) {
	err := testPodStateTransition(t, StateReady, StateRunning)
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodStateRunningPaused(t *testing.T) {
	err := testPodStateTransition(t, StateRunning, StatePaused)
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodStatePausedRunning(t *testing.T) {
	err := testPodStateTransition(t, StatePaused, StateRunning)
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodStatePausedStopped(t *testing.T) {
	err := testPodStateTransition(t, StatePaused, StateStopped)
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodStateRunningStopped(t *testing.T) {
	err := testPodStateTransition(t, StateRunning, StateStopped)
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodStateReadyPaused(t *testing.T) {
	err := testPodStateTransition(t, StateReady, StateStopped)
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodStatePausedReady(t *testing.T) {
	err := testPodStateTransition(t, StateStopped, StateReady)
	if err == nil {
		t.Fatal("Invalid transition from Ready to Paused")
	}
}

func testPodDir(t *testing.T, resource podResource, expected string) error {
	fs := filesystem{}
	_, dir, err := fs.podURI(testPodID, resource)
	if err != nil {
		return err
	}

	if dir != expected {
		return fmt.Errorf("Unexpected pod directory %s vs %s", dir, expected)
	}

	return nil
}

func testPodFile(t *testing.T, resource podResource, expected string) error {
	fs := filesystem{}
	file, _, err := fs.podURI(testPodID, resource)
	if err != nil {
		return err
	}

	if file != expected {
		return fmt.Errorf("Unexpected pod file %s vs %s", file, expected)
	}

	return nil
}

func TestPodDirConfig(t *testing.T) {
	err := testPodDir(t, configFileType, podDirConfig)
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodDirState(t *testing.T) {
	err := testPodDir(t, stateFileType, podDirState)
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodDirLock(t *testing.T) {
	err := testPodDir(t, lockFileType, podDirLock)
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodDirNegative(t *testing.T) {
	fs := filesystem{}
	_, _, err := fs.podURI("", lockFileType)
	if err == nil {
		t.Fatal("Empty pod IDs should not be allowed")
	}
}

func TestPodFileConfig(t *testing.T) {
	err := testPodFile(t, configFileType, podFileConfig)
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodFileState(t *testing.T) {
	err := testPodFile(t, stateFileType, podFileState)
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodFileLock(t *testing.T) {
	err := testPodFile(t, lockFileType, podFileLock)
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodFileNegative(t *testing.T) {
	fs := filesystem{}
	_, _, err := fs.podURI("", lockFileType)
	if err == nil {
		t.Fatal("Empty pod IDs should not be allowed")
	}
}

func testStateValid(t *testing.T, stateStr stateString, expected bool) {
	state := &State{
		State: stateStr,
	}

	ok := state.valid()
	if ok != expected {
		t.Fatal()
	}
}

func TestStateValidSuccessful(t *testing.T) {
	testStateValid(t, StateReady, true)
	testStateValid(t, StateRunning, true)
	testStateValid(t, StatePaused, true)
	testStateValid(t, StateStopped, true)
}

func TestStateValidFailing(t *testing.T) {
	testStateValid(t, "", false)
}

func TestValidTransitionFailingOldStateMismatch(t *testing.T) {
	state := &State{
		State: StateReady,
	}

	err := state.validTransition(StateRunning, StateStopped)
	if err == nil {
		t.Fatal()
	}
}

func TestVolumesSetSuccessful(t *testing.T) {
	volumes := &Volumes{}

	volStr := "mountTag1:hostPath1 mountTag2:hostPath2"

	expected := Volumes{
		{
			MountTag: "mountTag1",
			HostPath: "hostPath1",
		},
		{
			MountTag: "mountTag2",
			HostPath: "hostPath2",
		},
	}

	err := volumes.Set(volStr)
	if err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(*volumes, expected) == false {
		t.Fatal()
	}
}

func TestVolumesSetFailingTooFewArguments(t *testing.T) {
	volumes := &Volumes{}

	volStr := "mountTag1 mountTag2"

	err := volumes.Set(volStr)
	if err == nil {
		t.Fatal()
	}
}

func TestVolumesSetFailingTooManyArguments(t *testing.T) {
	volumes := &Volumes{}

	volStr := "mountTag1:hostPath1:Foo1 mountTag2:hostPath2:Foo2"

	err := volumes.Set(volStr)
	if err == nil {
		t.Fatal()
	}
}

func TestVolumesSetFailingVoidArguments(t *testing.T) {
	volumes := &Volumes{}

	volStr := ": : :"

	err := volumes.Set(volStr)
	if err == nil {
		t.Fatal()
	}
}

func TestVolumesStringSuccessful(t *testing.T) {
	volumes := &Volumes{
		{
			MountTag: "mountTag1",
			HostPath: "hostPath1",
		},
		{
			MountTag: "mountTag2",
			HostPath: "hostPath2",
		},
	}

	expected := "mountTag1:hostPath1 mountTag2:hostPath2"

	result := volumes.String()
	if result != expected {
		t.Fatal()
	}
}

func TestSocketsSetSuccessful(t *testing.T) {
	sockets := &Sockets{}

	sockStr := "devID1:id1:hostPath1:Name1 devID2:id2:hostPath2:Name2"

	expected := Sockets{
		{
			DeviceID: "devID1",
			ID:       "id1",
			HostPath: "hostPath1",
			Name:     "Name1",
		},
		{
			DeviceID: "devID2",
			ID:       "id2",
			HostPath: "hostPath2",
			Name:     "Name2",
		},
	}

	err := sockets.Set(sockStr)
	if err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(*sockets, expected) == false {
		t.Fatal()
	}
}

func TestSocketsSetFailingWrongArgsAmount(t *testing.T) {
	sockets := &Sockets{}

	sockStr := "devID1:id1:hostPath1"

	err := sockets.Set(sockStr)
	if err == nil {
		t.Fatal()
	}
}

func TestSocketsSetFailingVoidArguments(t *testing.T) {
	sockets := &Sockets{}

	sockStr := ":::"

	err := sockets.Set(sockStr)
	if err == nil {
		t.Fatal()
	}
}

func TestSocketsStringSuccessful(t *testing.T) {
	sockets := &Sockets{
		{
			DeviceID: "devID1",
			ID:       "id1",
			HostPath: "hostPath1",
			Name:     "Name1",
		},
		{
			DeviceID: "devID2",
			ID:       "id2",
			HostPath: "hostPath2",
			Name:     "Name2",
		},
	}

	expected := "devID1:id1:hostPath1:Name1 devID2:id2:hostPath2:Name2"

	result := sockets.String()
	if result != expected {
		t.Fatal()
	}
}

func TestPodListSuccessful(t *testing.T) {
	pod := &Pod{}

	podList, err := pod.list()
	if podList != nil || err != nil {
		t.Fatal()
	}
}

func TestPodEnterSuccessful(t *testing.T) {
	pod := &Pod{}

	err := pod.enter([]string{})
	if err != nil {
		t.Fatal(err)
	}
}

func testCheckInitPodAndContainerStates(p *Pod, initialPodState State, c *Container, initialContainerState State) error {
	if p.state.State != initialPodState.State {
		return fmt.Errorf("Expected pod state %v, got %v", initialPodState.State, p.state.State)
	}

	if c.state.State != initialContainerState.State {
		return fmt.Errorf("Expected container state %v, got %v", initialContainerState.State, c.state.State)
	}

	return nil
}

func testForcePodStateChangeAndCheck(t *testing.T, p *Pod, newPodState State) error {
	// force pod state change
	if err := p.setPodState(newPodState.State); err != nil {
		t.Fatalf("Unexpected error: %v (pod %+v)", err, p)
	}

	// check the in-memory state is correct
	if p.state.State != newPodState.State {
		return fmt.Errorf("Expected state %v, got %v", newPodState.State, p.state.State)
	}

	return nil
}

func testForceContainerStateChangeAndCheck(t *testing.T, p *Pod, c *Container, newContainerState State) error {
	// force container state change
	if err := c.setContainerState(newContainerState.State); err != nil {
		t.Fatalf("Unexpected error: %v (pod %+v)", err, p)
	}

	// check the in-memory state is correct
	if c.state.State != newContainerState.State {
		return fmt.Errorf("Expected state %v, got %v", newContainerState.State, c.state.State)
	}

	return nil
}

func testCheckPodOnDiskState(p *Pod, podState State) error {
	// check on-disk state is correct
	if p.state.State != podState.State {
		return fmt.Errorf("Expected state %v, got %v", podState.State, p.state.State)
	}

	return nil
}

func testCheckContainerOnDiskState(c *Container, containerState State) error {
	// check on-disk state is correct
	if c.state.State != containerState.State {
		return fmt.Errorf("Expected state %v, got %v", containerState.State, c.state.State)
	}

	return nil
}

func TestPodSetPodAndContainerState(t *testing.T) {
	contID := "505"
	contConfig := newTestContainerConfigNoop(contID)
	hConfig := newHypervisorConfig(nil, nil)

	// create a pod
	p, err := testCreatePod(t, testPodID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, []ContainerConfig{contConfig}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	l := len(p.GetAllContainers())
	if l != 1 {
		t.Fatalf("Expected 1 container found %v", l)
	}

	initialPodState := State{
		State: StateReady,
	}

	// After a pod creation, a container has a READY state
	initialContainerState := State{
		State: StateReady,
	}

	c, err := p.findContainer(contID)
	if err != nil {
		t.Fatalf("Failed to retrieve container %v: %v", contID, err)
	}

	// check initial pod and container states
	if err := testCheckInitPodAndContainerStates(p, initialPodState, c, initialContainerState); err != nil {
		t.Error(err)
	}

	// persist to disk
	err = p.storePod()
	if err != nil {
		t.Fatal(err)
	}

	newPodState := State{
		State: StateRunning,
	}

	if err := testForcePodStateChangeAndCheck(t, p, newPodState); err != nil {
		t.Error(err)
	}

	newContainerState := State{
		State: StateStopped,
	}

	if err := testForceContainerStateChangeAndCheck(t, p, c, newContainerState); err != nil {
		t.Error(err)
	}

	// force state to be read from disk
	p2, err := fetchPod(p.ID())
	if err != nil {
		t.Fatalf("Failed to fetch pod %v: %v", p.ID(), err)
	}

	if err := testCheckPodOnDiskState(p2, newPodState); err != nil {
		t.Error(err)
	}

	c2, err := p2.findContainer(contID)
	if err != nil {
		t.Fatalf("Failed to find container %v: %v", contID, err)
	}

	if err := testCheckContainerOnDiskState(c2, newContainerState); err != nil {
		t.Error(err)
	}

	// revert pod state to allow it to be deleted
	err = p.setPodState(initialPodState.State)
	if err != nil {
		t.Fatalf("Unexpected error: %v (pod %+v)", err, p)
	}

	// clean up
	err = p.delete()
	if err != nil {
		t.Fatal(err)
	}
}

func TestPodSetPodStateFailingStorePodResource(t *testing.T) {
	fs := &filesystem{}
	pod := &Pod{
		storage: fs,
	}

	err := pod.setPodState(StateReady)
	if err == nil {
		t.Fatal()
	}
}

func TestPodSetContainersStateFailingEmptyPodID(t *testing.T) {
	pod := &Pod{
		storage: &filesystem{},
	}

	containers := []*Container{
		{
			id:  "100",
			pod: pod,
		},
	}

	pod.containers = containers

	err := pod.setContainersState(StateReady)
	if err == nil {
		t.Fatal()
	}
}

func TestPodDeleteContainerStateSuccessful(t *testing.T) {
	contID := "100"

	fs := &filesystem{}
	pod := &Pod{
		id:      testPodID,
		storage: fs,
	}

	path := filepath.Join(runStoragePath, testPodID, contID)
	err := os.MkdirAll(path, dirMode)
	if err != nil {
		t.Fatal(err)
	}

	stateFilePath := filepath.Join(path, stateFile)

	os.Remove(stateFilePath)

	_, err = os.Create(stateFilePath)
	if err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(stateFilePath)
	if err != nil {
		t.Fatal(err)
	}

	err = pod.deleteContainerState(contID)
	if err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(stateFilePath)
	if err == nil {
		t.Fatal()
	}
}

func TestPodDeleteContainerStateFailingEmptyPodID(t *testing.T) {
	contID := "100"

	fs := &filesystem{}
	pod := &Pod{
		storage: fs,
	}

	err := pod.deleteContainerState(contID)
	if err == nil {
		t.Fatal()
	}
}

func TestPodDeleteContainersStateSuccessful(t *testing.T) {
	var err error

	containers := []ContainerConfig{
		{
			ID: "100",
		},
		{
			ID: "200",
		},
	}

	podConfig := &PodConfig{
		Containers: containers,
	}

	fs := &filesystem{}
	pod := &Pod{
		id:      testPodID,
		config:  podConfig,
		storage: fs,
	}

	for _, c := range containers {
		path := filepath.Join(runStoragePath, testPodID, c.ID)
		err = os.MkdirAll(path, dirMode)
		if err != nil {
			t.Fatal(err)
		}

		stateFilePath := filepath.Join(path, stateFile)

		os.Remove(stateFilePath)

		_, err = os.Create(stateFilePath)
		if err != nil {
			t.Fatal(err)
		}

		_, err = os.Stat(stateFilePath)
		if err != nil {
			t.Fatal(err)
		}
	}

	err = pod.deleteContainersState()
	if err != nil {
		t.Fatal(err)
	}

	for _, c := range containers {
		stateFilePath := filepath.Join(runStoragePath, testPodID, c.ID, stateFile)
		_, err = os.Stat(stateFilePath)
		if err == nil {
			t.Fatal()
		}
	}
}

func TestPodDeleteContainersStateFailingEmptyPodID(t *testing.T) {
	containers := []ContainerConfig{
		{
			ID: "100",
		},
	}

	podConfig := &PodConfig{
		Containers: containers,
	}

	fs := &filesystem{}
	pod := &Pod{
		config:  podConfig,
		storage: fs,
	}

	err := pod.deleteContainersState()
	if err == nil {
		t.Fatal()
	}
}

func TestGetContainer(t *testing.T) {
	containerIDs := []string{"abc", "123", "xyz", "rgb"}
	containers := []*Container{}

	for _, id := range containerIDs {
		c := Container{id: id}
		containers = append(containers, &c)
	}

	pod := Pod{
		containers: containers,
	}

	c := pod.GetContainer("noid")
	if c != nil {
		t.Fatal()
	}

	for _, id := range containerIDs {
		c = pod.GetContainer(id)
		if c == nil {
			t.Fatal()
		}
	}
}

func TestGetAllContainers(t *testing.T) {
	containerIDs := []string{"abc", "123", "xyz", "rgb"}
	containers := []*Container{}

	for _, id := range containerIDs {
		c := Container{id: id}
		containers = append(containers, &c)
	}

	pod := Pod{
		containers: containers,
	}

	list := pod.GetAllContainers()

	for i, c := range list {
		if c.ID() != containerIDs[i] {
			t.Fatal()
		}
	}
}

func TestSetAnnotations(t *testing.T) {
	pod := Pod{
		id:              "abcxyz123",
		storage:         &filesystem{},
		annotationsLock: &sync.RWMutex{},
		config: &PodConfig{
			Annotations: map[string]string{
				"annotation1": "abc",
			},
		},
	}

	keyAnnotation := "annotation2"
	valueAnnotation := "xyz"
	newAnnotations := map[string]string{
		keyAnnotation: valueAnnotation,
	}

	// Add a new annotation
	pod.SetAnnotations(newAnnotations)

	v, err := pod.Annotations(keyAnnotation)
	if err != nil {
		t.Fatal()
	}

	if v != valueAnnotation {
		t.Fatal()
	}

	//Change the value of an annotation
	valueAnnotation = "123"
	newAnnotations[keyAnnotation] = valueAnnotation

	pod.SetAnnotations(newAnnotations)

	v, err = pod.Annotations(keyAnnotation)
	if err != nil {
		t.Fatal()
	}

	if v != valueAnnotation {
		t.Fatal()
	}
}

func TestPodGetContainer(t *testing.T) {

	emptyPod := Pod{}
	_, err := emptyPod.findContainer("")
	if err == nil {
		t.Fatal("Expected error for containerless pod")
	}

	_, err = emptyPod.findContainer("foo")
	if err == nil {
		t.Fatal("Expected error for containerless pod and invalid containerID")
	}

	hConfig := newHypervisorConfig(nil, nil)
	p, err := testCreatePod(t, testPodID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	contID := "999"
	contConfig := newTestContainerConfigNoop(contID)
	newContainer, err := createContainer(p, contConfig)
	if err != nil {
		t.Fatalf("Failed to create container %+v in pod %+v: %v", contConfig, p, err)
	}

	if err := p.addContainer(newContainer); err != nil {
		t.Fatalf("Could not add container to pod %v", err)
	}

	got := false
	for _, c := range p.GetAllContainers() {
		c2, err := p.findContainer(c.ID())
		if err != nil {
			t.Fatalf("Failed to find container %v: %v", c.ID(), err)
		}

		if c2.ID() != c.ID() {
			t.Fatalf("Expected container %v but got %v", c.ID(), c2.ID())
		}

		if c2.ID() == contID {
			got = true
		}
	}

	if !got {
		t.Fatalf("Failed to find container %v", contID)
	}
}

func TestContainerSetStateBlockIndex(t *testing.T) {
	containers := []ContainerConfig{
		{
			ID: "100",
		},
	}

	hConfig := newHypervisorConfig(nil, nil)
	pod, err := testCreatePod(t, testPodID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, containers, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	fs := &filesystem{}
	pod.storage = fs

	c := pod.GetContainer("100")
	if c == nil {
		t.Fatal()
	}

	path := filepath.Join(runStoragePath, testPodID, c.ID())
	err = os.MkdirAll(path, dirMode)
	if err != nil {
		t.Fatal(err)
	}

	stateFilePath := filepath.Join(path, stateFile)

	os.Remove(stateFilePath)

	f, err := os.Create(stateFilePath)
	if err != nil {
		t.Fatal(err)
	}

	state := State{
		State:  "stopped",
		Fstype: "vfs",
	}

	cImpl, ok := c.(*Container)
	assert.True(t, ok)

	cImpl.state = state

	stateData := `{
		"state":"stopped",
		"fstype":"vfs"
	}`

	n, err := f.WriteString(stateData)
	if err != nil || n != len(stateData) {
		f.Close()
		t.Fatal()
	}
	f.Close()

	_, err = os.Stat(stateFilePath)
	if err != nil {
		t.Fatal(err)
	}

	newIndex := 20
	if err := cImpl.setStateBlockIndex(newIndex); err != nil {
		t.Fatal(err)
	}

	if cImpl.state.BlockIndex != newIndex {
		t.Fatal()
	}

	fileData, err := ioutil.ReadFile(stateFilePath)
	if err != nil {
		t.Fatal()
	}

	var res State
	err = json.Unmarshal([]byte(string(fileData)), &res)
	if err != nil {
		t.Fatal(err)
	}

	if res.BlockIndex != newIndex {
		t.Fatal()
	}

	if res.Fstype != state.Fstype {
		t.Fatal()
	}

	if res.State != state.State {
		t.Fatal()
	}
}

func TestContainerStateSetFstype(t *testing.T) {
	var err error

	containers := []ContainerConfig{
		{
			ID: "100",
		},
	}

	hConfig := newHypervisorConfig(nil, nil)
	pod, err := testCreatePod(t, testPodID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, containers, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	fs := &filesystem{}
	pod.storage = fs

	c := pod.GetContainer("100")
	if c == nil {
		t.Fatal()
	}

	path := filepath.Join(runStoragePath, testPodID, c.ID())
	err = os.MkdirAll(path, dirMode)
	if err != nil {
		t.Fatal(err)
	}

	stateFilePath := filepath.Join(path, stateFile)
	os.Remove(stateFilePath)

	f, err := os.Create(stateFilePath)
	if err != nil {
		t.Fatal(err)
	}

	state := State{
		State:      "ready",
		Fstype:     "vfs",
		BlockIndex: 3,
	}

	cImpl, ok := c.(*Container)
	assert.True(t, ok)

	cImpl.state = state

	stateData := `{
		"state":"ready",
		"fstype":"vfs",
		"blockIndex": 3
	}`

	n, err := f.WriteString(stateData)
	if err != nil || n != len(stateData) {
		f.Close()
		t.Fatal()
	}
	f.Close()

	_, err = os.Stat(stateFilePath)
	if err != nil {
		t.Fatal(err)
	}

	newFstype := "ext4"
	if err := cImpl.setStateFstype(newFstype); err != nil {
		t.Fatal(err)
	}

	if cImpl.state.Fstype != newFstype {
		t.Fatal()
	}

	fileData, err := ioutil.ReadFile(stateFilePath)
	if err != nil {
		t.Fatal()
	}

	var res State
	err = json.Unmarshal([]byte(string(fileData)), &res)
	if err != nil {
		t.Fatal(err)
	}

	if res.Fstype != newFstype {
		t.Fatal()
	}

	if res.BlockIndex != state.BlockIndex {
		t.Fatal()
	}

	if res.State != state.State {
		t.Fatal()
	}
}

func TestPodAttachDevicesVFIO(t *testing.T) {
	tmpDir, err := ioutil.TempDir("", "")
	assert.Nil(t, err)
	os.RemoveAll(tmpDir)

	testFDIOGroup := "2"
	testDeviceBDFPath := "0000:00:1c.0"

	devicesDir := filepath.Join(tmpDir, testFDIOGroup, "devices")
	err = os.MkdirAll(devicesDir, dirMode)
	assert.Nil(t, err)

	deviceFile := filepath.Join(devicesDir, testDeviceBDFPath)
	_, err = os.Create(deviceFile)
	assert.Nil(t, err)

	savedIOMMUPath := sysIOMMUPath
	sysIOMMUPath = tmpDir

	defer func() {
		sysIOMMUPath = savedIOMMUPath
	}()

	path := filepath.Join(vfioPath, testFDIOGroup)
	deviceInfo := DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "c",
	}
	vfioDevice := newVFIODevice(deviceInfo)

	c := &Container{
		id: "100",
		devices: []Device{
			vfioDevice,
		},
	}

	containers := []*Container{c}

	pod := Pod{
		containers: containers,
		hypervisor: &mockHypervisor{},
	}

	containers[0].pod = &pod

	err = containers[0].attachDevices()
	assert.Nil(t, err, "Error while attaching devices %s", err)

	err = containers[0].detachDevices()
	assert.Nil(t, err, "Error while detaching devices %s", err)
}

func TestPodCreateAssets(t *testing.T) {
	assert := assert.New(t)

	tmpfile, err := ioutil.TempFile("", "virtcontainers-test-")
	assert.Nil(err)

	defer func() {
		tmpfile.Close()
		os.Remove(tmpfile.Name()) // clean up
	}()

	_, err = tmpfile.Write(assetContent)
	assert.Nil(err)

	originalKernelPath := filepath.Join(testDir, testKernel)

	hc := HypervisorConfig{
		KernelPath: originalKernelPath,
		ImagePath:  filepath.Join(testDir, testImage),
	}

	p := &PodConfig{
		Annotations: map[string]string{
			annotations.KernelPath: tmpfile.Name(),
			annotations.KernelHash: assetContentHash,
		},

		HypervisorConfig: hc,
	}

	err = createAssets(p)
	assert.Nil(err)

	a, ok := p.HypervisorConfig.customAssets[kernelAsset]
	assert.True(ok)
	assert.Equal(a.path, tmpfile.Name())

	p = &PodConfig{
		Annotations: map[string]string{
			annotations.KernelPath: tmpfile.Name(),
			annotations.KernelHash: assetContentWrongHash,
		},

		HypervisorConfig: hc,
	}

	err = createAssets(p)
	assert.NotNil(err)
}

func testFindContainerFailure(t *testing.T, pod *Pod, cid string) {
	c, err := pod.findContainer(cid)
	assert.Nil(t, c, "Container pointer should be nil")
	assert.NotNil(t, err, "Should have returned an error")
}

func TestFindContainerPodNilFailure(t *testing.T) {
	testFindContainerFailure(t, nil, testContainerID)
}

func TestFindContainerContainerIDEmptyFailure(t *testing.T) {
	pod := &Pod{}
	testFindContainerFailure(t, pod, "")
}

func TestFindContainerNoContainerMatchFailure(t *testing.T) {
	pod := &Pod{}
	testFindContainerFailure(t, pod, testContainerID)
}

func TestFindContainerSuccess(t *testing.T) {
	pod := &Pod{
		containers: []*Container{
			{
				id: testContainerID,
			},
		},
	}
	c, err := pod.findContainer(testContainerID)
	assert.NotNil(t, c, "Container pointer should not be nil")
	assert.Nil(t, err, "Should not have returned an error: %v", err)

	assert.True(t, c == pod.containers[0], "Container pointers should point to the same address")
}

func TestRemoveContainerPodNilFailure(t *testing.T) {
	testFindContainerFailure(t, nil, testContainerID)
}

func TestRemoveContainerContainerIDEmptyFailure(t *testing.T) {
	pod := &Pod{}
	testFindContainerFailure(t, pod, "")
}

func TestRemoveContainerNoContainerMatchFailure(t *testing.T) {
	pod := &Pod{}
	testFindContainerFailure(t, pod, testContainerID)
}

func TestRemoveContainerSuccess(t *testing.T) {
	pod := &Pod{
		containers: []*Container{
			{
				id: testContainerID,
			},
		},
	}
	err := pod.removeContainer(testContainerID)
	assert.Nil(t, err, "Should not have returned an error: %v", err)

	assert.Equal(t, len(pod.containers), 0, "Containers list from pod structure should be empty")
}
