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

func TestQemuPPC64leCPUModel(t *testing.T) {
	assert := assert.New(t)
	ppc64le := newTestQemu(QemuPseries)

	expectedOut := defaultCPUModel
	model := ppc64le.cpuModel()
	assert.Equal(expectedOut, model)

	ppc64le.enableNestingChecks()
	expectedOut = defaultCPUModel + ",pmu=off"
	model = ppc64le.cpuModel()
	assert.Equal(expectedOut, model)
}

func TestQemuPPC64leMemoryTopology(t *testing.T) {
	assert := assert.New(t)
	ppc64le := newTestQemu(QemuPseries)
	memoryOffset := 1024

	hostMem := uint64(1024)
	mem := uint64(120)
	slots := uint8(10)
	expectedMemory := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  slots,
		MaxMem: fmt.Sprintf("%dM", hostMem+uint64(memoryOffset)),
	}

	m := ppc64le.memoryTopology(mem, hostMem, slots)
	assert.Equal(expectedMemory, m)
}
