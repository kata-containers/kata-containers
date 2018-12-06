// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"sync"
	"syscall"
	"testing"

	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
	"github.com/kata-containers/runtime/virtcontainers/device/manager"
	"github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"golang.org/x/sys/unix"
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

func testCreateSandbox(t *testing.T, id string,
	htype HypervisorType, hconfig HypervisorConfig, atype AgentType,
	nmodel NetworkModel, nconfig NetworkConfig, containers []ContainerConfig,
	volumes []Volume) (*Sandbox, error) {

	sconfig := SandboxConfig{
		ID:               id,
		HypervisorType:   htype,
		HypervisorConfig: hconfig,
		AgentType:        atype,
		NetworkModel:     nmodel,
		NetworkConfig:    nconfig,
		Volumes:          volumes,
		Containers:       containers,
	}

	sandbox, err := createSandbox(context.Background(), sconfig, nil)
	if err != nil {
		return nil, fmt.Errorf("Could not create sandbox: %s", err)
	}

	if err := sandbox.agent.startSandbox(sandbox); err != nil {
		return nil, err
	}

	if err := sandbox.createContainers(); err != nil {
		return nil, err
	}

	if sandbox.id == "" {
		return sandbox, fmt.Errorf("Invalid empty sandbox ID")
	}

	if id != "" && sandbox.id != id {
		return sandbox, fmt.Errorf("Invalid ID %s vs %s", id, sandbox.id)
	}

	return sandbox, nil
}

func TestCreateEmtpySandbox(t *testing.T) {
	_, err := testCreateSandbox(t, testSandboxID, MockHypervisor, HypervisorConfig{}, NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	if err == nil {
		t.Fatalf("VirtContainers should not allow empty sandboxes")
	}
	defer cleanUp()
}

func TestCreateEmtpyHypervisorSandbox(t *testing.T) {
	_, err := testCreateSandbox(t, testSandboxID, QemuHypervisor, HypervisorConfig{}, NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	if err == nil {
		t.Fatalf("VirtContainers should not allow sandboxes with empty hypervisors")
	}
	defer cleanUp()
}

func TestCreateMockSandbox(t *testing.T) {
	hConfig := newHypervisorConfig(nil, nil)

	_, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()
}

func TestCreateSandboxEmtpyID(t *testing.T) {
	hConfig := newHypervisorConfig(nil, nil)

	p, err := testCreateSandbox(t, "", MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	if err == nil {
		t.Fatalf("Expected sandbox with empty ID to fail, but got sandbox %v", p)
	}
	defer cleanUp()
}

func testSandboxStateTransition(t *testing.T, state stateString, newState stateString) error {
	hConfig := newHypervisorConfig(nil, nil)

	p, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	if err != nil {
		return err
	}
	defer cleanUp()

	p.state = State{
		State: state,
	}

	return p.state.validTransition(state, newState)
}

func TestSandboxStateReadyRunning(t *testing.T) {
	err := testSandboxStateTransition(t, StateReady, StateRunning)
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxStateRunningPaused(t *testing.T) {
	err := testSandboxStateTransition(t, StateRunning, StatePaused)
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxStatePausedRunning(t *testing.T) {
	err := testSandboxStateTransition(t, StatePaused, StateRunning)
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxStatePausedStopped(t *testing.T) {
	err := testSandboxStateTransition(t, StatePaused, StateStopped)
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxStateRunningStopped(t *testing.T) {
	err := testSandboxStateTransition(t, StateRunning, StateStopped)
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxStateReadyPaused(t *testing.T) {
	err := testSandboxStateTransition(t, StateReady, StateStopped)
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxStatePausedReady(t *testing.T) {
	err := testSandboxStateTransition(t, StateStopped, StateReady)
	if err == nil {
		t.Fatal("Invalid transition from Ready to Paused")
	}
}

func testSandboxDir(t *testing.T, resource sandboxResource, expected string) error {
	fs := filesystem{}
	_, dir, err := fs.sandboxURI(testSandboxID, resource)
	if err != nil {
		return err
	}

	if dir != expected {
		return fmt.Errorf("Unexpected sandbox directory %s vs %s", dir, expected)
	}

	return nil
}

func testSandboxFile(t *testing.T, resource sandboxResource, expected string) error {
	fs := filesystem{}
	file, _, err := fs.sandboxURI(testSandboxID, resource)
	if err != nil {
		return err
	}

	if file != expected {
		return fmt.Errorf("Unexpected sandbox file %s vs %s", file, expected)
	}

	return nil
}

func TestSandboxDirConfig(t *testing.T) {
	err := testSandboxDir(t, configFileType, sandboxDirConfig)
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxDirState(t *testing.T) {
	err := testSandboxDir(t, stateFileType, sandboxDirState)
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxDirLock(t *testing.T) {
	err := testSandboxDir(t, lockFileType, sandboxDirLock)
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxDirNegative(t *testing.T) {
	fs := filesystem{}
	_, _, err := fs.sandboxURI("", lockFileType)
	if err == nil {
		t.Fatal("Empty sandbox IDs should not be allowed")
	}
}

func TestSandboxFileConfig(t *testing.T) {
	err := testSandboxFile(t, configFileType, sandboxFileConfig)
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxFileState(t *testing.T) {
	err := testSandboxFile(t, stateFileType, sandboxFileState)
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxFileLock(t *testing.T) {
	err := testSandboxFile(t, lockFileType, sandboxFileLock)
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxFileNegative(t *testing.T) {
	fs := filesystem{}
	_, _, err := fs.sandboxURI("", lockFileType)
	if err == nil {
		t.Fatal("Empty sandbox IDs should not be allowed")
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

func TestSandboxListSuccessful(t *testing.T) {
	sandbox := &Sandbox{}

	sandboxList, err := sandbox.list()
	if sandboxList != nil || err != nil {
		t.Fatal()
	}
}

func TestSandboxEnterSuccessful(t *testing.T) {
	sandbox := &Sandbox{}

	err := sandbox.enter([]string{})
	if err != nil {
		t.Fatal(err)
	}
}

func testCheckInitSandboxAndContainerStates(p *Sandbox, initialSandboxState State, c *Container, initialContainerState State) error {
	if p.state.State != initialSandboxState.State {
		return fmt.Errorf("Expected sandbox state %v, got %v", initialSandboxState.State, p.state.State)
	}

	if c.state.State != initialContainerState.State {
		return fmt.Errorf("Expected container state %v, got %v", initialContainerState.State, c.state.State)
	}

	return nil
}

func testForceSandboxStateChangeAndCheck(t *testing.T, p *Sandbox, newSandboxState State) error {
	// force sandbox state change
	if err := p.setSandboxState(newSandboxState.State); err != nil {
		t.Fatalf("Unexpected error: %v (sandbox %+v)", err, p)
	}

	// check the in-memory state is correct
	if p.state.State != newSandboxState.State {
		return fmt.Errorf("Expected state %v, got %v", newSandboxState.State, p.state.State)
	}

	return nil
}

func testForceContainerStateChangeAndCheck(t *testing.T, p *Sandbox, c *Container, newContainerState State) error {
	// force container state change
	if err := c.setContainerState(newContainerState.State); err != nil {
		t.Fatalf("Unexpected error: %v (sandbox %+v)", err, p)
	}

	// check the in-memory state is correct
	if c.state.State != newContainerState.State {
		return fmt.Errorf("Expected state %v, got %v", newContainerState.State, c.state.State)
	}

	return nil
}

func testCheckSandboxOnDiskState(p *Sandbox, sandboxState State) error {
	// check on-disk state is correct
	if p.state.State != sandboxState.State {
		return fmt.Errorf("Expected state %v, got %v", sandboxState.State, p.state.State)
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

func TestSandboxSetSandboxAndContainerState(t *testing.T) {
	contID := "505"
	contConfig := newTestContainerConfigNoop(contID)
	hConfig := newHypervisorConfig(nil, nil)

	// create a sandbox
	p, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, []ContainerConfig{contConfig}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	l := len(p.GetAllContainers())
	if l != 1 {
		t.Fatalf("Expected 1 container found %v", l)
	}

	initialSandboxState := State{
		State: StateReady,
	}

	// After a sandbox creation, a container has a READY state
	initialContainerState := State{
		State: StateReady,
	}

	c, err := p.findContainer(contID)
	if err != nil {
		t.Fatalf("Failed to retrieve container %v: %v", contID, err)
	}

	// check initial sandbox and container states
	if err := testCheckInitSandboxAndContainerStates(p, initialSandboxState, c, initialContainerState); err != nil {
		t.Error(err)
	}

	// persist to disk
	err = p.storeSandbox()
	if err != nil {
		t.Fatal(err)
	}

	newSandboxState := State{
		State: StateRunning,
	}

	if err := testForceSandboxStateChangeAndCheck(t, p, newSandboxState); err != nil {
		t.Error(err)
	}

	newContainerState := State{
		State: StateStopped,
	}

	if err := testForceContainerStateChangeAndCheck(t, p, c, newContainerState); err != nil {
		t.Error(err)
	}

	// force state to be read from disk
	p2, err := fetchSandbox(context.Background(), p.ID())
	if err != nil {
		t.Fatalf("Failed to fetch sandbox %v: %v", p.ID(), err)
	}

	if err := testCheckSandboxOnDiskState(p2, newSandboxState); err != nil {
		t.Error(err)
	}

	c2, err := p2.findContainer(contID)
	if err != nil {
		t.Fatalf("Failed to find container %v: %v", contID, err)
	}

	if err := testCheckContainerOnDiskState(c2, newContainerState); err != nil {
		t.Error(err)
	}

	// revert sandbox state to allow it to be deleted
	err = p.setSandboxState(initialSandboxState.State)
	if err != nil {
		t.Fatalf("Unexpected error: %v (sandbox %+v)", err, p)
	}

	// clean up
	err = p.Delete()
	if err != nil {
		t.Fatal(err)
	}
}

func TestSandboxSetSandboxStateFailingStoreSandboxResource(t *testing.T) {
	fs := &filesystem{}
	sandbox := &Sandbox{
		storage: fs,
	}

	err := sandbox.setSandboxState(StateReady)
	if err == nil {
		t.Fatal()
	}
}

func TestSandboxSetContainersStateFailingEmptySandboxID(t *testing.T) {
	sandbox := &Sandbox{
		storage: &filesystem{},
	}

	containers := map[string]*Container{
		"100": {
			id:      "100",
			sandbox: sandbox,
		},
	}

	sandbox.containers = containers

	err := sandbox.setContainersState(StateReady)
	if err == nil {
		t.Fatal()
	}
}

func TestSandboxDeleteContainerStateSuccessful(t *testing.T) {
	contID := "100"

	fs := &filesystem{}
	sandbox := &Sandbox{
		id:      testSandboxID,
		storage: fs,
	}

	path := filepath.Join(runStoragePath, testSandboxID, contID)
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

	err = sandbox.deleteContainerState(contID)
	if err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(stateFilePath)
	if err == nil {
		t.Fatal()
	}
}

func TestSandboxDeleteContainerStateFailingEmptySandboxID(t *testing.T) {
	contID := "100"

	fs := &filesystem{}
	sandbox := &Sandbox{
		storage: fs,
	}

	err := sandbox.deleteContainerState(contID)
	if err == nil {
		t.Fatal()
	}
}

func TestSandboxDeleteContainersStateSuccessful(t *testing.T) {
	var err error

	containers := []ContainerConfig{
		{
			ID: "100",
		},
		{
			ID: "200",
		},
	}

	sandboxConfig := &SandboxConfig{
		Containers: containers,
	}

	fs := &filesystem{}
	sandbox := &Sandbox{
		id:      testSandboxID,
		config:  sandboxConfig,
		storage: fs,
	}

	for _, c := range containers {
		path := filepath.Join(runStoragePath, testSandboxID, c.ID)
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

	err = sandbox.deleteContainersState()
	if err != nil {
		t.Fatal(err)
	}

	for _, c := range containers {
		stateFilePath := filepath.Join(runStoragePath, testSandboxID, c.ID, stateFile)
		_, err = os.Stat(stateFilePath)
		if err == nil {
			t.Fatal()
		}
	}
}

func TestSandboxDeleteContainersStateFailingEmptySandboxID(t *testing.T) {
	containers := []ContainerConfig{
		{
			ID: "100",
		},
	}

	sandboxConfig := &SandboxConfig{
		Containers: containers,
	}

	fs := &filesystem{}
	sandbox := &Sandbox{
		config:  sandboxConfig,
		storage: fs,
	}

	err := sandbox.deleteContainersState()
	if err == nil {
		t.Fatal()
	}
}

func TestGetContainer(t *testing.T) {
	containerIDs := []string{"abc", "123", "xyz", "rgb"}
	containers := map[string]*Container{}

	for _, id := range containerIDs {
		c := Container{id: id}
		containers[id] = &c
	}

	sandbox := Sandbox{
		containers: containers,
	}

	c := sandbox.GetContainer("noid")
	if c != nil {
		t.Fatal()
	}

	for _, id := range containerIDs {
		c = sandbox.GetContainer(id)
		if c == nil {
			t.Fatal()
		}
	}
}

func TestGetAllContainers(t *testing.T) {
	containerIDs := []string{"abc", "123", "xyz", "rgb"}
	containers := map[string]*Container{}

	for _, id := range containerIDs {
		c := &Container{id: id}
		containers[id] = c
	}

	sandbox := Sandbox{
		containers: containers,
	}

	list := sandbox.GetAllContainers()

	for _, c := range list {
		if containers[c.ID()] == nil {
			t.Fatal()
		}
	}
}

func TestSetAnnotations(t *testing.T) {
	sandbox := Sandbox{
		id:              "abcxyz123",
		storage:         &filesystem{},
		annotationsLock: &sync.RWMutex{},
		config: &SandboxConfig{
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
	sandbox.SetAnnotations(newAnnotations)

	v, err := sandbox.Annotations(keyAnnotation)
	if err != nil {
		t.Fatal()
	}

	if v != valueAnnotation {
		t.Fatal()
	}

	//Change the value of an annotation
	valueAnnotation = "123"
	newAnnotations[keyAnnotation] = valueAnnotation

	sandbox.SetAnnotations(newAnnotations)

	v, err = sandbox.Annotations(keyAnnotation)
	if err != nil {
		t.Fatal()
	}

	if v != valueAnnotation {
		t.Fatal()
	}
}

func TestSandboxGetContainer(t *testing.T) {

	emptySandbox := Sandbox{}
	_, err := emptySandbox.findContainer("")
	if err == nil {
		t.Fatal("Expected error for containerless sandbox")
	}

	_, err = emptySandbox.findContainer("foo")
	if err == nil {
		t.Fatal("Expected error for containerless sandbox and invalid containerID")
	}

	hConfig := newHypervisorConfig(nil, nil)
	p, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	contID := "999"
	contConfig := newTestContainerConfigNoop(contID)
	nc, err := newContainer(p, contConfig)
	if err != nil {
		t.Fatalf("Failed to create container %+v in sandbox %+v: %v", contConfig, p, err)
	}

	if err := p.addContainer(nc); err != nil {
		t.Fatalf("Could not add container to sandbox %v", err)
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
	sandbox, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, containers, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	fs := &filesystem{}
	sandbox.storage = fs

	c := sandbox.GetContainer("100")
	if c == nil {
		t.Fatal()
	}

	path := filepath.Join(runStoragePath, testSandboxID, c.ID())
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
	sandbox, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, containers, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	fs := &filesystem{}
	sandbox.storage = fs

	c := sandbox.GetContainer("100")
	if c == nil {
		t.Fatal()
	}

	path := filepath.Join(runStoragePath, testSandboxID, c.ID())
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

const vfioPath = "/dev/vfio/"

func TestSandboxAttachDevicesVFIO(t *testing.T) {
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

	savedIOMMUPath := config.SysIOMMUPath
	config.SysIOMMUPath = tmpDir

	defer func() {
		config.SysIOMMUPath = savedIOMMUPath
	}()

	dm := manager.NewDeviceManager(manager.VirtioSCSI, nil)
	path := filepath.Join(vfioPath, testFDIOGroup)
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "c",
	}
	dev, err := dm.NewDevice(deviceInfo)
	assert.Nil(t, err, "deviceManager.NewDevice return error: %v", err)

	c := &Container{
		id: "100",
		devices: []ContainerDevice{
			{
				ID:            dev.DeviceID(),
				ContainerPath: path,
			},
		},
	}

	containers := map[string]*Container{}
	containers[c.id] = c

	sandbox := Sandbox{
		id:         "100",
		containers: containers,
		storage:    &filesystem{},
		hypervisor: &mockHypervisor{},
		devManager: dm,
	}

	containers[c.id].sandbox = &sandbox
	err = sandbox.storage.createAllResources(context.Background(), &sandbox)
	assert.Nil(t, err, "Error while create all resources for sandbox")

	err = sandbox.storeSandboxDevices()
	assert.Nil(t, err, "Error while store sandbox devices %s", err)
	err = containers[c.id].attachDevices()
	assert.Nil(t, err, "Error while attaching devices %s", err)

	err = containers[c.id].detachDevices()
	assert.Nil(t, err, "Error while detaching devices %s", err)
}

func TestSandboxCreateAssets(t *testing.T) {
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

	p := &SandboxConfig{
		Annotations: map[string]string{
			annotations.KernelPath: tmpfile.Name(),
			annotations.KernelHash: assetContentHash,
		},

		HypervisorConfig: hc,
	}

	err = createAssets(context.Background(), p)
	assert.Nil(err)

	a, ok := p.HypervisorConfig.customAssets[kernelAsset]
	assert.True(ok)
	assert.Equal(a.path, tmpfile.Name())

	p = &SandboxConfig{
		Annotations: map[string]string{
			annotations.KernelPath: tmpfile.Name(),
			annotations.KernelHash: assetContentWrongHash,
		},

		HypervisorConfig: hc,
	}

	err = createAssets(context.Background(), p)
	assert.NotNil(err)
}

func testFindContainerFailure(t *testing.T, sandbox *Sandbox, cid string) {
	c, err := sandbox.findContainer(cid)
	assert.Nil(t, c, "Container pointer should be nil")
	assert.NotNil(t, err, "Should have returned an error")
}

func TestFindContainerSandboxNilFailure(t *testing.T) {
	testFindContainerFailure(t, nil, testContainerID)
}

func TestFindContainerContainerIDEmptyFailure(t *testing.T) {
	sandbox := &Sandbox{}
	testFindContainerFailure(t, sandbox, "")
}

func TestFindContainerNoContainerMatchFailure(t *testing.T) {
	sandbox := &Sandbox{}
	testFindContainerFailure(t, sandbox, testContainerID)
}

func TestFindContainerSuccess(t *testing.T) {
	sandbox := &Sandbox{
		containers: map[string]*Container{
			testContainerID: {id: testContainerID},
		},
	}
	c, err := sandbox.findContainer(testContainerID)
	assert.NotNil(t, c, "Container pointer should not be nil")
	assert.Nil(t, err, "Should not have returned an error: %v", err)

	assert.True(t, c == sandbox.containers[testContainerID], "Container pointers should point to the same address")
}

func TestRemoveContainerSandboxNilFailure(t *testing.T) {
	testFindContainerFailure(t, nil, testContainerID)
}

func TestRemoveContainerContainerIDEmptyFailure(t *testing.T) {
	sandbox := &Sandbox{}
	testFindContainerFailure(t, sandbox, "")
}

func TestRemoveContainerNoContainerMatchFailure(t *testing.T) {
	sandbox := &Sandbox{}
	testFindContainerFailure(t, sandbox, testContainerID)
}

func TestRemoveContainerSuccess(t *testing.T) {
	sandbox := &Sandbox{
		containers: map[string]*Container{
			testContainerID: {id: testContainerID},
		},
	}
	err := sandbox.removeContainer(testContainerID)
	assert.Nil(t, err, "Should not have returned an error: %v", err)

	assert.Equal(t, len(sandbox.containers), 0, "Containers list from sandbox structure should be empty")
}

func TestCreateContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)
}

func TestDeleteContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	_, err = s.DeleteContainer(contID)
	assert.NotNil(t, err, "Deletng non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, err = s.DeleteContainer(contID)
	assert.Nil(t, err, "Failed to delete container %s in sandbox %s: %v", contID, s.ID(), err)
}

func TestStartContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	_, err = s.StartContainer(contID)
	assert.NotNil(t, err, "Starting non-existing container should fail")

	err = s.Start()
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, err = s.StartContainer(contID)
	assert.Nil(t, err, "Start container failed: %v", err)
}

func TestStatusContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	_, err = s.StatusContainer(contID)
	assert.NotNil(t, err, "Status non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, err = s.StatusContainer(contID)
	assert.Nil(t, err, "Status container failed: %v", err)

	_, err = s.DeleteContainer(contID)
	assert.Nil(t, err, "Failed to delete container %s in sandbox %s: %v", contID, s.ID(), err)
}

func TestStatusSandbox(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	s.Status()
}

func TestEnterContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	cmd := Cmd{}
	_, _, err = s.EnterContainer(contID, cmd)
	assert.NotNil(t, err, "Entering non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, _, err = s.EnterContainer(contID, cmd)
	assert.NotNil(t, err, "Entering non-running container should fail")

	err = s.Start()
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	_, _, err = s.EnterContainer(contID, cmd)
	assert.Nil(t, err, "Enter container failed: %v", err)
}

func TestMonitor(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	_, err = s.Monitor()
	assert.NotNil(t, err, "Monitoring non-running container should fail")

	err = s.Start()
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	_, err = s.Monitor()
	assert.Nil(t, err, "Monitor sandbox failed: %v", err)

	_, err = s.Monitor()
	assert.Nil(t, err, "Monitor sandbox again failed: %v", err)

	s.monitor.stop()
}

func TestWaitProcess(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "foo"
	execID := "bar"
	_, err = s.WaitProcess(contID, execID)
	assert.NotNil(t, err, "Wait process in stopped sandbox should fail")

	err = s.Start()
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	_, err = s.WaitProcess(contID, execID)
	assert.NotNil(t, err, "Wait process in non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, err = s.WaitProcess(contID, execID)
	assert.Nil(t, err, "Wait process in ready container failed: %v", err)

	_, err = s.StartContainer(contID)
	assert.Nil(t, err, "Start container failed: %v", err)

	_, err = s.WaitProcess(contID, execID)
	assert.Nil(t, err, "Wait process failed: %v", err)
}

func TestSignalProcess(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "foo"
	execID := "bar"
	err = s.SignalProcess(contID, execID, syscall.SIGKILL, true)
	assert.NotNil(t, err, "Wait process in stopped sandbox should fail")

	err = s.Start()
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	err = s.SignalProcess(contID, execID, syscall.SIGKILL, false)
	assert.NotNil(t, err, "Wait process in non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	err = s.SignalProcess(contID, execID, syscall.SIGKILL, true)
	assert.Nil(t, err, "Wait process in ready container failed: %v", err)

	_, err = s.StartContainer(contID)
	assert.Nil(t, err, "Start container failed: %v", err)

	err = s.SignalProcess(contID, execID, syscall.SIGKILL, false)
	assert.Nil(t, err, "Wait process failed: %v", err)
}

func TestWinsizeProcess(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "foo"
	execID := "bar"
	err = s.WinsizeProcess(contID, execID, 100, 200)
	assert.NotNil(t, err, "Winsize process in stopped sandbox should fail")

	err = s.Start()
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	err = s.WinsizeProcess(contID, execID, 100, 200)
	assert.NotNil(t, err, "Winsize process in non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	err = s.WinsizeProcess(contID, execID, 100, 200)
	assert.Nil(t, err, "Winsize process in ready container failed: %v", err)

	_, err = s.StartContainer(contID)
	assert.Nil(t, err, "Start container failed: %v", err)

	err = s.WinsizeProcess(contID, execID, 100, 200)
	assert.Nil(t, err, "Winsize process failed: %v", err)
}

func TestContainerProcessIOStream(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NoopAgentType, NoopNetworkModel, NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "foo"
	execID := "bar"
	_, _, _, err = s.IOStream(contID, execID)
	assert.NotNil(t, err, "Winsize process in stopped sandbox should fail")

	err = s.Start()
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	_, _, _, err = s.IOStream(contID, execID)
	assert.NotNil(t, err, "Winsize process in non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, _, _, err = s.IOStream(contID, execID)
	assert.Nil(t, err, "Winsize process in ready container failed: %v", err)

	_, err = s.StartContainer(contID)
	assert.Nil(t, err, "Start container failed: %v", err)

	_, _, _, err = s.IOStream(contID, execID)
	assert.Nil(t, err, "Winsize process failed: %v", err)
}

func TestAttachBlockDevice(t *testing.T) {
	fs := &filesystem{}
	hypervisor := &mockHypervisor{}

	hConfig := HypervisorConfig{
		BlockDeviceDriver: config.VirtioBlock,
	}

	sconfig := &SandboxConfig{
		HypervisorConfig: hConfig,
	}

	sandbox := &Sandbox{
		id:         testSandboxID,
		storage:    fs,
		hypervisor: hypervisor,
		config:     sconfig,
	}

	contID := "100"
	container := Container{
		sandbox: sandbox,
		id:      contID,
	}

	// create state file
	path := filepath.Join(runStoragePath, testSandboxID, container.ID())
	err := os.MkdirAll(path, dirMode)
	if err != nil {
		t.Fatal(err)
	}

	defer os.RemoveAll(path)

	stateFilePath := filepath.Join(path, stateFile)
	os.Remove(stateFilePath)

	_, err = os.Create(stateFilePath)
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(stateFilePath)

	path = "/dev/hda"
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "b",
	}

	dm := manager.NewDeviceManager(config.VirtioBlock, nil)
	device, err := dm.NewDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok := device.(*drivers.BlockDevice)
	assert.True(t, ok)

	container.state.State = ""
	err = device.Attach(sandbox)
	assert.Nil(t, err)

	err = device.Detach(sandbox)
	assert.Nil(t, err)

	container.state.State = StateReady
	err = device.Attach(sandbox)
	assert.Nil(t, err)

	err = device.Detach(sandbox)
	assert.Nil(t, err)

	container.sandbox.config.HypervisorConfig.BlockDeviceDriver = config.VirtioSCSI
	err = device.Attach(sandbox)
	assert.Nil(t, err)

	err = device.Detach(sandbox)
	assert.Nil(t, err)

	container.state.State = StateReady
	err = device.Attach(sandbox)
	assert.Nil(t, err)

	err = device.Detach(sandbox)
	assert.Nil(t, err)
}

func TestPreAddDevice(t *testing.T) {
	fs := &filesystem{}
	hypervisor := &mockHypervisor{}

	hConfig := HypervisorConfig{
		BlockDeviceDriver: config.VirtioBlock,
	}

	sconfig := &SandboxConfig{
		HypervisorConfig: hConfig,
	}

	dm := manager.NewDeviceManager(config.VirtioBlock, nil)
	// create a sandbox first
	sandbox := &Sandbox{
		id:         testSandboxID,
		storage:    fs,
		hypervisor: hypervisor,
		config:     sconfig,
		devManager: dm,
	}

	contID := "100"
	container := Container{
		sandbox:   sandbox,
		id:        contID,
		sandboxID: testSandboxID,
	}
	container.state.State = StateReady

	// create state file
	path := filepath.Join(runStoragePath, testSandboxID, container.ID())
	err := os.MkdirAll(path, dirMode)
	if err != nil {
		t.Fatal(err)
	}

	defer os.RemoveAll(path)

	stateFilePath := filepath.Join(path, stateFile)
	os.Remove(stateFilePath)

	_, err = os.Create(stateFilePath)
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(stateFilePath)

	path = "/dev/hda"
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "b",
	}

	// Add a mount device for a mountpoint before container's creation
	dev, err := sandbox.AddDevice(deviceInfo)
	assert.Nil(t, err)

	// in Frakti use case, here we will create and start the container
	// which will attach same device twice
	container.mounts = []Mount{
		{
			Destination:   path,
			Source:        path,
			Type:          "bind",
			BlockDeviceID: dev.DeviceID(),
		},
	}

	mounts, err := container.mountSharedDirMounts("", "")
	assert.Nil(t, err)
	assert.Equal(t, len(mounts), 0,
		"mounts should contain nothing because it only contains a block device")
}

func TestGetNetNs(t *testing.T) {
	s := Sandbox{}

	expected := ""
	netNs := s.GetNetNs()
	assert.Equal(t, netNs, expected)

	expected = "/foo/bar/ns/net"
	s.networkNS = NetworkNamespace{
		NetNsPath: expected,
	}

	netNs = s.GetNetNs()
	assert.Equal(t, netNs, expected)
}

func TestStartNetworkMonitor(t *testing.T) {
	trueBinPath, err := exec.LookPath("true")
	assert.Nil(t, err)
	assert.NotEmpty(t, trueBinPath)

	s := &Sandbox{
		id: testSandboxID,
		config: &SandboxConfig{
			NetworkConfig: NetworkConfig{
				NetmonConfig: NetmonConfig{
					Path: trueBinPath,
				},
			},
		},
		network: &defNetwork{},
		networkNS: NetworkNamespace{
			NetNsPath: fmt.Sprintf("/proc/%d/task/%d/ns/net", os.Getpid(), unix.Gettid()),
		},
	}

	err = s.startNetworkMonitor()
	assert.Nil(t, err)
}
