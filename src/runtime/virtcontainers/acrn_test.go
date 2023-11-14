//go:build linux

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

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
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
		NumVCPUsF:         defaultVCPUs,
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

	caps := a.Capabilities(a.ctx)
	assert.True(caps.IsBlockDeviceSupported())
	assert.True(caps.IsBlockDeviceHotplugSupported())
	assert.True(caps.IsNetworkDeviceHotplugSupported())
}

func testAcrnAddDevice(t *testing.T, devInfo interface{}, devType DeviceType, expected []Device) {
	assert := assert.New(t)
	a := &Acrn{
		ctx:  context.Background(),
		arch: &acrnArchBase{},
	}

	err := a.AddDevice(context.Background(), devInfo, devType)
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

	testAcrnAddDevice(t, socket, SerialPortDev, expectedOut)
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

	testAcrnAddDevice(t, drive, BlockDev, expectedOut)
}

func TestAcrnHotplugUnsupportedDeviceType(t *testing.T) {
	assert := assert.New(t)

	acrnConfig := newAcrnConfig()
	a := &Acrn{
		ctx:    context.Background(),
		id:     "acrnTest",
		config: acrnConfig,
	}

	_, err := a.HotplugAddDevice(a.ctx, &MemoryDevice{0, 128, uint64(0), false}, FsDev)
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
		ctx: context.Background(),
		config: HypervisorConfig{
			VMStorePath:  store.RunVMStoragePath(),
			RunStorePath: store.RunStoragePath(),
		},
		store: store,
	}
	sandboxID := "testSandboxID"
	expected := filepath.Join(store.RunVMStoragePath(), sandboxID, consoleSocket)

	proto, result, err := a.GetVMConsole(a.ctx, sandboxID)
	assert.NoError(err)
	assert.Equal(result, expected)
	assert.Equal(proto, consoleProtoUnix)
}

func TestAcrnCreateVM(t *testing.T) {
	assert := assert.New(t)
	acrnConfig := newAcrnConfig()
	store, err := persist.GetDriver()
	assert.NoError(err)

	a := &Acrn{
		store: store,
		config: HypervisorConfig{
			VMStorePath:  store.RunVMStoragePath(),
			RunStorePath: store.RunStoragePath(),
		},
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

	a.state.PID = 1
	network, err := NewNetwork()
	assert.NoError(err)
	err = a.CreateVM(context.Background(), sandbox.id, network, &sandbox.config.HypervisorConfig)
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

func TestAcrnSetConfig(t *testing.T) {
	assert := assert.New(t)

	config := newAcrnConfig()

	a := &Acrn{}

	assert.Equal(a.config, HypervisorConfig{})

	err := a.setConfig(&config)
	assert.NoError(err)

	assert.Equal(a.config, config)
}
