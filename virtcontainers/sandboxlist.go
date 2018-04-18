// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"sync"
)

type sandboxList struct {
	lock      sync.RWMutex
	sandboxes map[string]*Sandbox
}

// globalSandboxList tracks sandboxes globally
var globalSandboxList = &sandboxList{sandboxes: make(map[string]*Sandbox)}

func (p *sandboxList) addSandbox(sandbox *Sandbox) (err error) {
	if sandbox == nil {
		return nil
	}

	p.lock.Lock()
	defer p.lock.Unlock()
	if p.sandboxes[sandbox.id] == nil {
		p.sandboxes[sandbox.id] = sandbox
	} else {
		err = fmt.Errorf("sandbox %s exists", sandbox.id)
	}
	return err
}

func (p *sandboxList) removeSandbox(id string) {
	p.lock.Lock()
	defer p.lock.Unlock()
	delete(p.sandboxes, id)
}

func (p *sandboxList) lookupSandbox(id string) (*Sandbox, error) {
	p.lock.RLock()
	defer p.lock.RUnlock()
	if p.sandboxes[id] != nil {
		return p.sandboxes[id], nil
	}
	return nil, fmt.Errorf("sandbox %s does not exist", id)
}
