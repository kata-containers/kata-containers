// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persistapi

// PersistDriver is interface describing operations to save/restore persist data
type PersistDriver interface {
	// ToDisk flushes data to disk(or other storage media such as a remote db)
	ToDisk() error
	// FromDisk will restore all data for sandbox with `sid` from storage.
	// We only support get data for one whole sandbox
	FromDisk(sid string) error
	// AddSaveCallback addes callback function named `name` to driver storage list
	// The callback functions will be invoked when calling `ToDisk()`, notice that
	// callback functions are not order guaranteed,
	AddSaveCallback(name string, f SetFunc)
	// Destroy will remove everything from storage
	Destroy() error
	// GetStates will return SandboxState and ContainerState(s) directly
	GetStates() (*SandboxState, map[string]ContainerState, error)
}
