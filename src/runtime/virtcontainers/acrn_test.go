// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"path/filepath"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
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
	assert := assert.New(t)
	acrnConfig := newAcrnConfig()
	acrnConfig.KernelParams = kernelParams

	if debug == true {
		acrnConfig.Debug = true
	}

	a := &Acrn{
		config: acrnConfig,
		arch:   &acrnArchBase{},
	}

	expected := fmt.Sprintf("panic=1 maxcpus=%d foo=foo bar=bar", a.config.DefaultMaxVCPUs)

	params := a.kernelParameters()
	assert.Equal(params, expected)
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
	assert := assert.New(t)
	a := &Acrn{
		ctx:  context.Background(),
		arch: &acrnArchBase{},
	}

	caps := a.capabilities(a.ctx)
	assert.True(caps.IsBlockDeviceSupported())
	assert.True(caps.IsBlockDeviceHotplugSupported())
}

func testAcrnAddDevice(t *testing.T, devInfo interface{}, devType deviceType, expected []Device) {
	assert := assert.New(t)
	a := &Acrn{
		ctx:  context.Background(),
		arch: &acrnArchBase{},
	}

	err := a.addDevice(context.Background(), devInfo, devType)
	assert.NoError(err)
	assert.Exactly(a.acrnConfig.Devices, expected)
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
	a := &Acrn{
		ctx:    context.Background(),
		id:     "acrnTest",
		config: acrnConfig,
	}

	_, err := a.hotplugAddDevice(a.ctx, &memoryDevice{0, 128, uint64(0), false}, fsDev)
	assert.Error(err)
}

func TestAcrnUpdateBlockDeviceInvalidPath(t *testing.T) {
	assert := assert.New(t)

	path := ""
	index := 1

	acrnConfig := newAcrnConfig()
	a := &Acrn{
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
	a := &Acrn{
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
	assert := assert.New(t)

	store, err := persist.GetDriver()
	assert.NoError(err)

	a := &Acrn{
		ctx:   context.Background(),
		store: store,
	}
	sandboxID := "testSandboxID"
	expected := filepath.Join(a.store.RunVMStoragePath(), sandboxID, consoleSocket)

	proto, result, err := a.getSandboxConsole(a.ctx, sandboxID)
	assert.NoError(err)
	assert.Equal(result, expected)
	assert.Equal(proto, consoleProtoUnix)
}

func TestAcrnCreateSandbox(t *testing.T) {
	assert := assert.New(t)
	acrnConfig := newAcrnConfig()
	store, err := persist.GetDriver()
	assert.NoError(err)

	a := &Acrn{
		store: store,
	}

	sandbox := &Sandbox{
		ctx: context.Background(),
		id:  "testSandbox",
		config: &SandboxConfig{
			HypervisorConfig: acrnConfig,
		},
		state: types.SandboxState{BlockIndexMap: make(map[int]struct{})},
	}

	a.sandbox = sandbox

	//set PID to 1 to ignore hypercall to get UUID and set a random UUID
	a.state.PID = 1
	a.state.UUID = "f81d4fae-7dec-11d0-a765-00a0c91e6bf6"
	err = a.createSandbox(context.Background(), sandbox.id, NetworkNamespace{}, &sandbox.config.HypervisorConfig)
	assert.NoError(err)
	assert.Exactly(acrnConfig, a.config)
}

func TestAcrnMemoryTopology(t *testing.T) {
	mem := uint32(1000)
	assert := assert.New(t)

	a := &Acrn{
		arch: &acrnArchBase{},
		config: HypervisorConfig{
			MemorySize: mem,
		},
	}

	expectedOut := Memory{
		Size: fmt.Sprintf("%dM", mem),
	}

	memory, err := a.memoryTopology()
	assert.NoError(err)
	assert.Exactly(memory, expectedOut)
}
