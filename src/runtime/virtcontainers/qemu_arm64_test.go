// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"io/ioutil"
	"os"
	"testing"

	govmmQemu "github.com/kata-containers/govmm/qemu"
	"github.com/stretchr/testify/assert"
)

func qemuConfig(machineType string) HypervisorConfig {
	return HypervisorConfig{
		HypervisorMachineType: machineType,
	}
}

func newTestQemu(assert *assert.Assertions, machineType string) qemuArch {
	config := qemuConfig(machineType)
	arch, err := newQemuArch(config)
	assert.NoError(err)
	return arch
}

func TestQemuArm64CPUModel(t *testing.T) {
	assert := assert.New(t)
	arm64 := newTestQemu(assert, QemuVirt)

	expectedOut := defaultCPUModel
	model := arm64.cpuModel()
	assert.Equal(expectedOut, model)
}

func TestQemuArm64MemoryTopology(t *testing.T) {
	assert := assert.New(t)
	arm64 := newTestQemu(assert, QemuVirt)

	hostMem := uint64(4096)
	mem := uint64(1024)
	slots := uint8(3)
	expectedMemory := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  slots,
		MaxMem: fmt.Sprintf("%dM", hostMem),
	}

	m := arm64.memoryTopology(mem, hostMem, slots)
	assert.Equal(expectedMemory, m)
}

func TestMaxQemuVCPUs(t *testing.T) {
	assert := assert.New(t)

	vCPUs := MaxQemuVCPUs()
	assert.Equal(uint32(123), vCPUs)
}

func TestQemuArm64AppendBridges(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)

	arm64 := newTestQemu(assert, QemuVirt)

	arm64.bridges(1)
	bridges := arm64.getBridges()
	assert.Len(bridges, 1)

	devices = []govmmQemu.Device{}
	devices = arm64.appendBridges(devices)
	assert.Len(devices, 1)

	expectedOut := []govmmQemu.Device{
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

func TestQemuArm64AppendImage(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)

	f, err := ioutil.TempFile("", "img")
	assert.NoError(err)
	defer func() { _ = f.Close() }()
	defer func() { _ = os.Remove(f.Name()) }()

	imageStat, err := f.Stat()
	assert.NoError(err)

	// save default supportedQemuMachines options
	machinesCopy := make([]govmmQemu.Machine, len(supportedQemuMachines))
	assert.Equal(len(supportedQemuMachines), copy(machinesCopy, supportedQemuMachines))

	cfg := qemuConfig(QemuVirt)
	cfg.ImagePath = f.Name()
	arm64 := newQemuArch(cfg)
	assert.Contains(m.machine().Options, qemuNvdimmOption)

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

	devices, err = arm64.appendImage(context.Background(), devices, f.Name())
	assert.NoError(err)
	assert.Equal(expectedOut, devices)

	// restore default supportedQemuMachines options
	assert.Equal(len(supportedQemuMachines), copy(supportedQemuMachines, machinesCopy))
}

func TestQemuArm64WithInitrd(t *testing.T) {
	assert := assert.New(t)

	cfg := qemuConfig(QemuVirt)
	cfg.InitrdPath = "dummy-initrd"
	arm64 := newQemuArch(cfg)

	assert.NotContains(m.machine().Options, qemuNvdimmOption)
}
