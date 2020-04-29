// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNoopShimStart(t *testing.T) {
	assert := assert.New(t)
	s := &noopShim{}
	sandbox := &Sandbox{}
	params := ShimParams{}
	expected := 0

	pid, err := s.start(sandbox, params)
	assert.NoError(err)
	assert.Equal(pid, expected)
}
