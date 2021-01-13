// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"testing"

	govmmQemu "github.com/kata-containers/govmm/qemu"
	"github.com/stretchr/testify/assert"
)

func newTestQemu(assert *assert.Assertions, machineType string) qemuArch {
	config := HypervisorConfig{
		HypervisorMachineType: machineType,
	}
	arch, err := newQemuArch(config)
	assert.NoError(err)
	return arch
}

func TestQemuPPC64leCPUModel(t *testing.T) {
	assert := assert.New(t)
	ppc64le := newTestQemu(assert, QemuPseries)

	expectedOut := defaultCPUModel
	model := ppc64le.cpuModel()
	assert.Equal(expectedOut, model)
}

func TestQemuPPC64leMemoryTopology(t *testing.T) {
	assert := assert.New(t)
	ppc64le := newTestQemu(assert, QemuPseries)
	memoryOffset := 1024

	hostMem := uint64(1024)
	mem := uint64(120)
	slots := uint8(10)

	m := ppc64le.memoryTopology(mem, hostMem, slots)

	expectedMemory := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  slots,
		MaxMem: fmt.Sprintf("%dM", hostMem+uint64(memoryOffset)),
	}

	assert.Equal(expectedMemory, m)
}
