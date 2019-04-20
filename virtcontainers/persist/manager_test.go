// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persist

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestGetDriver(t *testing.T) {
	nonexist, err := GetDriver("non-exist")
	assert.NotNil(t, err)
	assert.Nil(t, nonexist)

	fsDriver, err := GetDriver("fs")
	assert.Nil(t, err)
	assert.NotNil(t, fsDriver)
}
