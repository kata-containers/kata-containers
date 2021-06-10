// Copyright (c) IBM Corp. 2021
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"math"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestCPUFacilities(t *testing.T) {
	assert := assert.New(t)

	facilities, err := CPUFacilities(procCPUInfo)
	assert.NoError(err)

	// z/Architecture facility should always be active (introduced in 2000)
	assert.Equal(facilities[1], true)
	// facility bits should not be as high as MaxInt
	assert.Equal(facilities[math.MaxInt64], false)
}
