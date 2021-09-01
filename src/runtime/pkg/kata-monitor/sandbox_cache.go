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
	// the bool value tracks if the pod is a kata one (true) or not (false)
	sandboxes map[string]bool
}

func (sc *sandboxCache) getAllSandboxes() map[string]bool {
	sc.Lock()
	defer sc.Unlock()
	return sc.sandboxes
}

func (sc *sandboxCache) getKataSandboxes() []string {
	sc.Lock()
	defer sc.Unlock()
	var katasandboxes []string
	for id, isKata := range sc.sandboxes {
		if isKata {
			katasandboxes = append(katasandboxes, id)
		}
	}
	return katasandboxes
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

func (sc *sandboxCache) putIfNotExists(id string, value bool) bool {
	sc.Lock()
	defer sc.Unlock()

	if _, found := sc.sandboxes[id]; !found {
		sc.sandboxes[id] = value
		return true
	}

	// already in sandbox cache
	return false
}

func (sc *sandboxCache) set(sandboxes map[string]bool) {
	sc.Lock()
	defer sc.Unlock()
	sc.sandboxes = sandboxes
}
