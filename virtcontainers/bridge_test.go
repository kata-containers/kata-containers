//
// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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
