// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"runtime"
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
	arm64 := newTestQemu(QemuVirt)

	expectedOut := defaultCPUModel
	model := arm64.cpuModel()
	assert.Equal(expectedOut, model)
}

func TestQemuArm64MemoryTopology(t *testing.T) {
	assert := assert.New(t)
	arm64 := newTestQemu(QemuVirt)

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

	type testData struct {
		contents       string
		expectedResult uint32
	}

	data := []testData{
		{"", uint32(runtime.NumCPU())},
		{"  1:          0          0     GICv2  25 Level     vgic \n", uint32(8)},
		{"  1:          0          0     GICv3  25 Level     vgic \n", uint32(123)},
		{"  1:          0          0     GICv4  25 Level     vgic \n", uint32(123)},
	}

	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	savedGicProfile := gicProfile

	testGicProfile := filepath.Join(tmpdir, "interrupts")

	// override
	gicProfile = testGicProfile

	defer func() {
		gicProfile = savedGicProfile
	}()

	savedHostGICVersion := hostGICVersion

	defer func() {
		hostGICVersion = savedHostGICVersion
	}()

	for _, d := range data {
		err := ioutil.WriteFile(gicProfile, []byte(d.contents), os.FileMode(0640))
		assert.NoError(err)

		hostGICVersion = getHostGICVersion()
		vCPUs := MaxQemuVCPUs()

		assert.Equal(d.expectedResult, vCPUs)
	}
}
