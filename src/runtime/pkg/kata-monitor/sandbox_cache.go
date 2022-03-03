// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"sync"
)

type sandboxCRIMetadata struct {
	uid       string
	name      string
	namespace string
}
type sandboxCache struct {
	*sync.Mutex
	// the sandboxCRIMetadata links the sandbox id from the container manager to the pod metadata of kubernetes
	sandboxes map[string]sandboxCRIMetadata
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

func (sc *sandboxCache) putIfNotExists(id string, value sandboxCRIMetadata) bool {
	sc.Lock()
	defer sc.Unlock()

	if _, found := sc.sandboxes[id]; !found {
		sc.sandboxes[id] = value
		return true
	}

	// already in sandbox cache
	return false
}

func (sc *sandboxCache) setCRIMetadata(id string, value sandboxCRIMetadata) {
	sc.Lock()
	defer sc.Unlock()

	sc.sandboxes[id] = value
}

func (sc *sandboxCache) getCRIMetadata(id string) (sandboxCRIMetadata, bool) {
	sc.Lock()
	defer sc.Unlock()

	metadata, ok := sc.sandboxes[id]
	return metadata, ok
}
