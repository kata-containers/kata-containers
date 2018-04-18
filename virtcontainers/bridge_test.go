// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestAddRemoveDevice(t *testing.T) {
	assert := assert.New(t)

	// create a bridge
	bridges := []*Bridge{{make(map[uint32]string), pciBridge, "rgb123"}}

	// add device
	devID := "abc123"
	b := bridges[0]
	addr, err := b.addDevice(devID)
	assert.NoError(err)
	if addr < 1 {
		assert.Fail("address cannot be less than 1")
	}

	// remove device
	err = b.removeDevice("")
	assert.Error(err)

	err = b.removeDevice(devID)
	assert.NoError(err)

	// add device when the bridge is full
	bridges[0].Address = make(map[uint32]string)
	for i := uint32(1); i <= pciBridgeMaxCapacity; i++ {
		bridges[0].Address[i] = fmt.Sprintf("%d", i)
	}
	addr, err = b.addDevice(devID)
	assert.Error(err)
	if addr != 0 {
		assert.Fail("address should be 0")
	}
}
