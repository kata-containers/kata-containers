// Copyright (c) 2017 Intel Corporation
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

package vcmock

import (
	vc "github.com/kata-containers/runtime/virtcontainers"
)

// ID implements the VCContainer function of the same name.
func (c *Container) ID() string {
	return c.MockID
}

// Pod implements the VCContainer function of the same name.
func (c *Container) Pod() vc.VCPod {
	return c.MockPod
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

// SetPid implements the VCContainer function of the same name.
func (c *Container) SetPid(pid int) error {
	c.MockPid = pid
	return nil
}

// GetAnnotations implements the VCContainer function of the same name.
func (c *Container) GetAnnotations() map[string]string {
	return c.MockAnnotations
}
