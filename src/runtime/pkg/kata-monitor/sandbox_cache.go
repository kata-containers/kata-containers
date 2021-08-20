// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"sync"
)

type sandboxCache struct {
	*sync.Mutex
	sandboxes map[string]struct{}
}

func (sc *sandboxCache) getAllSandboxes() map[string]struct{} {
	sc.Lock()
	defer sc.Unlock()
	return sc.sandboxes
}

func (sc *sandboxCache) deleteIfExists(id string) bool {
	sc.Lock()
	defer sc.Unlock()

	if _, found := sc.sandboxes[id]; found {
		delete(sc.sandboxes, id)
		return true
	}

	// not in sandbox cache
	return false
}

func (sc *sandboxCache) putIfNotExists(id string) bool {
	sc.Lock()
	defer sc.Unlock()

	if _, found := sc.sandboxes[id]; !found {
		sc.sandboxes[id] = struct{}{}
		return true
	}

	// already in sandbox cache
	return false
}

func (sc *sandboxCache) set(sandboxes map[string]struct{}) {
	sc.Lock()
	defer sc.Unlock()
	sc.sandboxes = sandboxes
}
