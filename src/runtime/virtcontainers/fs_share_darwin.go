// Copyright (c) 2023 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"sync"

	"github.com/sirupsen/logrus"
)

type FilesystemShare struct {
	sandbox *Sandbox
	sync.Mutex
	prepared bool
}

func NewFilesystemShare(s *Sandbox) (FilesystemSharer, error) {
	return &FilesystemShare{
		prepared: false,
		sandbox:  s,
	}, nil
}

// Logger returns a logrus logger appropriate for logging Filesystem sharing messages
func (f *FilesystemShare) Logger() *logrus.Entry {
	return virtLog.WithFields(logrus.Fields{
		"subsystem": "filesystem share",
		"sandbox":   f.sandbox.ID(),
	})
}

func (f *FilesystemShare) Prepare(ctx context.Context) error {
	return nil
}

func (f *FilesystemShare) Cleanup(ctx context.Context) error {
	return nil
}

func (f *FilesystemShare) ShareFile(ctx context.Context, c *Container, m *Mount) (*SharedFile, error) {
	return nil, nil
}

func (f *FilesystemShare) UnshareFile(ctx context.Context, c *Container, m *Mount) error {
	return nil
}

func (f *FilesystemShare) ShareRootFilesystem(ctx context.Context, c *Container) (*SharedFile, error) {
	return nil, nil
}

func (f *FilesystemShare) UnshareRootFilesystem(ctx context.Context, c *Container) error {
	return nil
}

func (f *FilesystemShare) StartFileEventWatcher(ctx context.Context) error {
	return nil
}

func (f *FilesystemShare) StopFileEventWatcher(ctx context.Context) {
}
