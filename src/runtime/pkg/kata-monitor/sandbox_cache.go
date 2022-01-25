// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"sync"
)

type sandboxKubeData struct {
	uid       string
	name      string
	namespace string
}
type sandboxCache struct {
	*sync.Mutex
	// the sandboxKubeData links the sandbox id from the container manager to the pod metadata of kubernetes
	sandboxes map[string]sandboxKubeData
}

func (sc *sandboxCache) getSandboxes() map[string]sandboxKubeData {
	sc.Lock()
	defer sc.Unlock()
	return sc.sandboxes
}

func (sc *sandboxCache) getSandboxList() []string {
	sc.Lock()
	defer sc.Unlock()
	var sandboxList []string
	for id := range sc.sandboxes {
		sandboxList = append(sandboxList, id)
	}
	return sandboxList
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

func (sc *sandboxCache) putIfNotExists(id string, value sandboxKubeData) bool {
	sc.Lock()
	defer sc.Unlock()

	if _, found := sc.sandboxes[id]; !found {
		sc.sandboxes[id] = value
		return true
	}

	// already in sandbox cache
	return false
}

func (sc *sandboxCache) setMetadata(id string, value sandboxKubeData) {
	sc.Lock()
	defer sc.Unlock()

	sc.sandboxes[id] = value
}

func (sc *sandboxCache) set(sandboxes map[string]sandboxKubeData) {
	sc.Lock()
	defer sc.Unlock()
	sc.sandboxes = sandboxes
}
