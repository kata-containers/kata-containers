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

type podList struct {
	lock sync.RWMutex
	pods map[string]*Pod
}

// globalPodList tracks pods globally
var globalPodList = &podList{pods: make(map[string]*Pod)}

func (p *podList) addPod(pod *Pod) (err error) {
	if pod == nil {
		return nil
	}

	p.lock.Lock()
	defer p.lock.Unlock()
	if p.pods[pod.id] == nil {
		p.pods[pod.id] = pod
	} else {
		err = fmt.Errorf("pod %s exists", pod.id)
	}
	return err
}

func (p *podList) removePod(id string) {
	p.lock.Lock()
	defer p.lock.Unlock()
	delete(p.pods, id)
}

func (p *podList) lookupPod(id string) (*Pod, error) {
	p.lock.RLock()
	defer p.lock.RUnlock()
	if p.pods[id] != nil {
		return p.pods[id], nil
	}
	return nil, fmt.Errorf("pod %s does not exist", id)
}
