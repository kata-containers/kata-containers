// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persistapi

// PersistDriver is interface describing operations to save/restore persist data
type PersistDriver interface {
	// ToDisk flushes data to disk(or other storage media such as a remote db)
	ToDisk(SandboxState, map[string]ContainerState) error
	// FromDisk will restore all data for sandbox with `sid` from storage.
	// We only support get data for one whole sandbox
	FromDisk(sid string) (SandboxState, map[string]ContainerState, error)
	// Destroy will remove everything from storage
	Destroy(sid string) error
	// Lock locks the persist driver, "exclusive" decides whether the lock is exclusive or shared.
	// It returns Unlock Function and errors
	Lock(sid string, exclusive bool) (func() error, error)

	// RunStoragePath is the sandbox runtime directory.
	// It will contain one state.json and one lock file for each created sandbox.
	RunStoragePath() string

	// RunVMStoragePath is the vm directory.
	// It will contain all guest vm sockets and shared mountpoints.
	RunVMStoragePath() string
}
