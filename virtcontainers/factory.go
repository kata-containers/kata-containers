// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

// Factory controls how a new VM is created.
type Factory interface {
	// GetVM gets a new VM from the factory.
	GetVM(config VMConfig) (*VM, error)

	// CloseFactory closes and cleans up the factory.
	CloseFactory()
}
