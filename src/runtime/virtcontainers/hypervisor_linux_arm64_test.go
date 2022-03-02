// Copyright (c) 2019 ARM Limited
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
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

func TestAvailableGuestProtection(t *testing.T) {
	assert := assert.New(t)

	out, _ := availableGuestProtection()
	assert.Equal(out, noneProtection)
}
