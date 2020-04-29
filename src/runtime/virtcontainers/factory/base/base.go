// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package base

import (
	"context"

	pb "github.com/kata-containers/kata-containers/src/runtime/protocols/cache"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
)

// FactoryBase is vm factory's internal base factory interfaces.
// The difference between FactoryBase and Factory is that the Factory
// also handles vm config validation/comparison and possible CPU/memory
// hotplugs. It's better to do it at the factory level instead of doing
// the same work in each of the factory implementations.
type FactoryBase interface {
	// Config returns base factory config.
	Config() vc.VMConfig

	// GetVMStatus returns the status of the paused VM created by the base factory.
	GetVMStatus() []*pb.GrpcVMStatus

	// GetBaseVM returns a paused VM created by the base factory.
	GetBaseVM(ctx context.Context, config vc.VMConfig) (*vc.VM, error)

	// CloseFactory closes the base factory.
	CloseFactory(ctx context.Context)
}
