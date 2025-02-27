// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"path/filepath"
	"syscall"

	merr "github.com/hashicorp/go-multierror"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/pkg/errors"
	otelLabel "go.opentelemetry.io/otel/attribute"
)

// Sadly golang/sys doesn't have UmountNoFollow although it's there since Linux 2.6.34
const UmountNoFollow = 0x8

var propagationTypes = map[string]uintptr{
	"shared":  syscall.MS_SHARED,
	"private": syscall.MS_PRIVATE,
	"slave":   syscall.MS_SLAVE,
	"ubind":   syscall.MS_UNBINDABLE,
}

// bindMount bind mounts a source in to a destination. This will
// do some bookkeeping:
// * evaluate all symlinks
// * ensure the source exists
// * recursively create the destination
// pgtypes stands for propagation types, which are shared, private, slave, and ubind.
func bindMount(ctx context.Context, source, destination string, readonly bool, pgtypes string) error {
	span, _ := katatrace.Trace(ctx, nil, "bindMount", mountTracingTags)
	defer span.End()
	span.SetAttributes(otelLabel.String("source", source), otelLabel.String("destination", destination))

	absSource, destination, err := evalMountPath(source, destination)
	if err != nil {
		return err
	}
	span.SetAttributes(otelLabel.String("source_after_eval", absSource))

	if err := syscall.Mount(absSource, destination, "bind", syscall.MS_BIND, ""); err != nil {
		return fmt.Errorf("Could not bind mount %v to %v: %v", absSource, destination, err)
	}

	if pgtype, exist := propagationTypes[pgtypes]; exist {
		if err := syscall.Mount("none", destination, "", pgtype, ""); err != nil {
			return fmt.Errorf("Could not make mount point %v %s: %v", destination, pgtypes, err)
		}
	} else {
		return fmt.Errorf("Wrong propagation type %s", pgtypes)
	}

	// For readonly bind mounts, we need to remount with the readonly flag.
	// This is needed as only very recent versions of libmount/util-linux support "bind,ro"
	if readonly {
		return syscall.Mount(absSource, destination, "bind", uintptr(syscall.MS_BIND|syscall.MS_REMOUNT|syscall.MS_RDONLY), "")
	}

	return nil
}

// An existing mount may be remounted by specifying `MS_REMOUNT` in
// mountflags.
// This allows you to change the mountflags of an existing mount.
// The mountflags should match the values used in the original mount() call,
// except for those parameters that you are trying to change.
func remount(ctx context.Context, mountflags uintptr, src string) error {
	span, _ := katatrace.Trace(ctx, nil, "remount", mountTracingTags)
	defer span.End()
	span.SetAttributes(otelLabel.String("source", src))

	absSrc, err := filepath.EvalSymlinks(src)
	if err != nil {
		return fmt.Errorf("Could not resolve symlink for %s", src)
	}
	span.SetAttributes(otelLabel.String("source_after_eval", absSrc))

	if err := syscall.Mount(absSrc, absSrc, "", syscall.MS_REMOUNT|mountflags, ""); err != nil {
		return fmt.Errorf("remount %s failed: %v", absSrc, err)
	}

	return nil
}

// remount a mount point as readonly
func remountRo(ctx context.Context, src string) error {
	return remount(ctx, syscall.MS_BIND|syscall.MS_RDONLY, src)
}

// bindMountContainerRootfs bind mounts a container rootfs into a 9pfs shared
// directory between the guest and the host.
func bindMountContainerRootfs(ctx context.Context, shareDir, cid, cRootFs string, readonly bool) error {
	span, _ := katatrace.Trace(ctx, nil, "bindMountContainerRootfs", mountTracingTags)
	defer span.End()

	rootfsDest := filepath.Join(shareDir, cid, rootfsDir)

	return bindMount(ctx, cRootFs, rootfsDest, readonly, "private")
}

func bindUnmountContainerShareDir(ctx context.Context, sharedDir, cID, target string) error {
	destDir := filepath.Join(sharedDir, cID, target)
	if isSymlink(filepath.Join(sharedDir, cID)) || isSymlink(destDir) {
		mountLogger().WithField("container", cID).Warnf("container dir is a symlink, malicious guest?")
		return nil
	}

	err := syscall.Unmount(destDir, syscall.MNT_DETACH|UmountNoFollow)
	if err == syscall.ENOENT {
		mountLogger().WithError(err).WithField("share-dir", destDir).Warn()
		return nil
	}
	if err := syscall.Rmdir(destDir); err != nil {
		mountLogger().WithError(err).WithField("share-dir", destDir).Warn("Could not remove container share dir")
	}

	return err
}

func bindUnmountContainerRootfs(ctx context.Context, sharedDir, cID string) error {
	span, _ := katatrace.Trace(ctx, nil, "bindUnmountContainerRootfs", mountTracingTags)
	defer span.End()
	span.SetAttributes(otelLabel.String("shared-dir", sharedDir), otelLabel.String("container-id", cID))
	return bindUnmountContainerShareDir(ctx, sharedDir, cID, rootfsDir)
}

func bindUnmountContainerSnapshotDir(ctx context.Context, sharedDir, cID string) error {
	span, _ := katatrace.Trace(ctx, nil, "bindUnmountContainerSnapshotDir", mountTracingTags)
	defer span.End()
	span.SetAttributes(otelLabel.String("shared-dir", sharedDir), otelLabel.String("container-id", cID))
	return bindUnmountContainerShareDir(ctx, sharedDir, cID, snapshotDir)
}

func getVirtiofsDaemonForNydus(sandbox *Sandbox) (VirtiofsDaemon, error) {
	var virtiofsDaemon VirtiofsDaemon
	switch sandbox.GetHypervisorType() {
	case string(QemuHypervisor):
		virtiofsDaemon = sandbox.hypervisor.(*qemu).virtiofsDaemon
	case string(ClhHypervisor):
		virtiofsDaemon = sandbox.hypervisor.(*cloudHypervisor).virtiofsDaemon
	default:
		return nil, errNydusdNotSupport
	}
	return virtiofsDaemon, nil
}

func nydusContainerCleanup(ctx context.Context, sharedDir string, c *Container) error {
	sandbox := c.sandbox
	virtiofsDaemon, err := getVirtiofsDaemonForNydus(sandbox)
	if err != nil {
		return err
	}
	if err := virtiofsDaemon.Umount(rafsMountPath(c.id)); err != nil {
		return errors.Wrap(err, "umount rafs failed")
	}
	if err := bindUnmountContainerSnapshotDir(ctx, sharedDir, c.id); err != nil {
		return errors.Wrap(err, "umount snapshotdir err")
	}
	destDir := filepath.Join(sharedDir, c.id, c.rootfsSuffix)
	if err := syscall.Rmdir(destDir); err != nil {
		return errors.Wrap(err, "remove container rootfs err")
	}
	return nil
}

func bindUnmountAllRootfs(ctx context.Context, sharedDir string, sandbox *Sandbox) error {
	span, ctx := katatrace.Trace(ctx, nil, "bindUnmountAllRootfs", mountTracingTags)
	defer span.End()
	span.SetAttributes(otelLabel.String("shared-dir", sharedDir), otelLabel.String("sandbox-id", sandbox.id))

	var errors *merr.Error
	for _, c := range sandbox.containers {
		if isSymlink(filepath.Join(sharedDir, c.id)) {
			mountLogger().WithField("container", c.id).Warnf("container dir is a symlink, malicious guest?")
			continue
		}
		c.unmountHostMounts(ctx)
		if c.state.Fstype == "" {
			// even if error found, don't break out of loop until all mounts attempted
			// to be unmounted, and collect all errors
			if IsNydusRootFSType(c.state.Fstype) {
				errors = merr.Append(errors, nydusContainerCleanup(ctx, sharedDir, c))
			} else {
				errors = merr.Append(errors, bindUnmountContainerRootfs(ctx, sharedDir, c.id))
			}
		}
	}
	return errors.ErrorOrNil()
}
