// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"

	pb "github.com/kata-containers/kata-containers/src/runtime/protocols/cache"
)

// Factory controls how a new VM is created.
type Factory interface {
	// Config returns base factory config.
	Config() VMConfig

	// GetVMStatus returns the status of the paused VM created by the base factory.
	GetVMStatus() []*pb.GrpcVMStatus

	// GetVM gets a new VM from the factory.
	GetVM(ctx context.Context, config VMConfig) (*VM, error)

	// GetBaseVM returns a paused VM created by the base factory.
	GetBaseVM(ctx context.Context, config VMConfig) (*VM, error)

	// CloseFactory closes and cleans up the factory.
	CloseFactory(ctx context.Context)
}
