// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/store"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

func newAcrnConfig() HypervisorConfig {
	return HypervisorConfig{
		KernelPath:        testAcrnKernelPath,
		ImagePath:         testAcrnImagePath,
		HypervisorPath:    testAcrnPath,
		HypervisorCtlPath: testAcrnCtlPath,
		NumVCPUs:          defaultVCPUs,
		MemorySize:        defaultMemSzMiB,
		BlockDeviceDriver: config.VirtioBlock,
		DefaultBridges:    defaultBridges,
		DefaultMaxVCPUs:   MaxAcrnVCPUs(),
		// Adding this here, as hypervisorconfig.valid()
		// forcefully adds it even when 9pfs is not supported
		Msize9p: defaultMsize9p,
	}
}

func testAcrnKernelParameters(t *testing.T, kernelParams []Param, debug bool) {
	acrnConfig := newAcrnConfig()
	acrnConfig.KernelParams = kernelParams

	if debug == true {
		acrnConfig.Debug = true
	}

	a := &acrn{
		config: acrnConfig,
		arch:   &acrnArchBase{},
	}

	expected := fmt.Sprintf("panic=1 maxcpus=%d foo=foo bar=bar", a.config.NumVCPUs)

	params := a.kernelParameters()
	if params != expected {
		t.Fatalf("Got: %v, Expecting: %v", params, expected)
	}
}

func TestAcrnKernelParameters(t *testing.T) {
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

	testAcrnKernelParameters(t, params, true)
	testAcrnKernelParameters(t, params, false)
}

func TestAcrnCapabilities(t *testing.T) {
	a := &acrn{
		ctx:  context.Background(),
		arch: &acrnArchBase{},
	}

	caps := a.capabilities()
	if !caps.IsBlockDeviceSupported() {
		t.Fatal("Block device should be supported")
	}

	if !caps.IsBlockDeviceHotplugSupported() {
		t.Fatal("Block device hotplug should be supported")
	}
}

func testAcrnAddDevice(t *testing.T, devInfo interface{}, devType deviceType, expected []Device) {
	a := &acrn{
		ctx:  context.Background(),
		arch: &acrnArchBase{},
	}

	err := a.addDevice(devInfo, devType)
	if err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(a.acrnConfig.Devices, expected) == false {
		t.Fatalf("Got %v\nExpecting %v", a.acrnConfig.Devices, expected)
	}
}

func TestAcrnAddDeviceSerialPortDev(t *testing.T) {
	name := "serial.test"
	hostPath := "/tmp/serial.sock"

	expectedOut := []Device{
		ConsoleDevice{
			Name:     name,
			Backend:  Socket,
			PortType: SerialBE,
			Path:     hostPath,
		},
	}

	socket := types.Socket{
		HostPath: hostPath,
		Name:     name,
	}

	testAcrnAddDevice(t, socket, serialPortDev, expectedOut)
}

func TestAcrnAddDeviceBlockDev(t *testing.T) {
	path := "/tmp/test.img"
	index := 1

	expectedOut := []Device{
		BlockDevice{
			FilePath: path,
			Index:    index,
		},
	}

	drive := config.BlockDrive{
		File:  path,
		Index: index,
	}

	testAcrnAddDevice(t, drive, blockDev, expectedOut)
}

func TestAcrnHotplugUnsupportedDeviceType(t *testing.T) {
	assert := assert.New(t)

	acrnConfig := newAcrnConfig()
	a := &acrn{
		ctx:    context.Background(),
		id:     "acrnTest",
		config: acrnConfig,
	}

	_, err := a.hotplugAddDevice(&memoryDevice{0, 128, uint64(0), false}, fsDev)
	assert.Error(err)
}

func TestAcrnUpdateBlockDeviceInvalidPath(t *testing.T) {
	assert := assert.New(t)

	path := ""
	index := 1

	acrnConfig := newAcrnConfig()
	a := &acrn{
		ctx:    context.Background(),
		id:     "acrnBlkTest",
		config: acrnConfig,
	}

	drive := &config.BlockDrive{
		File:  path,
		Index: index,
	}

	err := a.updateBlockDevice(drive)
	assert.Error(err)
}

func TestAcrnUpdateBlockDeviceInvalidIdx(t *testing.T) {
	assert := assert.New(t)

	path := "/tmp/test.img"
	index := AcrnBlkDevPoolSz + 1

	acrnConfig := newAcrnConfig()
	a := &acrn{
		ctx:    context.Background(),
		id:     "acrnBlkTest",
		config: acrnConfig,
	}

	drive := &config.BlockDrive{
		File:  path,
		Index: index,
	}

	err := a.updateBlockDevice(drive)
	assert.Error(err)
}

func TestAcrnGetSandboxConsole(t *testing.T) {
	a := &acrn{
		ctx: context.Background(),
	}
	sandboxID := "testSandboxID"
	expected := filepath.Join(store.RunVMStoragePath, sandboxID, consoleSocket)

	result, err := a.getSandboxConsole(sandboxID)
	if err != nil {
		t.Fatal(err)
	}

	if result != expected {
		t.Fatalf("Got %s\nExpecting %s", result, expected)
	}
}

func TestAcrnCreateSandbox(t *testing.T) {
	acrnConfig := newAcrnConfig()
	a := &acrn{}

	sandbox := &Sandbox{
		ctx: context.Background(),
		id:  "testSandbox",
		config: &SandboxConfig{
			HypervisorConfig: acrnConfig,
		},
	}

	vcStore, err := store.NewVCSandboxStore(sandbox.ctx, sandbox.id)
	if err != nil {
		t.Fatal(err)
	}
	sandbox.store = vcStore

	if err = globalSandboxList.addSandbox(sandbox); err != nil {
		t.Fatalf("Could not add sandbox to global list: %v", err)
	}

	defer globalSandboxList.removeSandbox(sandbox.id)

	// Create the hypervisor fake binary
	testAcrnPath := filepath.Join(testDir, testHypervisor)
	_, err = os.Create(testAcrnPath)
	if err != nil {
		t.Fatalf("Could not create hypervisor file %s: %v", testAcrnPath, err)
	}

	if err := a.createSandbox(context.Background(), sandbox.id, &sandbox.config.HypervisorConfig, sandbox.store); err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(acrnConfig, a.config) == false {
		t.Fatalf("Got %v\nExpecting %v", a.config, acrnConfig)
	}
}

func TestAcrnMemoryTopology(t *testing.T) {
	mem := uint32(1000)

	a := &acrn{
		arch: &acrnArchBase{},
		config: HypervisorConfig{
			MemorySize: mem,
		},
	}

	expectedOut := Memory{
		Size: fmt.Sprintf("%dM", mem),
	}

	memory, err := a.memoryTopology()
	if err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(memory, expectedOut) == false {
		t.Fatalf("Got %v\nExpecting %v", memory, expectedOut)
	}
}
