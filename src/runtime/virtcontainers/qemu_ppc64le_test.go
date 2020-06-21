// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"os/exec"
	"regexp"
	"strconv"
	"testing"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/stretchr/testify/assert"
)

var qemuVersionArgs = "--version"

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
}

func getQemuVersion() (qemuMajorVersion int, qemuMinorVersion int) {

	cmd := exec.Command(defaultQemuPath, qemuVersionArgs)
	additionalEnv := "LANG=C"
	cmd.Env = append(cmd.Env, additionalEnv)
	out, err := cmd.Output()
	if err != nil {
		err = fmt.Errorf("Could not execute command %s %s", defaultQemuPath, qemuVersionArgs)
		fmt.Println(err.Error())
	}

	re := regexp.MustCompile("[0-9]+")
	qVer := re.FindAllString(string(out), -1)

	qMajor, err := strconv.Atoi(qVer[0])
	qMinor, err1 := strconv.Atoi(qVer[1])

	if err != nil || err1 != nil {
		err = fmt.Errorf("Could not convert string to int")
		fmt.Println(err.Error())
	}

	return qMajor, qMinor
}

func TestQemuPPC64leMemoryTopology(t *testing.T) {
	assert := assert.New(t)
	ppc64le := newTestQemu(QemuPseries)
	memoryOffset := 1024

	hostMem := uint64(1024)
	mem := uint64(120)
	slots := uint8(10)

	qemuMajorVersion, qemuMinorVersion = getQemuVersion()
	m := ppc64le.memoryTopology(mem, hostMem, slots)

	if qemuMajorVersion <= 2 && qemuMinorVersion < 10 {
		hostMem = uint64(defaultMemMaxPPC64le)
	}

	expectedMemory := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  slots,
		MaxMem: fmt.Sprintf("%dM", hostMem+uint64(memoryOffset)),
	}

	assert.Equal(expectedMemory, m)
}
