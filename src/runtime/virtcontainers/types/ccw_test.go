// Copyright 2025 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"github.com/stretchr/testify/assert"
	"testing"
)

func TestPciDevice(t *testing.T) {
	assert := assert.New(t)

	// Valid devices
	dev, err := CcwDeviceFrom(0, "0000")
	assert.NoError(err)
	assert.Equal(dev, CcwDevice{0, 0})
	assert.Equal(dev.String(), "0.0.0000")

	dev, err = CcwDeviceFrom(3, "ffff")
	assert.NoError(err)
	assert.Equal(dev, CcwDevice{3, 65535})
	assert.Equal(dev.String(), "0.3.ffff")

	// Invalid devices
	_, err = CcwDeviceFrom(4, "0000")
	assert.Error(err)

	_, err = CcwDeviceFrom(-1, "0000")
	assert.Error(err)

	_, err = CcwDeviceFrom(-1, "10000")
	assert.Error(err)

	_, err = CcwDeviceFrom(-1, "NaN")
	assert.Error(err)
}
