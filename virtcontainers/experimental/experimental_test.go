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
	f := Feature("mock")
	assert.False(t, Supported(f))

	err := Register("mock")
	assert.Nil(t, err)

	err = Register("mock")
	assert.NotNil(t, err)
	assert.Equal(t, len(supportedFeatures), 1)

	assert.True(t, Supported(f))
}
