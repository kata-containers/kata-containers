// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package vcmock

import (
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
)

// ID implements the VCContainer function of the same name.
func (c *Container) ID() string {
	return c.MockID
}

// Sandbox implements the VCContainer function of the same name.
func (c *Container) Sandbox() vc.VCSandbox {
	return c.MockSandbox
}

// Process implements the VCContainer function of the same name.
func (c *Container) Process() vc.Process {
	// always return a mockprocess with a non-zero Pid
	if c.MockProcess.Pid == 0 {
		c.MockProcess.Pid = 1000
	}
	return c.MockProcess
}

// GetToken implements the VCContainer function of the same name.
func (c *Container) GetToken() string {
	return c.MockToken
}

// GetPid implements the VCContainer function of the same name.
func (c *Container) GetPid() int {
	return c.MockPid
}

// GetAnnotations implements the VCContainer function of the same name.
func (c *Container) GetAnnotations() map[string]string {
	return c.MockAnnotations
}
