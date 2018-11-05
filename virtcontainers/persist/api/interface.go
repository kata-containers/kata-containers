// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persistapi

// PersistDriver is interface describing operations to save/restore persist data
type PersistDriver interface {
	// Dump persist data to
	Dump() error
	Restore(sid string) error
	Destroy() error
	GetStates() (*SandboxState, map[string]ContainerState, error)
	RegisterHook(name string, f SetFunc)
}
