// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Description: The true virtcontainers function of the same name.
// This indirection is required to allow an alternative implemenation to be
// used for testing purposes.

package virtcontainers

import (
	"context"

	"github.com/sirupsen/logrus"
)

// VCImpl is the official virtcontainers function of the same name.
type VCImpl struct {
	factory Factory
}

// SetLogger implements the VC function of the same name.
func (impl *VCImpl) SetLogger(ctx context.Context, logger *logrus.Entry) {
	SetLogger(ctx, logger)
}

// SetFactory implements the VC function of the same name.
func (impl *VCImpl) SetFactory(ctx context.Context, factory Factory) {
	impl.factory = factory
}

// CreateSandbox implements the VC function of the same name.
func (impl *VCImpl) CreateSandbox(ctx context.Context, sandboxConfig SandboxConfig) (VCSandbox, error) {
	return CreateSandbox(ctx, sandboxConfig, impl.factory)
}

// CleanupContainer is used by shimv2 to stop and delete a container exclusively, once there is no container
// in the sandbox left, do stop the sandbox and delete it. Those serial operations will be done exclusively by
// locking the sandbox.
func (impl *VCImpl) CleanupContainer(ctx context.Context, sandboxID, containerID string, force bool) error {
	return CleanupContainer(ctx, sandboxID, containerID, force)
}
