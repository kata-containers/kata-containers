// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"path"
	"time"

	"github.com/containerd/containerd/api/types/task"
	"github.com/containerd/containerd/v2/core/mount"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
)

const containerTeardownTimeout = 10 * time.Second

func waitForContainerTeardown(ctx context.Context, done <-chan struct{}) error {
	if done == nil {
		return nil
	}

	ctx, cancel := context.WithTimeout(ctx, containerTeardownTimeout)
	defer cancel()

	select {
	case <-done:
		return nil
	case <-ctx.Done():
		select {
		case <-done:
			return nil
		default:
			return ctx.Err()
		}
	}
}

func deleteContainer(ctx context.Context, s *service, c *container) error {
	if !c.cType.IsSandbox() {
		if c.status != task.Status_STOPPED {
			if _, err := s.sandbox.StopContainer(ctx, c.id, false); err != nil && !isNotFound(err) {
				return err
			}
		}

		if _, err := s.sandbox.DeleteContainer(ctx, c.id); err != nil && !isNotFound(err) {
			return err
		}
	}

	// Run post-stop OCI hooks.
	if err := katautils.PostStopHooks(ctx, *c.spec, s.sandbox.ID(), c.bundle); err != nil {
		// log warning and continue, as defined in oci runtime spec
		// https://github.com/opencontainers/runtime-spec/blob/master/runtime.md#lifecycle
		shimLog.WithError(err).Warn("Failed to run post-stop hooks")
	}

	if c.mounted {
		rootfs := path.Join(c.bundle, "rootfs")
		if err := mount.UnmountAll(rootfs, 0); err != nil {
			shimLog.WithError(err).Warn("failed to cleanup rootfs mount")
		}
	}

	delete(s.containers, c.id)

	return nil
}
