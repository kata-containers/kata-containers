// Copyright (c) 2019 ARM Limited
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
	"syscall"
)

const (
	KVM_CHECK_EXTENSION = 0xae03
	KVM_CAP_ARM_RME_ID  = 240
)

func TestRunningOnVMM(t *testing.T) {
	assert := assert.New(t)
	expectedOutput := false

	f, err := os.CreateTemp("", "cpuinfo")
	assert.NoError(err)
	defer os.Remove(f.Name())
	defer f.Close()

	running, err := RunningOnVMM(f.Name())
	assert.NoError(err)
	assert.Equal(expectedOutput, running)
}

func mockSyscall(trap, a1, a2, a3 uintptr) (uintptr, uintptr, syscall.Errno) {
	if uintptr(a2) == KVM_CHECK_EXTENSION && uintptr(a3) == KVM_CAP_ARM_RME_ID {
		return 1, 0, 0
	}
	return 0, 0, syscall.EINVAL
}

func mockOpen(path string, flags int, perm uint32) (int, error) {
	if path == kvmDevice {
		return 3, nil
	}
	return 0, syscall.ENOENT
}

func mockClose(fd int) error {
	return nil
}

func TestCheckKVMExtensionsRMESupported(t *testing.T) {
	assert := assert.New(t)
	oldSyscall := syscallSyscall
	oldOpen := syscallOpen
	oldClose := syscallClose

	syscallOpen = mockOpen
	syscallClose = mockClose
	syscallSyscall = mockSyscall

	defer func() {
		syscallSyscall = oldSyscall
		syscallOpen = oldOpen
		syscallClose = oldClose
	}()

	supported, err := checkKVMExtensionsRME()
	assert.NoError(err)
	assert.True(supported)
}
