// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"path"

	"github.com/containerd/containerd/api/types/task"
	"github.com/containerd/containerd/mount"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
)

func deleteContainer(ctx context.Context, s *service, c *container) error {
	if !c.cType.IsSandbox() {
		if c.status != task.StatusStopped {
			_, err := s.sandbox.StopContainer(c.id, false)
			if err != nil {
				return err
			}
		}

		if _, err := s.sandbox.DeleteContainer(c.id); err != nil {
			return err
		}
	}

	// Run post-stop OCI hooks.
	if err := katautils.PostStopHooks(ctx, *c.spec, s.sandbox.ID(), c.bundle); err != nil {
		return err
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
