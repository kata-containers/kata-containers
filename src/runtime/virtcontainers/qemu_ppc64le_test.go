//go:build linux

// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"testing"

	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
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

func TestQemuPPC64leAppendProtectionDevice(t *testing.T) {
	assert := assert.New(t)
	ppc64le := newTestQemu(assert, QemuPseries)

	var devices []govmmQemu.Device
	var bios, firmware string
	var err error
	devices, bios, err = ppc64le.appendProtectionDevice(devices, firmware, "")
	assert.NoError(err)

	//no protection
	assert.Empty(bios)

	//Secure Execution protection
	ppc64le.(*qemuPPC64le).protection = seProtection
	devices, bios, err = ppc64le.appendProtectionDevice(devices, firmware, "")
	assert.Error(err)
	assert.Empty(bios)

	//SEV protection
	ppc64le.(*qemuPPC64le).protection = sevProtection
	devices, bios, err = ppc64le.appendProtectionDevice(devices, firmware, "")
	assert.Error(err)
	assert.Empty(bios)

	//SNP protection
	ppc64le.(*qemuPPC64le).protection = snpProtection
	devices, bios, err = ppc64le.appendProtectionDevice(devices, firmware, "")
	assert.Error(err)
	assert.Empty(bios)

	//TDX protection
	ppc64le.(*qemuPPC64le).protection = tdxProtection
	devices, bios, err = ppc64le.appendProtectionDevice(devices, firmware, "")
	assert.Error(err)
	assert.Empty(bios)

	//PEF protection
	ppc64le.(*qemuPPC64le).protection = pefProtection
	devices, bios, err = ppc64le.appendProtectionDevice(devices, firmware, "")
	assert.NoError(err)
	assert.Empty(bios)

	expectedOut := []govmmQemu.Device{
		govmmQemu.Object{
			Driver:   govmmQemu.SpaprTPMProxy,
			Type:     govmmQemu.PEFGuest,
			ID:       pefID,
			DeviceID: tpmID,
			File:     tpmHostPath,
		},
	}
	assert.Equal(expectedOut, devices)

}
