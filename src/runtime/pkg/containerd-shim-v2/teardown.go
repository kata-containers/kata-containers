// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"time"
)

const cleanupTimeout = 60 * time.Second

func newCleanupContext(ctx context.Context) (context.Context, context.CancelFunc) {
	return context.WithTimeout(context.WithoutCancel(ctx), cleanupTimeout)
}

func (s *service) teardownSandbox(ctx context.Context) {
	if s.sandbox == nil {
		return
	}

	s.teardownOnce.Do(func() {
		stopCtx, cancelStop := newCleanupContext(ctx)
		if err := s.sandbox.Stop(stopCtx, true); err != nil {
			shimLog.WithError(err).WithField("sandbox", s.sandbox.ID()).Error("failed to stop sandbox")
		}
		cancelStop()
		deleteCtx, cancelDelete := newCleanupContext(ctx)
		defer cancelDelete()
		if err := s.sandbox.Delete(deleteCtx); err != nil {
			shimLog.WithError(err).WithField("sandbox", s.sandbox.ID()).Error("failed to delete sandbox")
		}
	})
}
