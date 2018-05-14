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
	"testing"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/stretchr/testify/assert"
)

func newQemuConfig() HypervisorConfig {
	return HypervisorConfig{
		KernelPath:        testQemuKernelPath,
		ImagePath:         testQemuImagePath,
		InitrdPath:        testQemuInitrdPath,
		HypervisorPath:    testQemuPath,
		DefaultVCPUs:      defaultVCPUs,
		DefaultMemSz:      defaultMemSzMiB,
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
	expectedOut := fmt.Sprintf("panic=1 initcall_debug nr_cpus=%d foo=foo bar=bar", MaxQemuVCPUs())
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

func TestQemuInit(t *testing.T) {
	qemuConfig := newQemuConfig()
	q := &qemu{}

	sandbox := &Sandbox{
		id:      "testSandbox",
		storage: &filesystem{},
		config: &SandboxConfig{
			HypervisorConfig: qemuConfig,
		},
	}

	// Create parent dir path for hypervisor.json
	parentDir := filepath.Join(runStoragePath, sandbox.id)
	if err := os.MkdirAll(parentDir, dirMode); err != nil {
		t.Fatalf("Could not create parent directory %s: %v", parentDir, err)
	}

	if err := q.init(sandbox); err != nil {
		t.Fatal(err)
	}

	if err := os.RemoveAll(parentDir); err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(qemuConfig, q.config) == false {
		t.Fatalf("Got %v\nExpecting %v", q.config, qemuConfig)
	}
}

func TestQemuInitMissingParentDirFail(t *testing.T) {
	qemuConfig := newQemuConfig()
	q := &qemu{}

	sandbox := &Sandbox{
		id:      "testSandbox",
		storage: &filesystem{},
		config: &SandboxConfig{
			HypervisorConfig: qemuConfig,
		},
	}

	// Ensure parent dir path for hypervisor.json does not exist.
	parentDir := filepath.Join(runStoragePath, sandbox.id)
	if err := os.RemoveAll(parentDir); err != nil {
		t.Fatal(err)
	}

	if err := q.init(sandbox); err == nil {
		t.Fatal("Qemu init() expected to fail because of missing parent directory for storage")
	}
}

func TestQemuCPUTopology(t *testing.T) {
	vcpus := 1

	q := &qemu{
		arch: &qemuArchBase{},
		config: HypervisorConfig{
			DefaultVCPUs:    uint32(vcpus),
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
	mem := 1000

	q := &qemu{
		arch: &qemuArchBase{},
	}

	hostMemKb, err := getHostMemorySizeKb(procMemInfo)
	if err != nil {
		t.Fatal(err)
	}
	memMax := fmt.Sprintf("%dM", int(float64(hostMemKb)/1024))

	expectedOut := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  defaultMemSlots,
		MaxMem: memMax,
	}

	vmConfig := Resources{
		Memory: uint(mem),
	}

	sandboxConfig := SandboxConfig{
		VMConfig: vmConfig,
	}

	memory, err := q.memoryTopology(sandboxConfig)
	if err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(memory, expectedOut) == false {
		t.Fatalf("Got %v\nExpecting %v", memory, expectedOut)
	}
}

func testQemuAddDevice(t *testing.T, devInfo interface{}, devType deviceType, expected []govmmQemu.Device) {
	q := &qemu{
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

	volume := Volume{
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

	socket := Socket{
		DeviceID: deviceID,
		ID:       id,
		HostPath: hostPath,
		Name:     name,
	}

	testQemuAddDevice(t, socket, serialPortDev, expectedOut)
}

func TestQemuGetSandboxConsole(t *testing.T) {
	q := &qemu{}
	sandboxID := "testSandboxID"
	expected := filepath.Join(runStoragePath, sandboxID, defaultConsole)

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
		arch: &qemuArchBase{},
	}

	caps := q.capabilities()
	if !caps.isBlockDeviceHotplugSupported() {
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
