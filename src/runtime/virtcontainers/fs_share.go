// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
)

// fsShareTracingTags defines tags for the trace span
var fsShareTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "fs_share",
}

// SharedFile represents the outcome of a host filesystem sharing
// operation.
type SharedFile struct {
	containerStorages []*grpc.Storage
	volumeStorages    []*grpc.Storage
	guestPath         string
}

type FilesystemSharer interface {
	// Prepare will set the host filesystem up, making it ready
	// to share container files and directories with the guest.
	// It will be called before any container is running.
	//
	// For example, the Linux implementation would create and
	// prepare the host shared folders, and also make all
	// sandbox mounts ready to be shared.
	//
	// Implementation of this method must be idempotent and be
	// ready to potentially be called several times in a row,
	// without symmetric calls to Cleanup in between.
	Prepare(context.Context) error

	// Cleanup cleans the host filesystem up from the initial
	// setup created by Prepare.
	// It will be called after all containers are terminated.
	//
	// Implementation of this method must be idempotent and be
	// ready to potentially be called several times in a row,
	// without symmetric calls to Prepare in between.
	Cleanup(context.Context) error

	// ShareFile shares a file (a regular file or a directory)
	// from the host filesystem with a container running in the
	// guest. The host file to be shared is described by the
	// Mount argument.
	// This method should be called for each container file to
	// be shared with the guest.
	//
	// The returned SharedFile pointer describes how the file
	// should be shared between the host and the guest. If it
	// is nil, then the shared filed described by the Mount
	// argument will be ignored by the guest, i.e. it will NOT
	// be shared with the guest.
	ShareFile(context.Context, *Container, *Mount) (*SharedFile, error)

	// UnshareFile stops sharing a container file, described by
	// the Mount argument.
	UnshareFile(context.Context, *Container, *Mount) error

	// ShareRootFilesystem shares a container bundle rootfs with
	// the Kata guest, allowing the kata agent to eventually start
	// the container from that shared rootfs.
	ShareRootFilesystem(context.Context, *Container) (*SharedFile, error)

	// UnshareRootFilesystem stops sharing a container bundle
	// rootfs.
	UnshareRootFilesystem(context.Context, *Container) error

	// startFileEventWatcher is the event loop to detect changes in
	// specific volumes - configmap, secrets, downward-api, projected-volumes
	// and copy the changes to the guest
	StartFileEventWatcher(context.Context) error

	// Stops the event loop for file watcher
	StopFileEventWatcher(context.Context)
}
