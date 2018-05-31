// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"testing"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/stretchr/testify/assert"
)

func newTestQemu(machineType string) qemuArch {
	config := HypervisorConfig{
		HypervisorMachineType: machineType,
	}
	return newQemuArch(config)
}

func TestQemuArm64CPUModel(t *testing.T) {
	assert := assert.New(t)
	arm64 := newTestQemu(virt)

	expectedOut := defaultCPUModel
	model := arm64.cpuModel()
	assert.Equal(expectedOut, model)

	arm64.enableNestingChecks()
	expectedOut = defaultCPUModel + ",pmu=off"
	model = arm64.cpuModel()
	assert.Equal(expectedOut, model)
}

func TestQemuArm64MemoryTopology(t *testing.T) {
	assert := assert.New(t)
	arm64 := newTestQemu(virt)
	memoryOffset := 1024

	hostMem := uint64(1024)
	mem := uint64(120)
	expectedMemory := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  defaultMemSlots,
		MaxMem: fmt.Sprintf("%dM", hostMem+uint64(memoryOffset)),
	}

	m := arm64.memoryTopology(mem, hostMem)
	assert.Equal(expectedMemory, m)
}
