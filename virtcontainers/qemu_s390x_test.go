// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"testing"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/stretchr/testify/assert"
)

func newTestQemu(machineType string) qemuArch {
	config := HypervisorConfig{
		HypervisorMachineType: machineType,
	}
	return newQemuArch(config)
}

func TestQemuS390xCPUModel(t *testing.T) {
	assert := assert.New(t)
	s390x := newTestQemu(QemuCCWVirtio)

	expectedOut := defaultCPUModel
	model := s390x.cpuModel()
	assert.Equal(expectedOut, model)

	s390x.enableNestingChecks()
	expectedOut = defaultCPUModel + ",pmu=off"
	model = s390x.cpuModel()
	assert.Equal(expectedOut, model)
}

func TestQemuS390xMemoryTopology(t *testing.T) {
	assert := assert.New(t)
	s390x := newTestQemu(QemuCCWVirtio)
	memoryOffset := 1024

	hostMem := uint64(1024)
	mem := uint64(120)
	slots := uint8(10)
	expectedMemory := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  slots,
		MaxMem: fmt.Sprintf("%dM", hostMem+uint64(memoryOffset)),
	}

	m := s390x.memoryTopology(mem, hostMem, slots)
	assert.Equal(expectedMemory, m)
}

func TestQemuS390xAppendVhostUserDevice(t *testing.T) {
	macAddress := "00:11:22:33:44:55:66"
	qemu := qemuS390x{}
	assert := assert.New(t)

	vhostUserDevice := config.VhostUserDeviceAttrs{
		Type:       config.VhostUserNet,
		MacAddress: macAddress,
	}

	_, err := qemu.appendVhostUserDevice(nil, vhostUserDevice)
	assert.Error(err)
}
