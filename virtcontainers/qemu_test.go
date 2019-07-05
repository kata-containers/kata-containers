// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/store"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

func newQemuConfig() HypervisorConfig {
	return HypervisorConfig{
		KernelPath:        testQemuKernelPath,
		ImagePath:         testQemuImagePath,
		InitrdPath:        testQemuInitrdPath,
		HypervisorPath:    testQemuPath,
		NumVCPUs:          defaultVCPUs,
		MemorySize:        defaultMemSzMiB,
		DefaultBridges:    defaultBridges,
		BlockDeviceDriver: defaultBlockDriver,
		DefaultMaxVCPUs:   defaultMaxQemuVCPUs,
		Msize9p:           defaultMsize9p,
	}
}

func testQemuKernelParameters(t *testing.T, kernelParams []Param, expected string, debug bool) {
	qemuConfig := newQemuConfig()
	qemuConfig.KernelParams = kernelParams

	if debug == true {
		qemuConfig.Debug = true
	}

	q := &qemu{
		config: qemuConfig,
		arch:   &qemuArchBase{},
	}

	params := q.kernelParameters()
	if params != expected {
		t.Fatalf("Got: %v, Expecting: %v", params, expected)
	}
}

func TestQemuKernelParameters(t *testing.T) {
	expectedOut := fmt.Sprintf("panic=1 nr_cpus=%d agent.use_vsock=false foo=foo bar=bar", MaxQemuVCPUs())
	params := []Param{
		{
			Key:   "foo",
			Value: "foo",
		},
		{
			Key:   "bar",
			Value: "bar",
		},
	}

	testQemuKernelParameters(t, params, expectedOut, true)
	testQemuKernelParameters(t, params, expectedOut, false)
}

func TestQemuCreateSandbox(t *testing.T) {
	qemuConfig := newQemuConfig()
	q := &qemu{}

	sandbox := &Sandbox{
		ctx: context.Background(),
		id:  "testSandbox",
		config: &SandboxConfig{
			HypervisorConfig: qemuConfig,
		},
	}

	vcStore, err := store.NewVCSandboxStore(sandbox.ctx, sandbox.id)
	if err != nil {
		t.Fatal(err)
	}
	sandbox.store = vcStore

	// Create the hypervisor fake binary
	testQemuPath := filepath.Join(testDir, testHypervisor)
	_, err = os.Create(testQemuPath)
	if err != nil {
		t.Fatalf("Could not create hypervisor file %s: %v", testQemuPath, err)
	}

	// Create parent dir path for hypervisor.json
	parentDir := store.SandboxConfigurationRootPath(sandbox.id)
	if err := os.MkdirAll(parentDir, store.DirMode); err != nil {
		t.Fatalf("Could not create parent directory %s: %v", parentDir, err)
	}

	if err := q.createSandbox(context.Background(), sandbox.id, &sandbox.config.HypervisorConfig, sandbox.store); err != nil {
		t.Fatal(err)
	}

	if err := os.RemoveAll(parentDir); err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(qemuConfig, q.config) == false {
		t.Fatalf("Got %v\nExpecting %v", q.config, qemuConfig)
	}
}

func TestQemuCreateSandboxMissingParentDirFail(t *testing.T) {
	qemuConfig := newQemuConfig()
	q := &qemu{}

	sandbox := &Sandbox{
		ctx: context.Background(),
		id:  "testSandbox",
		config: &SandboxConfig{
			HypervisorConfig: qemuConfig,
		},
	}

	vcStore, err := store.NewVCSandboxStore(sandbox.ctx, sandbox.id)
	if err != nil {
		t.Fatal(err)
	}
	sandbox.store = vcStore

	// Create the hypervisor fake binary
	testQemuPath := filepath.Join(testDir, testHypervisor)
	_, err = os.Create(testQemuPath)
	if err != nil {
		t.Fatalf("Could not create hypervisor file %s: %v", testQemuPath, err)
	}

	// Ensure parent dir path for hypervisor.json does not exist.
	parentDir := store.SandboxConfigurationRootPath(sandbox.id)
	if err := os.RemoveAll(parentDir); err != nil {
		t.Fatal(err)
	}

	if err := q.createSandbox(context.Background(), sandbox.id, &sandbox.config.HypervisorConfig, sandbox.store); err != nil {
		t.Fatalf("Qemu createSandbox() is not expected to fail because of missing parent directory for storage: %v", err)
	}
}

func TestQemuCPUTopology(t *testing.T) {
	vcpus := 1

	q := &qemu{
		arch: &qemuArchBase{},
		config: HypervisorConfig{
			NumVCPUs:        uint32(vcpus),
			DefaultMaxVCPUs: uint32(vcpus),
		},
	}

	expectedOut := govmmQemu.SMP{
		CPUs:    uint32(vcpus),
		Sockets: uint32(vcpus),
		Cores:   defaultCores,
		Threads: defaultThreads,
		MaxCPUs: uint32(vcpus),
	}

	smp := q.cpuTopology()

	if reflect.DeepEqual(smp, expectedOut) == false {
		t.Fatalf("Got %v\nExpecting %v", smp, expectedOut)
	}
}

func TestQemuMemoryTopology(t *testing.T) {
	mem := uint32(1000)
	slots := uint32(8)

	q := &qemu{
		arch: &qemuArchBase{},
		config: HypervisorConfig{
			MemorySize: mem,
			MemSlots:   slots,
		},
	}

	hostMemKb, err := getHostMemorySizeKb(procMemInfo)
	if err != nil {
		t.Fatal(err)
	}
	memMax := fmt.Sprintf("%dM", int(float64(hostMemKb)/1024))

	expectedOut := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  uint8(slots),
		MaxMem: memMax,
	}

	memory, err := q.memoryTopology()
	if err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(memory, expectedOut) == false {
		t.Fatalf("Got %v\nExpecting %v", memory, expectedOut)
	}
}

func testQemuAddDevice(t *testing.T, devInfo interface{}, devType deviceType, expected []govmmQemu.Device) {
	q := &qemu{
		ctx:  context.Background(),
		arch: &qemuArchBase{},
	}

	err := q.addDevice(devInfo, devType)
	if err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(q.qemuConfig.Devices, expected) == false {
		t.Fatalf("Got %v\nExpecting %v", q.qemuConfig.Devices, expected)
	}
}

func TestQemuAddDeviceFsDev(t *testing.T) {
	mountTag := "testMountTag"
	hostPath := "testHostPath"

	expectedOut := []govmmQemu.Device{
		govmmQemu.FSDevice{
			Driver:        govmmQemu.Virtio9P,
			FSDriver:      govmmQemu.Local,
			ID:            fmt.Sprintf("extra-9p-%s", mountTag),
			Path:          hostPath,
			MountTag:      mountTag,
			SecurityModel: govmmQemu.None,
		},
	}

	volume := types.Volume{
		MountTag: mountTag,
		HostPath: hostPath,
	}

	testQemuAddDevice(t, volume, fsDev, expectedOut)
}

func TestQemuAddDeviceSerialPortDev(t *testing.T) {
	deviceID := "channelTest"
	id := "charchTest"
	hostPath := "/tmp/hyper_test.sock"
	name := "sh.hyper.channel.test"

	expectedOut := []govmmQemu.Device{
		govmmQemu.CharDevice{
			Driver:   govmmQemu.VirtioSerialPort,
			Backend:  govmmQemu.Socket,
			DeviceID: deviceID,
			ID:       id,
			Path:     hostPath,
			Name:     name,
		},
	}

	socket := types.Socket{
		DeviceID: deviceID,
		ID:       id,
		HostPath: hostPath,
		Name:     name,
	}

	testQemuAddDevice(t, socket, serialPortDev, expectedOut)
}

func TestQemuAddDeviceKataVSOCK(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(dir)

	vsockFilename := filepath.Join(dir, "vsock")

	contextID := uint64(3)
	port := uint32(1024)

	vsockFile, err := os.Create(vsockFilename)
	assert.NoError(err)
	defer vsockFile.Close()

	expectedOut := []govmmQemu.Device{
		govmmQemu.VSOCKDevice{
			ID:        fmt.Sprintf("vsock-%d", contextID),
			ContextID: contextID,
			VHostFD:   vsockFile,
		},
	}

	vsock := kataVSOCK{
		contextID: contextID,
		port:      port,
		vhostFd:   vsockFile,
	}

	testQemuAddDevice(t, vsock, vSockPCIDev, expectedOut)
}

func TestQemuGetSandboxConsole(t *testing.T) {
	q := &qemu{
		ctx: context.Background(),
	}
	sandboxID := "testSandboxID"
	expected := filepath.Join(store.RunVMStoragePath, sandboxID, consoleSocket)

	result, err := q.getSandboxConsole(sandboxID)
	if err != nil {
		t.Fatal(err)
	}

	if result != expected {
		t.Fatalf("Got %s\nExpecting %s", result, expected)
	}
}

func TestQemuCapabilities(t *testing.T) {
	q := &qemu{
		ctx:  context.Background(),
		arch: &qemuArchBase{},
	}

	caps := q.capabilities()
	if !caps.IsBlockDeviceHotplugSupported() {
		t.Fatal("Block device hotplug should be supported")
	}
}

func TestQemuQemuPath(t *testing.T) {
	assert := assert.New(t)

	f, err := ioutil.TempFile("", "qemu")
	assert.NoError(err)
	defer func() { _ = f.Close() }()
	defer func() { _ = os.Remove(f.Name()) }()

	expectedPath := f.Name()
	qemuConfig := newQemuConfig()
	qemuConfig.HypervisorPath = expectedPath
	qkvm := &qemuArchBase{
		machineType: "pc",
		qemuPaths: map[string]string{
			"pc": expectedPath,
		},
	}

	q := &qemu{
		config: qemuConfig,
		arch:   qkvm,
	}

	// get config hypervisor path
	path, err := q.qemuPath()
	assert.NoError(err)
	assert.Equal(path, expectedPath)

	// config hypervisor path does not exist
	q.config.HypervisorPath = "/abc/rgb/123"
	path, err = q.qemuPath()
	assert.Error(err)
	assert.Equal(path, "")

	// get arch hypervisor path
	q.config.HypervisorPath = ""
	path, err = q.qemuPath()
	assert.NoError(err)
	assert.Equal(path, expectedPath)

	// bad machine type, arch should fail
	qkvm.machineType = "rgb"
	q.arch = qkvm
	path, err = q.qemuPath()
	assert.Error(err)
	assert.Equal(path, "")
}

func TestHotplugUnsupportedDeviceType(t *testing.T) {
	assert := assert.New(t)

	qemuConfig := newQemuConfig()
	q := &qemu{
		ctx:    context.Background(),
		id:     "qemuTest",
		config: qemuConfig,
	}

	vcStore, err := store.NewVCSandboxStore(q.ctx, q.id)
	if err != nil {
		t.Fatal(err)
	}
	q.store = vcStore

	_, err = q.hotplugAddDevice(&memoryDevice{0, 128, uint64(0), false}, fsDev)
	assert.Error(err)
	_, err = q.hotplugRemoveDevice(&memoryDevice{0, 128, uint64(0), false}, fsDev)
	assert.Error(err)
}

func TestQMPSetupShutdown(t *testing.T) {
	assert := assert.New(t)

	qemuConfig := newQemuConfig()
	q := &qemu{
		config: qemuConfig,
	}

	q.qmpShutdown()

	q.qmpMonitorCh.qmp = &govmmQemu.QMP{}
	err := q.qmpSetup()
	assert.Nil(err)
}

func TestQemuCleanup(t *testing.T) {
	assert := assert.New(t)

	q := &qemu{
		ctx:    context.Background(),
		config: newQemuConfig(),
	}

	err := q.cleanup()
	assert.Nil(err)
}

func TestQemuGrpc(t *testing.T) {
	assert := assert.New(t)

	config := newQemuConfig()
	q := &qemu{
		id:     "testqemu",
		config: config,
	}

	json, err := q.toGrpc()
	assert.Nil(err)

	var q2 qemu
	err = q2.fromGrpc(context.Background(), &config, nil, json)
	assert.Nil(err)

	assert.True(q.id == q2.id)
}

func TestQemuAddDeviceToBridge(t *testing.T) {
	assert := assert.New(t)

	config := newQemuConfig()
	config.DefaultBridges = defaultBridges

	// addDeviceToBridge successfully
	config.HypervisorMachineType = QemuPC
	q := &qemu{
		config: config,
		arch:   newQemuArch(config),
	}

	q.state.Bridges = q.arch.bridges(q.config.DefaultBridges)
	// get pciBridgeMaxCapacity value from virtcontainers/types/pci.go
	const pciBridgeMaxCapacity = 30
	for i := uint32(1); i <= pciBridgeMaxCapacity; i++ {
		_, _, err := q.addDeviceToBridge(fmt.Sprintf("qemu-bridge-%d", i))
		assert.Nil(err)
	}

	// fail to add device to bridge cause no more available bridge slot
	_, _, err := q.addDeviceToBridge("qemu-bridge-31")
	exceptErr := errors.New("no more bridge slots available")
	assert.Equal(exceptErr, err)

	// addDeviceToBridge fails cause q.state.Bridges == 0
	config.HypervisorMachineType = QemuPCLite
	q = &qemu{
		config: config,
		arch:   newQemuArch(config),
	}
	q.state.Bridges = q.arch.bridges(q.config.DefaultBridges)
	_, _, err = q.addDeviceToBridge("qemu-bridge")
	exceptErr = errors.New("failed to get available address from bridges")
	assert.Equal(exceptErr, err)
}

func TestQemuFileBackedMem(t *testing.T) {
	assert := assert.New(t)

	// Check default Filebackedmem location for virtio-fs
	sandbox, err := createQemuSandboxConfig()
	if err != nil {
		t.Fatal(err)
	}
	q := &qemu{}
	sandbox.config.HypervisorConfig.SharedFS = config.VirtioFS
	if err = q.createSandbox(context.Background(), sandbox.id, &sandbox.config.HypervisorConfig, sandbox.store); err != nil {
		t.Fatal(err)
	}
	assert.Equal(q.qemuConfig.Knobs.FileBackedMem, true)
	assert.Equal(q.qemuConfig.Knobs.FileBackedMemShared, true)
	assert.Equal(q.qemuConfig.Memory.Path, fallbackFileBackedMemDir)

	// Check failure for VM templating
	sandbox, err = createQemuSandboxConfig()
	if err != nil {
		t.Fatal(err)
	}
	q = &qemu{}
	sandbox.config.HypervisorConfig.BootToBeTemplate = true
	sandbox.config.HypervisorConfig.SharedFS = config.VirtioFS
	sandbox.config.HypervisorConfig.MemoryPath = fallbackFileBackedMemDir

	err = q.createSandbox(context.Background(), sandbox.id, &sandbox.config.HypervisorConfig, sandbox.store)

	expectErr := errors.New("VM templating has been enabled with either virtio-fs or file backed memory and this configuration will not work")
	assert.Equal(expectErr, err)

	// Check Setting of non-existent shared-mem path
	sandbox, err = createQemuSandboxConfig()
	if err != nil {
		t.Fatal(err)
	}
	q = &qemu{}
	sandbox.config.HypervisorConfig.FileBackedMemRootDir = "/tmp/xyzabc"
	if err = q.createSandbox(context.Background(), sandbox.id, &sandbox.config.HypervisorConfig, sandbox.store); err != nil {
		t.Fatal(err)
	}
	assert.Equal(q.qemuConfig.Knobs.FileBackedMem, false)
	assert.Equal(q.qemuConfig.Knobs.FileBackedMemShared, false)
	assert.Equal(q.qemuConfig.Memory.Path, "")
}

func createQemuSandboxConfig() (*Sandbox, error) {

	qemuConfig := newQemuConfig()
	sandbox := Sandbox{
		ctx: context.Background(),
		id:  "testSandbox",
		config: &SandboxConfig{
			HypervisorConfig: qemuConfig,
		},
	}

	vcStore, err := store.NewVCSandboxStore(sandbox.ctx, sandbox.id)
	if err != nil {
		return &Sandbox{}, err
	}
	sandbox.store = vcStore

	return &sandbox, nil
}
