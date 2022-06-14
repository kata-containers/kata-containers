// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/stretchr/testify/assert"
)

func TestBumpAttachCount(t *testing.T) {
	type testData struct {
		attachCount uint
		expectedAC  uint
		attach      bool
		expectSkip  bool
		expectErr   bool
	}

	data := []testData{
		{0, 1, true, false, false},
		{1, 2, true, true, false},
		{intMax, intMax, true, true, true},
		{0, 0, false, true, true},
		{1, 0, false, false, false},
		{intMax, intMax - 1, false, true, false},
	}

	dev := &GenericDevice{}
	for _, d := range data {
		dev.AttachCount = d.attachCount
		skip, err := dev.bumpAttachCount(d.attach)
		assert.Equal(t, skip, d.expectSkip, "")
		assert.Equal(t, dev.GetAttachCount(), d.expectedAC, "")
		if d.expectErr {
			assert.NotNil(t, err)
		} else {
			assert.Nil(t, err)
		}
	}
}

func TestGetHostPath(t *testing.T) {
	assert := assert.New(t)
	dev := &GenericDevice{}
	assert.Empty(dev.GetHostPath())

	expectedHostPath := "/dev/null"
	dev.DeviceInfo = &config.DeviceInfo{
		HostPath: expectedHostPath,
	}
	assert.Equal(expectedHostPath, dev.GetHostPath())
}
