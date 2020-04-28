// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package experimental

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestExperimental(t *testing.T) {
	f := Feature{
		Name:        "mock",
		Description: "mock experimental feature for test",
		ExpRelease:  "2.0",
	}
	assert.Nil(t, Get(f.Name))

	err := Register(f)
	assert.Nil(t, err)

	err = Register(f)
	assert.NotNil(t, err)
	assert.Equal(t, len(supportedFeatures), 1)

	assert.NotNil(t, Get(f.Name))
}

func TestValidateFeature(t *testing.T) {
	f := Feature{}
	assert.NotNil(t, validateFeature(f))

	for _, names := range []struct {
		name  string
		valid bool
	}{
		{"mock_test_1", true},
		{"m1234ock_test_1", true},
		{"1_mock_test", false},
		{"_mock_test_1", false},
		{"Mock", false},
		{"mock*&", false},
	} {
		f := Feature{
			Name:        names.name,
			Description: "test",
			ExpRelease:  "2.0",
		}

		err := validateFeature(f)
		if names.valid {
			assert.Nil(t, err)
		} else {
			assert.NotNil(t, err)
		}
	}
}
