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
		sandboxes: make(map[string]sandboxKubeData),
	}

	scMap := map[string]sandboxKubeData{"111": {"1-2-3", "test-name", "test-namespace"}}

	sc.set(scMap)

	scMap = sc.getSandboxes()
	assert.Equal(1, len(scMap))

	// put new item
	id := "new-id"
	b := sc.putIfNotExists(id, sandboxKubeData{})
	assert.Equal(true, b)
	assert.Equal(2, len(scMap))

	// put key that alreay exists
	b = sc.putIfNotExists(id, sandboxKubeData{})
	assert.Equal(false, b)

	b = sc.deleteIfExists(id)
	assert.Equal(true, b)
	assert.Equal(1, len(scMap))

	b = sc.deleteIfExists(id)
	assert.Equal(false, b)
	assert.Equal(1, len(scMap))
}
