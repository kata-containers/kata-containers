// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"io/ioutil"
	"os"
	"testing"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

func newTestQemu(machineType string) qemuArch {
	config := HypervisorConfig{
		HypervisorMachineType: machineType,
	}
	return newQemuArch(config)
}

func TestQemuAmd64Capabilities(t *testing.T) {
	assert := assert.New(t)

	amd64 := newTestQemu(QemuPC)
	caps := amd64.capabilities()
	assert.True(caps.IsBlockDeviceHotplugSupported())

	amd64 = newTestQemu(QemuQ35)
	caps = amd64.capabilities()
	assert.True(caps.IsBlockDeviceHotplugSupported())
}

func TestQemuAmd64Bridges(t *testing.T) {
	assert := assert.New(t)
	amd64 := newTestQemu(QemuPC)
	len := 5

	bridges := amd64.bridges(uint32(len))
	assert.Len(bridges, len)

	for i, b := range bridges {
		id := fmt.Sprintf("%s-bridge-%d", types.PCI, i)
		assert.Equal(types.PCI, b.Type)
		assert.Equal(id, b.ID)
		assert.NotNil(b.Address)
	}

	amd64 = newTestQemu(QemuQ35)
	bridges = amd64.bridges(uint32(len))
	assert.Len(bridges, len)

	for i, b := range bridges {
		id := fmt.Sprintf("%s-bridge-%d", types.PCI, i)
		assert.Equal(types.PCI, b.Type)
		assert.Equal(id, b.ID)
		assert.NotNil(b.Address)
	}

	amd64 = newTestQemu(QemuQ35 + QemuPC)
	bridges = amd64.bridges(uint32(len))
	assert.Nil(bridges)
}

func TestQemuAmd64CPUModel(t *testing.T) {
	assert := assert.New(t)
	amd64 := newTestQemu(QemuPC)

	expectedOut := defaultCPUModel
	model := amd64.cpuModel()
	assert.Equal(expectedOut, model)

	amd64.enableNestingChecks()
	expectedOut = defaultCPUModel + ",pmu=off"
	model = amd64.cpuModel()
	assert.Equal(expectedOut, model)
}

func TestQemuAmd64MemoryTopology(t *testing.T) {
	assert := assert.New(t)
	amd64 := newTestQemu(QemuPC)
	memoryOffset := 1024

	hostMem := uint64(100)
	mem := uint64(120)
	slots := uint8(10)
	expectedMemory := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  slots,
		MaxMem: fmt.Sprintf("%dM", hostMem+uint64(memoryOffset)),
	}

	m := amd64.memoryTopology(mem, hostMem, slots)
	assert.Equal(expectedMemory, m)
}

func TestQemuAmd64AppendImage(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)
	amd64 := newTestQemu(QemuPC)

	f, err := ioutil.TempFile("", "img")
	assert.NoError(err)
	defer func() { _ = f.Close() }()
	defer func() { _ = os.Remove(f.Name()) }()

	imageStat, err := f.Stat()
	assert.NoError(err)

	expectedOut := []govmmQemu.Device{
		govmmQemu.Object{
			Driver:   govmmQemu.NVDIMM,
			Type:     govmmQemu.MemoryBackendFile,
			DeviceID: "nv0",
			ID:       "mem0",
			MemPath:  f.Name(),
			Size:     (uint64)(imageStat.Size()),
		},
	}

	devices, err = amd64.appendImage(devices, f.Name())
	assert.NoError(err)

	assert.Equal(expectedOut, devices)
}

func TestQemuAmd64AppendBridges(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)

	// check PC
	amd64 := newTestQemu(QemuPC)

	bridges := amd64.bridges(1)
	assert.Len(bridges, 1)

	devices = amd64.appendBridges(devices, bridges)
	assert.Len(devices, 1)

	expectedOut := []govmmQemu.Device{
		govmmQemu.BridgeDevice{
			Type:    govmmQemu.PCIBridge,
			Bus:     defaultPCBridgeBus,
			ID:      bridges[0].ID,
			Chassis: 1,
			SHPC:    true,
			Addr:    "2",
		},
	}

	assert.Equal(expectedOut, devices)

	// Check Q35
	amd64 = newTestQemu(QemuQ35)

	bridges = amd64.bridges(1)
	assert.Len(bridges, 1)

	devices = []govmmQemu.Device{}
	devices = amd64.appendBridges(devices, bridges)
	assert.Len(devices, 1)

	expectedOut = []govmmQemu.Device{
		govmmQemu.BridgeDevice{
			Type:    govmmQemu.PCIBridge,
			Bus:     defaultBridgeBus,
			ID:      bridges[0].ID,
			Chassis: 1,
			SHPC:    true,
			Addr:    "2",
		},
	}

	assert.Equal(expectedOut, devices)
}
