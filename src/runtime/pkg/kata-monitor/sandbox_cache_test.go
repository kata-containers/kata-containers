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
		sandboxes: make(map[string]string),
	}

	scMap := map[string]string{"111": "222"}

	sc.init(scMap)

	scMap = sc.getAllSandboxes()
	assert.Equal(1, len(scMap))

	// put new item
	id := "new-id"
	value := "new-value"
	b := sc.putIfNotExists(id, "new-value")
	assert.Equal(true, b)
	assert.Equal(2, len(scMap))

	// put key that alreay exists
	b = sc.putIfNotExists(id, "new-value")
	assert.Equal(false, b)

	v, b := sc.deleteIfExists(id)
	assert.Equal(value, v)
	assert.Equal(true, b)
	assert.Equal(1, len(scMap))

	v, b = sc.deleteIfExists(id)
	assert.Equal("", v)
	assert.Equal(false, b)
	assert.Equal(1, len(scMap))
}
