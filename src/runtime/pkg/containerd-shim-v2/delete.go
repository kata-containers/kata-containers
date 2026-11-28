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

// teardownWaitTimeout bounds how long deleteContainer waits for the
// in-flight post-exit teardown started by wait() before proceeding anyway
// (which restores the pre-fix racing behavior).  Package-level var so tests
// can shorten it.
var teardownWaitTimeout = 10 * time.Second

func deleteContainer(ctx context.Context, s *service, c *container) error {
	if !c.cType.IsSandbox() {
		if c.status != task.Status_STOPPED {
			if _, err := s.sandbox.StopContainer(ctx, c.id, false); err != nil && !isNotFound(err) {
				return err
			}
		} else {
			// c.status == STOPPED is set by wait() under s.mu right before
			// it starts the teardown (which runs without s.mu, see
			// d3291b877), so the teardown may still be in flight and the
			// virtcontainers state still StateRunning.  Wait for it,
			// bounded, so DeleteContainer below does not fail with
			// "Container not ready or stopped, impossible to delete".  On
			// timeout or cancellation, proceed; the isNotFound tolerance
			// below remains the safety net for the fully torn-down case.
			select {
			case <-c.teardownDone:
			case <-ctx.Done():
				shimLog.WithField("container", c.id).Warn("Delete context cancelled while waiting for container teardown")
			case <-time.After(teardownWaitTimeout):
				shimLog.WithField("container", c.id).Warn("timed out waiting for container teardown, proceeding with delete")
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
