// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"fmt"
	"sync"
)

type sandboxCache struct {
	*sync.Mutex
	sandboxes map[string]string
}

func (sc *sandboxCache) getAllSandboxes() map[string]string {
	sc.Lock()
	defer sc.Unlock()
	return sc.sandboxes
}

func (sc *sandboxCache) deleteIfExists(id string) (string, bool) {
	sc.Lock()
	defer sc.Unlock()

	if val, found := sc.sandboxes[id]; found {
		delete(sc.sandboxes, id)
		return val, true
	}

	// not in sandbox cache
	return "", false
}

func (sc *sandboxCache) putIfNotExists(id, value string) bool {
	sc.Lock()
	defer sc.Unlock()

	if _, found := sc.sandboxes[id]; !found {
		sc.sandboxes[id] = value
		return true
	}

	// already in sandbox cache
	return false
}

func (sc *sandboxCache) set(sandboxes map[string]string) {
	sc.Lock()
	defer sc.Unlock()
	sc.sandboxes = sandboxes
}

func (sc *sandboxCache) getSandboxRuntime(sandbox string) (string, error) {
	sc.Lock()
	defer sc.Unlock()

	if val, found := sc.sandboxes[sandbox]; found {
		return val, nil
	}

	return "", fmt.Errorf("sandbox %s not in cache", sandbox)
}
