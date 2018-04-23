// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
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
func (n *noopAgent) init(sandbox *Sandbox, config interface{}) error {
	return nil
}

// createSandbox is the Noop agent sandbox creation implementation. It does nothing.
func (n *noopAgent) createSandbox(sandbox *Sandbox) error {
	return nil
}

// capabilities returns empty capabilities, i.e no capabilties are supported.
func (n *noopAgent) capabilities() capabilities {
	return capabilities{}
}

// disconnect is the Noop agent connection closer. It does nothing.
func (n *noopAgent) disconnect() error {
	return nil
}

// exec is the Noop agent command execution implementation. It does nothing.
func (n *noopAgent) exec(sandbox *Sandbox, c Container, cmd Cmd) (*Process, error) {
	return nil, nil
}

// startSandbox is the Noop agent Sandbox starting implementation. It does nothing.
func (n *noopAgent) startSandbox(sandbox *Sandbox) error {
	return nil
}

// stopSandbox is the Noop agent Sandbox stopping implementation. It does nothing.
func (n *noopAgent) stopSandbox(sandbox *Sandbox) error {
	return nil
}

// createContainer is the Noop agent Container creation implementation. It does nothing.
func (n *noopAgent) createContainer(sandbox *Sandbox, c *Container) (*Process, error) {
	return &Process{}, nil
}

// startContainer is the Noop agent Container starting implementation. It does nothing.
func (n *noopAgent) startContainer(sandbox *Sandbox, c *Container) error {
	return nil
}

// stopContainer is the Noop agent Container stopping implementation. It does nothing.
func (n *noopAgent) stopContainer(sandbox *Sandbox, c Container) error {
	return nil
}

// killContainer is the Noop agent Container signaling implementation. It does nothing.
func (n *noopAgent) killContainer(sandbox *Sandbox, c Container, signal syscall.Signal, all bool) error {
	return nil
}

// processListContainer is the Noop agent Container ps implementation. It does nothing.
func (n *noopAgent) processListContainer(sandbox *Sandbox, c Container, options ProcessListOptions) (ProcessList, error) {
	return nil, nil
}

// onlineCPUMem is the Noop agent Container online CPU and Memory implementation. It does nothing.
func (n *noopAgent) onlineCPUMem(cpus uint32) error {
	return nil
}
