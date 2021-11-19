// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/containerd/containerd/mount"
	cdshim "github.com/containerd/containerd/runtime/v2/shim"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
)

func cReap(s *service, status int, id, execid string, exitat time.Time) {
	s.ec <- exit{
		timestamp: exitat,
		pid:       s.hpid,
		status:    status,
		id:        id,
		execid:    execid,
	}
}

func cleanupContainer(ctx context.Context, sandboxID, cid, bundlePath string) error {
	shimLog.WithField("service", "cleanup").WithField("container", cid).Info("Cleanup container")

	err := vci.CleanupContainer(ctx, sandboxID, cid, true)
	if err != nil {
		shimLog.WithError(err).WithField("container", cid).Warn("failed to cleanup container")
		return err
	}

	rootfs := filepath.Join(bundlePath, "rootfs")

	if err := mount.UnmountAll(rootfs, 0); err != nil {
		shimLog.WithError(err).WithField("container", cid).Warn("failed to cleanup container rootfs")
		return err
	}

	return nil
}

func validBundle(containerID, bundlePath string) (string, error) {
	// container ID MUST be provided.
	if containerID == "" {
		return "", fmt.Errorf("Missing container ID")
	}

	// bundle path MUST be provided.
	if bundlePath == "" {
		return "", fmt.Errorf("Missing bundle path")
	}

	// bundle path MUST be valid.
	fileInfo, err := os.Stat(bundlePath)
	if err != nil {
		return "", fmt.Errorf("Invalid bundle path '%s': %s", bundlePath, err)
	}
	if !fileInfo.IsDir() {
		return "", fmt.Errorf("Invalid bundle path '%s', it should be a directory", bundlePath)
	}

	resolved, err := katautils.ResolvePath(bundlePath)
	if err != nil {
		return "", err
	}

	return resolved, nil
}

func getAddress(ctx context.Context, bundlePath, address, id string) (string, error) {
	var err error

	// Checks the MUST and MUST NOT from OCI runtime specification
	if bundlePath, err = validBundle(id, bundlePath); err != nil {
		return "", err
	}

	ociSpec, err := compatoci.ParseConfigJSON(bundlePath)
	if err != nil {
		return "", err
	}

	containerType, err := oci.ContainerType(ociSpec)
	if err != nil {
		return "", err
	}

	if containerType == vc.PodContainer {
		sandboxID, err := oci.SandboxID(ociSpec)
		if err != nil {
			return "", err
		}
		address, err := cdshim.SocketAddress(ctx, address, sandboxID)
		if err != nil {
			return "", err
		}
		return address, nil
	}

	return "", nil
}

func noNeedForOutput(detach bool, tty bool) bool {
	return detach && tty
}
