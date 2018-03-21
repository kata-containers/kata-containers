//
// Copyright (c) 2016 Intel Corporation
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
	"syscall"
)

// noopAgent a.k.a. NO-OP Agent is an empty Agent implementation, for testing and
// mocking purposes.
type noopAgent struct {
}

// init initializes the Noop agent, i.e. it does nothing.
func (n *noopAgent) init(pod *Pod, config interface{}) error {
	return nil
}

// createPod is the Noop agent pod creation implementation. It does nothing.
func (n *noopAgent) createPod(pod *Pod) error {
	return nil
}

// capabilities returns empty capabilities, i.e no capabilties are supported.
func (n *noopAgent) capabilities() capabilities {
	return capabilities{}
}

// exec is the Noop agent command execution implementation. It does nothing.
func (n *noopAgent) exec(pod *Pod, c Container, cmd Cmd) (*Process, error) {
	return nil, nil
}

// startPod is the Noop agent Pod starting implementation. It does nothing.
func (n *noopAgent) startPod(pod Pod) error {
	return nil
}

// stopPod is the Noop agent Pod stopping implementation. It does nothing.
func (n *noopAgent) stopPod(pod Pod) error {
	return nil
}

// createContainer is the Noop agent Container creation implementation. It does nothing.
func (n *noopAgent) createContainer(pod *Pod, c *Container) (*Process, error) {
	return &Process{}, nil
}

// startContainer is the Noop agent Container starting implementation. It does nothing.
func (n *noopAgent) startContainer(pod Pod, c *Container) error {
	return nil
}

// stopContainer is the Noop agent Container stopping implementation. It does nothing.
func (n *noopAgent) stopContainer(pod Pod, c Container) error {
	return nil
}

// killContainer is the Noop agent Container signaling implementation. It does nothing.
func (n *noopAgent) killContainer(pod Pod, c Container, signal syscall.Signal, all bool) error {
	return nil
}

// processListContainer is the Noop agent Container ps implementation. It does nothing.
func (n *noopAgent) processListContainer(pod Pod, c Container, options ProcessListOptions) (ProcessList, error) {
	return nil, nil
}

// onlineCPUMem is the Noop agent Container online CPU and Memory implementation. It does nothing.
func (n *noopAgent) onlineCPUMem() error {
	return nil
}
