// Copyright (c) 2022 Intel.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerstatus

import (
	"sync"

	"github.com/containerd/containerd/api/types/task"
)

// ContainerStatus is a thread-safe container status
// that can be used to get the current status of a container
// and set the status of a container.
// The status includes an error, if there is an  error in the container lifecycle,
// this can be set by the SetError method.
// This may be useul for the shim to propagate the error or take extra actions
// if the error is set  but the container is still running.
type ContainerStatus struct {
	err error
	sync.Mutex
	status task.Status
}

// Get returns the current status of the container
// and the error if there is an error in the container lifecycle.
func (c *ContainerStatus) Get() (task.Status, error) {
	c.Lock()
	defer c.Unlock()
	return c.status, c.err
}

// Set sets the status of the container
func (c *ContainerStatus) Set(status task.Status) {
	c.Lock()
	defer c.Unlock()
	c.status = status
}

// SetError sets the error of the container
func (c *ContainerStatus) SetError(err error) {
	c.Lock()
	defer c.Unlock()
	c.err = err
}
