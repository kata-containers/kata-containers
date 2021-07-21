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
		sandboxes: make(map[string]struct{}),
	}

	scMap := map[string]struct{}{"111": {}}

	sc.set(scMap)

	scMap = sc.getAllSandboxes()
	assert.Equal(1, len(scMap))

	// put new item
	id := "new-id"
	b := sc.putIfNotExists(id)
	assert.Equal(true, b)
	assert.Equal(2, len(scMap))

	// put key that alreay exists
	b = sc.putIfNotExists(id)
	assert.Equal(false, b)

	b = sc.deleteIfExists(id)
	assert.Equal(true, b)
	assert.Equal(1, len(scMap))

	b = sc.deleteIfExists(id)
	assert.Equal(false, b)
	assert.Equal(1, len(scMap))
}
