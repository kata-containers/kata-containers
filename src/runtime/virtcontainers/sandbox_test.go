// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strings"
	"sync"
	"syscall"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/drivers"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/manager"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	exp "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/experimental"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/fs"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

// dirMode is the permission bits used for creating a directory
const dirMode = os.FileMode(0750) | os.ModeDir

func newHypervisorConfig(kernelParams []Param, hParams []Param) HypervisorConfig {
	return HypervisorConfig{
		KernelPath:       filepath.Join(testDir, testKernel),
		ImagePath:        filepath.Join(testDir, testImage),
		HypervisorPath:   filepath.Join(testDir, testHypervisor),
		KernelParams:     kernelParams,
		HypervisorParams: hParams,
		MemorySize:       1,
	}

}

func testCreateSandbox(t *testing.T, id string,
	htype HypervisorType, hconfig HypervisorConfig,
	nconfig NetworkConfig, containers []ContainerConfig,
	volumes []types.Volume) (*Sandbox, error) {

	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	sconfig := SandboxConfig{
		ID:               id,
		HypervisorType:   htype,
		HypervisorConfig: hconfig,
		NetworkConfig:    nconfig,
		Volumes:          volumes,
		Containers:       containers,
		Annotations:      sandboxAnnotations,
		VfioMode:         config.VFIOModeGuestKernel,
	}

	ctx := WithNewAgentFunc(context.Background(), newMockAgent)
	sandbox, err := createSandbox(ctx, sconfig, nil)
	if err != nil {
		return nil, fmt.Errorf("Could not create sandbox: %s", err)
	}

	if err := sandbox.agent.startSandbox(context.Background(), sandbox); err != nil {
		return nil, err
	}

	if err := sandbox.createContainers(context.Background()); err != nil {
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

func TestCreateEmptySandbox(t *testing.T) {
	_, err := testCreateSandbox(t, testSandboxID, MockHypervisor, HypervisorConfig{}, NetworkConfig{}, nil, nil)
	assert.Error(t, err)
	defer cleanUp()
}

func TestCreateEmptyHypervisorSandbox(t *testing.T) {
	_, err := testCreateSandbox(t, testSandboxID, QemuHypervisor, HypervisorConfig{}, NetworkConfig{}, nil, nil)
	assert.Error(t, err)
	defer cleanUp()
}

func TestCreateMockSandbox(t *testing.T) {
	hConfig := newHypervisorConfig(nil, nil)
	_, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, nil, nil)
	assert.NoError(t, err)
	defer cleanUp()
}

func TestCalculateSandboxCPUs(t *testing.T) {
	sandbox := &Sandbox{}
	sandbox.config = &SandboxConfig{}

	unconstrained := newTestContainerConfigNoop("cont-00001")
	constrained := newTestContainerConfigNoop("cont-00002")
	unconstrainedCpusets0_1 := newTestContainerConfigNoop("cont-00003")
	unconstrainedCpusets2 := newTestContainerConfigNoop("cont-00004")
	constrainedCpusets0_7 := newTestContainerConfigNoop("cont-00005")
	quota := int64(4000)
	period := uint64(1000)
	constrained.Resources.CPU = &specs.LinuxCPU{Period: &period, Quota: &quota}
	unconstrainedCpusets0_1.Resources.CPU = &specs.LinuxCPU{Cpus: "0-1"}
	unconstrainedCpusets2.Resources.CPU = &specs.LinuxCPU{Cpus: "2"}
	constrainedCpusets0_7.Resources.CPU = &specs.LinuxCPU{Period: &period, Quota: &quota, Cpus: "0-7"}
	tests := []struct {
		name       string
		containers []ContainerConfig
		want       float32
	}{
		{"1-unconstrained", []ContainerConfig{unconstrained}, 0},
		{"2-unconstrained", []ContainerConfig{unconstrained, unconstrained}, 0},
		{"1-constrained", []ContainerConfig{constrained}, 4},
		{"2-constrained", []ContainerConfig{constrained, constrained}, 8},
		{"3-mix-constraints", []ContainerConfig{unconstrained, constrained, constrained}, 8},
		{"3-constrained", []ContainerConfig{constrained, constrained, constrained}, 12},
		{"unconstrained-1-cpuset", []ContainerConfig{unconstrained, unconstrained, unconstrainedCpusets0_1}, 2},
		{"unconstrained-2-cpuset", []ContainerConfig{unconstrainedCpusets0_1, unconstrainedCpusets2}, 3},
		{"constrained-cpuset", []ContainerConfig{constrainedCpusets0_7}, 4},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			sandbox.config.Containers = tt.containers
			got, _ := sandbox.calculateSandboxCPUs()
			assert.Equal(t, got, tt.want)
		})
	}
}

func TestCalculateSandboxMem(t *testing.T) {
	sandbox := &Sandbox{}
	sandbox.config = &SandboxConfig{}
	unconstrained := newTestContainerConfigNoop("cont-00001")
	constrained := newTestContainerConfigNoop("cont-00001")
	mlimit := int64(4000)
	limit := uint64(4000)
	constrained.Resources.Memory = &specs.LinuxMemory{Limit: &mlimit}

	tests := []struct {
		name       string
		containers []ContainerConfig
		want       uint64
	}{
		{"1-unconstrained", []ContainerConfig{unconstrained}, 0},
		{"2-unconstrained", []ContainerConfig{unconstrained, unconstrained}, 0},
		{"1-constrained", []ContainerConfig{constrained}, limit},
		{"2-constrained", []ContainerConfig{constrained, constrained}, limit * 2},
		{"3-mix-constraints", []ContainerConfig{unconstrained, constrained, constrained}, limit * 2},
		{"3-constrained", []ContainerConfig{constrained, constrained, constrained}, limit * 3},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			sandbox.config.Containers = tt.containers
			mem, needSwap, swap := sandbox.calculateSandboxMemory()
			assert.Equal(t, mem, tt.want)
			assert.Equal(t, needSwap, false)
			assert.Equal(t, swap, int64(0))
		})
	}
}

func TestPrepareEphemeralMounts(t *testing.T) {
	sandbox := &Sandbox{}
	sandbox.containers = map[string]*Container{
		"container1": {
			mounts: []Mount{
				{
					// happy path
					Type: KataEphemeralDevType,
					Options: []string{
						"rw",
						"relatime",
					},
					Source: "/tmp",
				},
				{
					// should be ignored because it is not KataEphemeralDevType
					Type: KataLocalDevType,
					Options: []string{
						"rw",
						"relatime",
					},
					Source: "/tmp",
				},
				{
					// should be ignored because it is sizeLimited
					Type: KataLocalDevType,
					Options: []string{
						"rw",
						"relatime",
						"size=1M",
					},
					Source: "/tmp",
				},
			},
		},
	}

	storages, err := sandbox.prepareEphemeralMounts(1024)
	assert.NoError(t, err)
	assert.Equal(t, len(storages), 1)
	for _, s := range storages {
		assert.Equal(t, s.Driver, KataEphemeralDevType)
		assert.Equal(t, s.Source, "tmpfs")
		assert.Equal(t, s.Fstype, "tmpfs")
		assert.Equal(t, s.MountPoint, filepath.Join(ephemeralPath(), "tmp"))
		assert.Equal(t, len(s.Options), 2) // remount, size=1024M

		validSet := map[string]struct{}{
			"remount":    {},
			"size=1024M": {},
		}
		for _, opt := range s.Options {
			if _, ok := validSet[opt]; !ok {
				t.Error("invalid mount opt: " + opt)
			}
		}
	}
}

func TestCalculateSandboxMemHandlesNegativeLimits(t *testing.T) {
	sandbox := &Sandbox{}
	sandbox.config = &SandboxConfig{}
	container := newTestContainerConfigNoop("cont-00001")
	limit := int64(-1)
	container.Resources.Memory = &specs.LinuxMemory{Limit: &limit}

	sandbox.config.Containers = []ContainerConfig{container}
	mem, needSwap, swap := sandbox.calculateSandboxMemory()
	assert.Equal(t, mem, uint64(0))
	assert.Equal(t, needSwap, false)
	assert.Equal(t, swap, int64(0))
}

func TestCreateSandboxEmptyID(t *testing.T) {
	hConfig := newHypervisorConfig(nil, nil)
	_, err := testCreateSandbox(t, "", MockHypervisor, hConfig, NetworkConfig{}, nil, nil)
	assert.Error(t, err)
	defer cleanUp()
}

func testCheckInitSandboxAndContainerStates(p *Sandbox, initialSandboxState types.SandboxState, c *Container, initialContainerState types.ContainerState) error {
	if p.state.State != initialSandboxState.State {
		return fmt.Errorf("Expected sandbox state %v, got %v", initialSandboxState.State, p.state.State)
	}

	if c.state.State != initialContainerState.State {
		return fmt.Errorf("Expected container state %v, got %v", initialContainerState.State, c.state.State)
	}

	return nil
}

func testForceSandboxStateChangeAndCheck(t *testing.T, p *Sandbox, newSandboxState types.SandboxState) error {
	// force sandbox state change
	err := p.setSandboxState(newSandboxState.State)
	assert.NoError(t, err)
	// Check the in-memory state is correct
	if p.state.State != newSandboxState.State {
		return fmt.Errorf("Expected state %v, got %v", newSandboxState.State, p.state.State)
	}

	return nil
}

func testForceContainerStateChangeAndCheck(t *testing.T, p *Sandbox, c *Container, newContainerState types.ContainerState) error {
	// force container state change
	err := c.setContainerState(newContainerState.State)
	assert.NoError(t, err)

	// Check the in-memory state is correct
	if c.state.State != newContainerState.State {
		return fmt.Errorf("Expected state %v, got %v", newContainerState.State, c.state.State)
	}

	return nil
}

func testCheckSandboxOnDiskState(p *Sandbox, sandboxState types.SandboxState) error {
	// Check on-disk state is correct
	if p.state.State != sandboxState.State {
		return fmt.Errorf("Expected state %v, got %v", sandboxState.State, p.state.State)
	}

	return nil
}

func testCheckContainerOnDiskState(c *Container, containerState types.ContainerState) error {
	// Check on-disk state is correct
	if c.state.State != containerState.State {
		return fmt.Errorf("Expected state %v, got %v", containerState.State, c.state.State)
	}

	return nil
}

// writeContainerConfig write config.json to bundle path
// and return bundle path.
// NOTE: the bundle path is automatically removed by t.Cleanup
func writeContainerConfig(t *testing.T) (string, error) {

	basicSpec := `
{
	"ociVersion": "1.0.0-rc2-dev",
	"process": {
		"Capabilities": [
		]
	}
}`

	configDir := t.TempDir()

	err := os.MkdirAll(configDir, DirMode)
	if err != nil {
		return "", err
	}

	configFilePath := filepath.Join(configDir, "config.json")
	err = os.WriteFile(configFilePath, []byte(basicSpec), 0644)
	if err != nil {
		return "", err
	}

	return configDir, nil
}

func TestSandboxSetSandboxAndContainerState(t *testing.T) {
	contID := "505"
	contConfig := newTestContainerConfigNoop(contID)
	assert := assert.New(t)

	configDir, err := writeContainerConfig(t)
	assert.NoError(err)

	// set bundle path annotation, fetchSandbox need this annotation to get containers
	contConfig.Annotations[vcAnnotations.BundlePathKey] = configDir

	hConfig := newHypervisorConfig(nil, nil)

	// create a sandbox
	p, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, []ContainerConfig{contConfig}, nil)
	assert.NoError(err)
	defer cleanUp()

	l := len(p.GetAllContainers())
	assert.Equal(l, 1)

	initialSandboxState := types.SandboxState{
		State: types.StateReady,
	}

	// After a sandbox creation, a container has a READY state
	initialContainerState := types.ContainerState{
		State: types.StateReady,
	}

	c, err := p.findContainer(contID)
	assert.NoError(err)

	// Check initial sandbox and container states
	if err := testCheckInitSandboxAndContainerStates(p, initialSandboxState, c, initialContainerState); err != nil {
		t.Error(err)
	}

	// persist to disk
	err = p.storeSandbox(p.ctx)
	assert.NoError(err)

	newSandboxState := types.SandboxState{
		State: types.StateRunning,
	}

	if err := testForceSandboxStateChangeAndCheck(t, p, newSandboxState); err != nil {
		t.Error(err)
	}

	newContainerState := types.ContainerState{
		State: types.StateStopped,
	}

	if err := testForceContainerStateChangeAndCheck(t, p, c, newContainerState); err != nil {
		t.Error(err)
	}

	// force state to be read from disk
	p2, err := fetchSandbox(context.Background(), p.ID())
	assert.NoError(err)

	if err := testCheckSandboxOnDiskState(p2, newSandboxState); err != nil {
		t.Error(err)
	}

	c2, err := p2.findContainer(contID)
	assert.NoError(err)

	if err := testCheckContainerOnDiskState(c2, newContainerState); err != nil {
		t.Error(err)
	}

	// revert sandbox state to allow it to be deleted
	err = p.setSandboxState(initialSandboxState.State)
	assert.NoError(err)

	// clean up
	err = p.Delete(context.Background())
	assert.NoError(err)
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
	assert.Nil(t, c)

	for _, id := range containerIDs {
		c = sandbox.GetContainer(id)
		assert.NotNil(t, c)
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
		assert.NotNil(t, containers[c.ID()], nil)
	}
}

func TestSetAnnotations(t *testing.T) {
	assert := assert.New(t)
	sandbox := Sandbox{
		ctx:             context.Background(),
		id:              "abcxyz123",
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
	assert.NoError(err)
	assert.Equal(v, valueAnnotation)

	//Change the value of an annotation
	valueAnnotation = "123"
	newAnnotations[keyAnnotation] = valueAnnotation

	sandbox.SetAnnotations(newAnnotations)

	v, err = sandbox.Annotations(keyAnnotation)
	assert.NoError(err)
	assert.Equal(v, valueAnnotation)
}

func TestSandboxGetContainer(t *testing.T) {
	assert := assert.New(t)

	emptySandbox := Sandbox{}
	_, err := emptySandbox.findContainer("")
	assert.Error(err)

	_, err = emptySandbox.findContainer("foo")
	assert.Error(err)

	hConfig := newHypervisorConfig(nil, nil)
	p, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, nil, nil)
	assert.NoError(err)
	defer cleanUp()

	contID := "999"
	contConfig := newTestContainerConfigNoop(contID)
	nc, err := newContainer(context.Background(), p, &contConfig)
	assert.NoError(err)

	err = p.addContainer(nc)
	assert.NoError(err)

	got := false
	for _, c := range p.GetAllContainers() {
		c2, err := p.findContainer(c.ID())
		assert.NoError(err)
		assert.Equal(c2.ID(), c.ID())

		if c2.ID() == contID {
			got = true
		}
	}

	assert.True(got)
}

func TestContainerStateSetFstype(t *testing.T) {
	var err error
	assert := assert.New(t)

	containers := []ContainerConfig{
		{
			ID:          "100",
			Annotations: containerAnnotations,
			CustomSpec:  newEmptySpec(),
		},
	}

	hConfig := newHypervisorConfig(nil, nil)
	sandbox, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, containers, nil)
	assert.Nil(err)
	defer cleanUp()

	c := sandbox.GetContainer("100")
	assert.NotNil(c)

	cImpl, ok := c.(*Container)
	assert.True(ok)

	state := types.ContainerState{
		State:  "ready",
		Fstype: "vfs",
	}

	cImpl.state = state

	newFstype := "ext4"
	err = cImpl.setStateFstype(newFstype)
	assert.NoError(err)
	assert.Equal(cImpl.state.Fstype, newFstype)
}

func TestSandboxAttachDevicesVFIO(t *testing.T) {
	tmpDir := t.TempDir()
	os.RemoveAll(tmpDir)

	testFDIOGroup := "2"
	testDeviceBDFPath := "0000:00:1c.0"

	devicesDir := filepath.Join(tmpDir, testFDIOGroup, "devices")
	err := os.MkdirAll(devicesDir, DirMode)
	assert.Nil(t, err)

	deviceFile := filepath.Join(devicesDir, testDeviceBDFPath)
	_, err = os.Create(deviceFile)
	assert.Nil(t, err)

	savedIOMMUPath := config.SysIOMMUGroupPath
	config.SysIOMMUGroupPath = tmpDir

	defer func() {
		config.SysIOMMUGroupPath = savedIOMMUPath
	}()

	dm := manager.NewDeviceManager(config.VirtioSCSI, false, "", 0, nil)
	path := filepath.Join(vfioPath, testFDIOGroup)
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "c",
		Port:          config.RootPort,
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
		hypervisor: &mockHypervisor{},
		devManager: dm,
		ctx:        context.Background(),
		config:     &SandboxConfig{},
	}

	containers[c.id].sandbox = &sandbox

	err = containers[c.id].attachDevices(context.Background())
	assert.Nil(t, err, "Error while attaching devices %s", err)

	err = containers[c.id].detachDevices(context.Background())
	assert.Nil(t, err, "Error while detaching devices %s", err)
}

var assetContent = []byte("FakeAsset fake asset FAKE ASSET")
var assetContentHash = "92549f8d2018a95a294d28a65e795ed7d1a9d150009a28cea108ae10101178676f04ab82a6950d0099e4924f9c5e41dcba8ece56b75fc8b4e0a7492cb2a8c880"
var assetContentWrongHash = "92549f8d2018a95a294d28a65e795ed7d1a9d150009a28cea108ae10101178676f04ab82a6950d0099e4924f9c5e41dcba8ece56b75fc8b4e0a7492cb2a8c881"

func TestSandboxCreateAssets(t *testing.T) {
	assert := assert.New(t)

	// nolint: govet
	type testData struct {
		assetType   types.AssetType
		annotations map[string]string
	}

	tmpfile, err := os.CreateTemp("", "virtcontainers-test-")
	assert.Nil(err)

	filename := tmpfile.Name()

	defer func() {
		tmpfile.Close()
		os.Remove(filename) // clean up
	}()

	_, err = tmpfile.Write(assetContent)
	assert.Nil(err)

	originalKernelPath := filepath.Join(testDir, testKernel)
	originalImagePath := filepath.Join(testDir, testImage)
	originalInitrdPath := filepath.Join(testDir, testInitrd)
	originalFirmwarePath := filepath.Join(testDir, testFirmware)
	originalHypervisorPath := filepath.Join(testDir, testHypervisor)
	originalJailerPath := filepath.Join(testDir, testJailer)

	hc := HypervisorConfig{
		KernelPath:     originalKernelPath,
		ImagePath:      originalImagePath,
		InitrdPath:     originalInitrdPath,
		FirmwarePath:   originalFirmwarePath,
		HypervisorPath: originalHypervisorPath,
		JailerPath:     originalJailerPath,
	}

	data := []testData{
		{
			types.FirmwareAsset,
			map[string]string{
				annotations.FirmwarePath: filename,
				annotations.FirmwareHash: assetContentHash,
			},
		},
		{
			types.HypervisorAsset,
			map[string]string{
				annotations.HypervisorPath: filename,
				annotations.HypervisorHash: assetContentHash,
			},
		},
		{
			types.ImageAsset,
			map[string]string{
				annotations.ImagePath: filename,
				annotations.ImageHash: assetContentHash,
			},
		},
		{
			types.InitrdAsset,
			map[string]string{
				annotations.InitrdPath: filename,
				annotations.InitrdHash: assetContentHash,
			},
		},
		{
			types.JailerAsset,
			map[string]string{
				annotations.JailerPath: filename,
				annotations.JailerHash: assetContentHash,
			},
		},
		{
			types.KernelAsset,
			map[string]string{
				annotations.KernelPath: filename,
				annotations.KernelHash: assetContentHash,
			},
		},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		config := &SandboxConfig{
			Annotations:      d.annotations,
			HypervisorConfig: hc,
		}

		err = createAssets(context.Background(), config)
		assert.NoError(err, msg)

		a, ok := config.HypervisorConfig.customAssets[d.assetType]
		assert.True(ok, msg)
		assert.Equal(a.Path(), filename, msg)

		// Now test with invalid hashes
		badHashAnnotations := make(map[string]string)
		for k, v := range d.annotations {
			if strings.HasSuffix(k, "_hash") {
				badHashAnnotations[k] = assetContentWrongHash
			} else {
				badHashAnnotations[k] = v
			}
		}

		config = &SandboxConfig{
			Annotations:      badHashAnnotations,
			HypervisorConfig: hc,
		}

		err = createAssets(context.Background(), config)
		assert.Error(err, msg)
	}

	// Remote Hypervisor scenario for ImagePath
	msg := "test[image]: imagePath"
	imagePathData := &testData{
		assetType: types.ImageAsset,
		annotations: map[string]string{
			annotations.ImagePath: "rhel9-os",
		},
	}

	config := &SandboxConfig{
		Annotations:      imagePathData.annotations,
		HypervisorConfig: hc,
		HypervisorType:   RemoteHypervisor,
	}

	err = createAssets(context.Background(), config)
	assert.NoError(err, msg)
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
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	assert.Equal(t, len(s.config.Containers), 1, "Container config list length from sandbox structure should be 1")

	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.NotNil(t, err, "Should failed to create a duplicated container")
	assert.Equal(t, len(s.config.Containers), 1, "Container config list length from sandbox structure should be 1")
}

func TestDeleteContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	_, err = s.DeleteContainer(context.Background(), contID)
	assert.NotNil(t, err, "Deletng non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, err = s.DeleteContainer(context.Background(), contID)
	assert.Nil(t, err, "Failed to delete container %s in sandbox %s: %v", contID, s.ID(), err)
}

func TestStartContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	_, err = s.StartContainer(context.Background(), contID)
	assert.NotNil(t, err, "Starting non-existing container should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, err = s.StartContainer(context.Background(), contID)
	assert.Nil(t, err, "Start container failed: %v", err)
}

func TestStatusContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	_, err = s.StatusContainer(contID)
	assert.NotNil(t, err, "Status non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, err = s.StatusContainer(contID)
	assert.Nil(t, err, "Status container failed: %v", err)

	_, err = s.DeleteContainer(context.Background(), contID)
	assert.Nil(t, err, "Failed to delete container %s in sandbox %s: %v", contID, s.ID(), err)
}

func TestStatusSandbox(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	s.Status()
}

func TestEnterContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	cmd := types.Cmd{}
	_, _, err = s.EnterContainer(context.Background(), contID, cmd)
	assert.NotNil(t, err, "Entering non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, _, err = s.EnterContainer(context.Background(), contID, cmd)
	assert.NotNil(t, err, "Entering non-running container should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	_, _, err = s.EnterContainer(context.Background(), contID, cmd)
	assert.Nil(t, err, "Enter container failed: %v", err)
}

func TestDeleteStoreWhenNewContainerFail(t *testing.T) {
	hConfig := newHypervisorConfig(nil, nil)
	p, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	contID := "999"
	contConfig := newTestContainerConfigNoop(contID)
	contConfig.DeviceInfos = []config.DeviceInfo{
		{
			ContainerPath: "",
			DevType:       "",
		},
	}
	_, err = newContainer(context.Background(), p, &contConfig)
	assert.NotNil(t, err, "New container with invalid device info should fail")
	storePath := filepath.Join(p.store.RunStoragePath(), testSandboxID, contID)
	_, err = os.Stat(storePath)
	assert.NotNil(t, err, "Should delete configuration root after failed to create a container")
}

func TestMonitor(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	_, err = s.Monitor(context.Background())
	assert.NotNil(t, err, "Monitoring non-running container should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	_, err = s.Monitor(context.Background())
	assert.Nil(t, err, "Monitor sandbox failed: %v", err)

	_, err = s.Monitor(context.Background())
	assert.Nil(t, err, "Monitor sandbox again failed: %v", err)

	s.monitor.stop()
}

func TestWaitProcess(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "foo"
	execID := "bar"
	_, err = s.WaitProcess(context.Background(), contID, execID)
	assert.NotNil(t, err, "Wait process in stopped sandbox should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	_, err = s.WaitProcess(context.Background(), contID, execID)
	assert.NotNil(t, err, "Wait process in non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, err = s.WaitProcess(context.Background(), contID, execID)
	assert.Nil(t, err, "Wait process in ready container failed: %v", err)

	_, err = s.StartContainer(context.Background(), contID)
	assert.Nil(t, err, "Start container failed: %v", err)

	_, err = s.WaitProcess(context.Background(), contID, execID)
	assert.Nil(t, err, "Wait process failed: %v", err)
}

func TestSignalProcess(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "foo"
	execID := "bar"
	err = s.SignalProcess(context.Background(), contID, execID, syscall.SIGKILL, true)
	assert.NotNil(t, err, "Wait process in stopped sandbox should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	err = s.SignalProcess(context.Background(), contID, execID, syscall.SIGKILL, false)
	assert.NotNil(t, err, "Wait process in non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	err = s.SignalProcess(context.Background(), contID, execID, syscall.SIGKILL, true)
	assert.Nil(t, err, "Wait process in ready container failed: %v", err)

	_, err = s.StartContainer(context.Background(), contID)
	assert.Nil(t, err, "Start container failed: %v", err)

	err = s.SignalProcess(context.Background(), contID, execID, syscall.SIGKILL, false)
	assert.Nil(t, err, "Wait process failed: %v", err)
}

func TestWinsizeProcess(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "foo"
	execID := "bar"
	err = s.WinsizeProcess(context.Background(), contID, execID, 100, 200)
	assert.NotNil(t, err, "Winsize process in stopped sandbox should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	err = s.WinsizeProcess(context.Background(), contID, execID, 100, 200)
	assert.NotNil(t, err, "Winsize process in non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	err = s.WinsizeProcess(context.Background(), contID, execID, 100, 200)
	assert.Nil(t, err, "Winsize process in ready container failed: %v", err)

	_, err = s.StartContainer(context.Background(), contID)
	assert.Nil(t, err, "Start container failed: %v", err)

	err = s.WinsizeProcess(context.Background(), contID, execID, 100, 200)
	assert.Nil(t, err, "Winsize process failed: %v", err)
}

func TestContainerProcessIOStream(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "foo"
	execID := "bar"
	_, _, _, err = s.IOStream(contID, execID)
	assert.NotNil(t, err, "Winsize process in stopped sandbox should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	_, _, _, err = s.IOStream(contID, execID)
	assert.NotNil(t, err, "Winsize process in non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, _, _, err = s.IOStream(contID, execID)
	assert.Nil(t, err, "Winsize process in ready container failed: %v", err)

	_, err = s.StartContainer(context.Background(), contID)
	assert.Nil(t, err, "Start container failed: %v", err)

	_, _, _, err = s.IOStream(contID, execID)
	assert.Nil(t, err, "Winsize process failed: %v", err)
}

func TestAttachBlockDevice(t *testing.T) {
	hypervisor := &mockHypervisor{}

	hConfig := HypervisorConfig{
		BlockDeviceDriver: config.VirtioBlock,
	}

	sconfig := &SandboxConfig{
		HypervisorConfig: hConfig,
	}

	sandbox := &Sandbox{
		id:         testSandboxID,
		hypervisor: hypervisor,
		config:     sconfig,
		ctx:        context.Background(),
		state:      types.SandboxState{BlockIndexMap: make(map[int]struct{})},
	}

	contID := "100"
	container := Container{
		sandbox: sandbox,
		id:      contID,
	}

	// create state file
	path := filepath.Join(fs.MockRunStoragePath(), testSandboxID, container.ID())
	err := os.MkdirAll(path, DirMode)
	assert.NoError(t, err)

	defer os.RemoveAll(path)

	path = "/dev/hda"
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "b",
	}

	dm := manager.NewDeviceManager(config.VirtioBlock, false, "", 0, nil)
	device, err := dm.NewDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok := device.(*drivers.BlockDevice)
	assert.True(t, ok)

	container.state.State = ""
	index, err := sandbox.getAndSetSandboxBlockIndex()
	assert.Nil(t, err)
	assert.Equal(t, index, 0)

	err = device.Attach(context.Background(), sandbox)
	assert.Nil(t, err)
	index, err = sandbox.getAndSetSandboxBlockIndex()
	assert.Nil(t, err)
	assert.Equal(t, index, 2)

	err = device.Detach(context.Background(), sandbox)
	assert.Nil(t, err)
	index, err = sandbox.getAndSetSandboxBlockIndex()
	assert.Nil(t, err)
	assert.Equal(t, index, 1)

	container.state.State = types.StateReady
	err = device.Attach(context.Background(), sandbox)
	assert.Nil(t, err)

	err = device.Detach(context.Background(), sandbox)
	assert.Nil(t, err)

	container.sandbox.config.HypervisorConfig.BlockDeviceDriver = config.VirtioSCSI
	err = device.Attach(context.Background(), sandbox)
	assert.Nil(t, err)

	err = device.Detach(context.Background(), sandbox)
	assert.Nil(t, err)

	container.state.State = types.StateReady
	err = device.Attach(context.Background(), sandbox)
	assert.Nil(t, err)

	err = device.Detach(context.Background(), sandbox)
	assert.Nil(t, err)
}

func TestPreAddDevice(t *testing.T) {
	hypervisor := &mockHypervisor{}

	hConfig := HypervisorConfig{
		BlockDeviceDriver: config.VirtioBlock,
	}

	sconfig := &SandboxConfig{
		HypervisorConfig: hConfig,
	}

	dm := manager.NewDeviceManager(config.VirtioBlock, false, "", 0, nil)
	// create a sandbox first
	sandbox := &Sandbox{
		id:         testSandboxID,
		hypervisor: hypervisor,
		config:     sconfig,
		devManager: dm,
		ctx:        context.Background(),
		state:      types.SandboxState{BlockIndexMap: make(map[int]struct{})},
	}

	contID := "100"
	container := Container{
		sandbox:   sandbox,
		id:        contID,
		sandboxID: testSandboxID,
	}
	container.state.State = types.StateReady

	// create state file
	path := filepath.Join(fs.MockRunStoragePath(), testSandboxID, container.ID())
	err := os.MkdirAll(path, DirMode)
	assert.NoError(t, err)

	defer os.RemoveAll(path)

	path = "/dev/hda"
	deviceInfo := config.DeviceInfo{
		HostPath:      path,
		ContainerPath: path,
		DevType:       "b",
	}

	// Add a mount device for a mountpoint before container's creation
	dev, err := sandbox.AddDevice(context.Background(), deviceInfo)
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

	mounts := make(map[string]Mount)
	ignoreMounts := make(map[string]Mount)
	_, err = container.mountSharedDirMounts(context.Background(), mounts, ignoreMounts)

	assert.Nil(t, err)
	assert.Equal(t, len(mounts), 0,
		"mounts should contain nothing because it only contains a block device")
	assert.Equal(t, len(ignoreMounts), 0,
		"ignoreMounts should contain nothing because it only contains a block device")
}

func TestGetNetNs(t *testing.T) {
	s := Sandbox{}

	expected := "/foo/bar/ns/net"
	network, err := NewNetwork(&NetworkConfig{NetworkID: expected})
	assert.Nil(t, err)

	s.network = network
	netNs := s.GetNetNs()
	assert.Equal(t, netNs, expected)
}

func TestSandboxStopStopped(t *testing.T) {
	s := &Sandbox{
		ctx:   context.Background(),
		state: types.SandboxState{State: types.StateStopped},
	}
	err := s.Stop(context.Background(), false)

	assert.Nil(t, err)
}

func checkDirNotExist(path string) error {
	if _, err := os.Stat(path); os.IsExist(err) {
		return fmt.Errorf("%s is still exists", path)
	}
	return nil
}

func checkSandboxRemains() error {
	var err error
	if err = checkDirNotExist(sandboxDirState); err != nil {
		return fmt.Errorf("%s still exists", sandboxDirState)
	}
	if err = checkDirNotExist(path.Join(kataHostSharedDir(), testSandboxID)); err != nil {
		return fmt.Errorf("%s still exists", path.Join(kataHostSharedDir(), testSandboxID))
	}

	return nil
}

func TestSandboxCreationFromConfigRollbackFromCreateSandbox(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)
	ctx := context.Background()
	hConf := newHypervisorConfig(nil, nil)
	sConf := SandboxConfig{
		ID:               testSandboxID,
		HypervisorType:   QemuHypervisor,
		HypervisorConfig: hConf,
		NetworkConfig:    NetworkConfig{},
		Volumes:          nil,
		Containers:       nil,
	}

	// Ensure hypervisor doesn't exist
	assert.NoError(os.Remove(hConf.HypervisorPath))

	_, err := createSandboxFromConfig(ctx, sConf, nil, nil)
	// Fail at createSandbox: QEMU path does not exist, it is expected. Then rollback is called
	assert.Error(err)

	// Check dirs
	err = checkSandboxRemains()
	assert.NoError(err)
}

func TestSandboxUpdateResources(t *testing.T) {
	contConfig1 := newTestContainerConfigNoop("cont-00001")
	contConfig2 := newTestContainerConfigNoop("cont-00002")
	hConfig := newHypervisorConfig(nil, nil)
	defer cleanUp()
	// create a sandbox
	s, err := testCreateSandbox(t,
		testSandboxID,
		MockHypervisor,
		hConfig,
		NetworkConfig{},
		[]ContainerConfig{contConfig1, contConfig2},
		nil)
	assert.NoError(t, err)

	err = s.updateResources(context.Background())
	assert.NoError(t, err)

	// For mock hypervisor, we MemSlots to be 0 since the memory wasn't changed.
	assert.Equal(t, s.hypervisor.HypervisorConfig().MemSlots, uint32(0))

	containerMemLimit := int64(4 * 1024 * 1024 * 1024)
	containerCPUPeriod := uint64(1000)
	containerCPUQouta := int64(5)
	for idx := range s.config.Containers {
		s.config.Containers[idx].Resources.Memory = &specs.LinuxMemory{
			Limit: new(int64),
		}
		s.config.Containers[idx].Resources.CPU = &specs.LinuxCPU{
			Period: new(uint64),
			Quota:  new(int64),
		}
		s.config.Containers[idx].Resources.Memory.Limit = &containerMemLimit
		s.config.Containers[idx].Resources.CPU.Period = &containerCPUPeriod
		s.config.Containers[idx].Resources.CPU.Quota = &containerCPUQouta
	}
	err = s.updateResources(context.Background())
	assert.NoError(t, err)

	// Since we're starting with a memory of 1 MB, we expect it to take 3 hotplugs to add 4GiB of memory when using ACPI hotplug:
	// +48MB
	// +2352MB
	// +the remaining
	assert.Equal(t, s.hypervisor.HypervisorConfig().MemSlots, uint32(3))
}

func TestSandboxExperimentalFeature(t *testing.T) {
	testFeature := exp.Feature{
		Name:        "mock",
		Description: "exp feature for test",
		ExpRelease:  "1.8.0",
	}
	sconfig := SandboxConfig{
		ID:           testSandboxID,
		Experimental: []exp.Feature{testFeature},
	}

	assert.Nil(t, exp.Get(testFeature.Name))
	assert.False(t, sconfig.valid())

	exp.Register(testFeature)
	assert.NotNil(t, exp.Get(testFeature.Name))
	assert.True(t, sconfig.valid())
}

func TestSandbox_Cgroups(t *testing.T) {
	sandboxContainer := ContainerConfig{}
	sandboxContainer.Annotations = make(map[string]string)
	sandboxContainer.Annotations[annotations.ContainerTypeKey] = string(PodSandbox)

	emptyJSONLinux := ContainerConfig{
		CustomSpec: newEmptySpec(),
	}
	emptyJSONLinux.Annotations = make(map[string]string)
	emptyJSONLinux.Annotations[annotations.ContainerTypeKey] = string(PodSandbox)

	cloneSpec1 := newEmptySpec()
	cloneSpec1.Linux.CgroupsPath = "/myRuntime/myContainer"
	successfulContainer := ContainerConfig{
		CustomSpec: cloneSpec1,
	}
	successfulContainer.Annotations = make(map[string]string)
	successfulContainer.Annotations[annotations.ContainerTypeKey] = string(PodSandbox)

	// nolint: govet
	tests := []struct {
		name     string
		s        *Sandbox
		wantErr  bool
		needRoot bool
	}{
		{
			"New sandbox",
			&Sandbox{},
			true,
			false,
		},
		{
			"New sandbox, new config",
			&Sandbox{config: &SandboxConfig{}},
			false,
			true,
		},
		{
			"sandbox, container no sandbox type",
			&Sandbox{
				config: &SandboxConfig{Containers: []ContainerConfig{
					{},
				}}},
			false,
			true,
		},
		{
			"sandbox, container sandbox type",
			&Sandbox{
				config: &SandboxConfig{Containers: []ContainerConfig{
					sandboxContainer,
				}}},
			false,
			true,
		},
		{
			"sandbox, empty linux json",
			&Sandbox{
				config: &SandboxConfig{Containers: []ContainerConfig{
					emptyJSONLinux,
				}}},
			false,
			true,
		},
		{
			"sandbox, successful config",
			&Sandbox{
				config: &SandboxConfig{Containers: []ContainerConfig{
					successfulContainer,
				}}},
			false,
			true,
		},
	}
	for _, tt := range tests {
		if tt.needRoot && os.Getuid() != 0 {
			t.Skip(tt.name + "needs root")
		}

		t.Run(tt.name, func(t *testing.T) {
			err := tt.s.createResourceController()
			t.Logf("create groups error %v", err)
			if (err != nil) != tt.wantErr {
				t.Errorf("Sandbox.CreateCgroups() error = %v, wantErr %v", err, tt.wantErr)
			}

			if err == nil {
				if err := tt.s.setupResourceController(); (err != nil) != tt.wantErr {
					t.Errorf("Sandbox.SetupCgroups() error = %v, wantErr %v", err, tt.wantErr)
				}
			}
		})
	}
}

func getContainerConfigWithCPUSet(cpuset, memset string) ContainerConfig {
	return ContainerConfig{
		Resources: specs.LinuxResources{
			CPU: &specs.LinuxCPU{
				Cpus: cpuset,
				Mems: memset,
			},
		},
	}
}

func getSimpleSandbox(cpusets, memsets [3]string) *Sandbox {
	sandbox := Sandbox{}

	sandbox.config = &SandboxConfig{
		Containers: []ContainerConfig{
			getContainerConfigWithCPUSet(cpusets[0], memsets[0]),
			getContainerConfigWithCPUSet(cpusets[1], memsets[1]),
			getContainerConfigWithCPUSet(cpusets[2], memsets[2]),
		},
	}

	return &sandbox
}

func TestGetSandboxCpuSet(t *testing.T) {

	tests := []struct {
		name      string
		cpusets   [3]string
		memsets   [3]string
		cpuResult string
		memResult string
		wantErr   bool
	}{
		{
			"single, no cpuset",
			[3]string{"", "", ""},
			[3]string{"", "", ""},
			"",
			"",
			false,
		},
		{
			"single cpuset",
			[3]string{"0", "", ""},
			[3]string{"", "", ""},
			"0",
			"",
			false,
		},
		{
			"two duplicate cpuset",
			[3]string{"0", "0", ""},
			[3]string{"", "", ""},
			"0",
			"",
			false,
		},
		{
			"3 cpusets",
			[3]string{"0-3", "5-7", "1"},
			[3]string{"", "", ""},
			"0-3,5-7",
			"",
			false,
		},

		{
			"weird, but should be okay",
			[3]string{"0-3", "99999", ""},
			[3]string{"", "", ""},
			"0-3,99999",
			"",
			false,
		},
		{
			"two, overlapping cpuset",
			[3]string{"0-3", "1-2", ""},
			[3]string{"", "", ""},
			"0-3",
			"",
			false,
		},
		{
			"garbage, should fail",
			[3]string{"7 beard-seconds", "Audrey + 7", "Elliott - 17"},
			[3]string{"", "", ""},
			"",
			"",
			true,
		},
		{
			"cpuset and memset",
			[3]string{"0-3", "1-2", ""},
			[3]string{"0", "1", "0-1"},
			"0-3",
			"0-1",
			false,
		},
		{
			"memset",
			[3]string{"0-3", "1-2", ""},
			[3]string{"0", "3", ""},
			"0-3",
			"0,3",
			false,
		},
	}
	for _, tt := range tests {

		t.Run(tt.name, func(t *testing.T) {
			s := getSimpleSandbox(tt.cpusets, tt.memsets)
			res, _, err := s.getSandboxCPUSet()
			if (err != nil) != tt.wantErr {
				t.Errorf("getSandboxCPUSet() error = %v, wantErr %v", err, tt.wantErr)
			}
			if res != tt.cpuResult {
				t.Errorf("getSandboxCPUSet() result = %s, wanted result %s", res, tt.cpuResult)
			}
		})
	}
}

func TestSandboxHugepageLimit(t *testing.T) {
	contConfig1 := newTestContainerConfigNoop("cont-00001")
	contConfig2 := newTestContainerConfigNoop("cont-00002")
	limit := int64(4000)
	contConfig1.Resources.Memory = &specs.LinuxMemory{Limit: &limit}
	contConfig2.Resources.Memory = &specs.LinuxMemory{Limit: &limit}
	hConfig := newHypervisorConfig(nil, nil)

	defer cleanUp()
	// create a sandbox
	s, err := testCreateSandbox(t,
		testSandboxID,
		MockHypervisor,
		hConfig,
		NetworkConfig{},
		[]ContainerConfig{contConfig1, contConfig2},
		nil)

	assert.NoError(t, err)

	hugepageLimits := []specs.LinuxHugepageLimit{
		{
			Pagesize: "1GB",
			Limit:    322122547,
		},
		{
			Pagesize: "2MB",
			Limit:    134217728,
		},
	}

	for i := range s.config.Containers {
		s.config.Containers[i].Resources.HugepageLimits = hugepageLimits
	}
	err = s.updateResources(context.Background())
	assert.NoError(t, err)
}
