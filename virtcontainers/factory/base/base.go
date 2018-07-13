// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package base

import vc "github.com/kata-containers/runtime/virtcontainers"

// FactoryBase is vm factory's internal base factory interfaces.
// The difference between FactoryBase and Factory is that the Factory
// also handles vm config validation/comparison and possible CPU/memory
// hotplugs. It's better to do it at the factory level instead of doing
// the same work in each of the factory implementations.
type FactoryBase interface {
	// Config returns base factory config.
	Config() vc.VMConfig

	// GetBaseVM returns a paused VM created by the base factory.
	GetBaseVM() (*vc.VM, error)

	// CloseFactory closes the base factory.
	CloseFactory()
}
