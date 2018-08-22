// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestBumpAttachCount(t *testing.T) {
	type testData struct {
		attach      bool
		attachCount uint
		expectedAC  uint
		expectSkip  bool
		expectErr   bool
	}

	data := []testData{
		{true, 0, 0, false, false},
		{true, 1, 2, true, false},
		{true, intMax, intMax, true, true},
		{false, 0, 0, true, true},
		{false, 1, 1, false, false},
		{false, intMax, intMax - 1, true, false},
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
