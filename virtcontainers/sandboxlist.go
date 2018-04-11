//
// Copyright (c) 2018 HyperHQ Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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
