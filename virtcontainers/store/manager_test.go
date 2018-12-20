// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package store

import (
	"context"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNewStore(t *testing.T) {
	s, err := New(context.Background(), "file:///root/")
	assert.Nil(t, err)
	assert.Equal(t, s.scheme, "file")
	assert.Equal(t, s.host, "")
	assert.Equal(t, s.path, "/root/")
}
