// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestGetBDF(t *testing.T) {
	type testData struct {
		deviceStr   string
		expectedBDF string
	}

	data := []testData{
		{"0000:02:10.0", "02:10.0"},
		{"0000:0210.0", ""},
		{"test", ""},
		{"", ""},
	}

	for _, d := range data {
		deviceBDF, err := getBDF(d.deviceStr)
		assert.Equal(t, d.expectedBDF, deviceBDF)
		if d.expectedBDF == "" {
			assert.NotNil(t, err)
		} else {
			assert.Nil(t, err)
		}
	}
}
