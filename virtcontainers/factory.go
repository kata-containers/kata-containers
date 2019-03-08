// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import "context"

// Factory controls how a new VM is created.
type Factory interface {
	// Config returns base factory config.
	Config() VMConfig

	// GetVM gets a new VM from the factory.
	GetVM(ctx context.Context, config VMConfig) (*VM, error)

	// GetBaseVM returns a paused VM created by the base factory.
	GetBaseVM(ctx context.Context, config VMConfig) (*VM, error)

	// CloseFactory closes and cleans up the factory.
	CloseFactory(ctx context.Context)
}
