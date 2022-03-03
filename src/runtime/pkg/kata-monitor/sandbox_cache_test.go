// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"sync"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestSandboxCache(t *testing.T) {
	assert := assert.New(t)
	sc := &sandboxCache{
		Mutex:     &sync.Mutex{},
		sandboxes: map[string]sandboxCRIMetadata{"111": {"1-2-3", "test-name", "test-namespace"}},
	}

	assert.Equal(1, len(sc.getSandboxList()))

	// put new item
	id := "new-id"
	b := sc.putIfNotExists(id, sandboxCRIMetadata{})
	assert.Equal(true, b)
	assert.Equal(2, len(sc.getSandboxList()))

	// put key that alreay exists
	b = sc.putIfNotExists(id, sandboxCRIMetadata{})
	assert.Equal(false, b)

	b = sc.deleteIfExists(id)
	assert.Equal(true, b)
	assert.Equal(1, len(sc.getSandboxList()))

	b = sc.deleteIfExists(id)
	assert.Equal(false, b)
	assert.Equal(1, len(sc.getSandboxList()))
}
