// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2014,2015,2016,2017 Docker, Inc.
// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"syscall"

	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

func unmountNoFollow(path string) error {
	return syscall.Unmount(path, syscall.MNT_DETACH|UmountNoFollow)
}

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

func (f *FilesystemShare) prepareBindMounts(ctx context.Context) error {
	span, ctx := katatrace.Trace(ctx, f.Logger(), "setupBindMounts", fsShareTracingTags)
	defer span.End()

	var err error

	if len(f.sandbox.config.SandboxBindMounts) == 0 {
		return nil
	}

	// Create subdirectory in host shared path for sandbox mounts
	sandboxMountDir := filepath.Join(getMountPath(f.sandbox.ID()), sandboxMountsDir)
	sandboxShareDir := filepath.Join(GetSharePath(f.sandbox.ID()), sandboxMountsDir)
	if err := os.MkdirAll(sandboxMountDir, DirMode); err != nil {
		return fmt.Errorf("Creating sandbox shared mount directory: %v: %w", sandboxMountDir, err)
	}
	var mountedList []string
	defer func() {
		if err != nil {
			for _, mnt := range mountedList {
				if derr := unmountNoFollow(mnt); derr != nil {
					f.Logger().WithError(derr).Errorf("Cleanup: couldn't unmount %s", mnt)
				}
			}
			if derr := os.RemoveAll(sandboxMountDir); derr != nil {
				f.Logger().WithError(derr).Errorf("Cleanup: failed to remove %s", sandboxMountDir)
			}

		}
	}()

	for _, m := range f.sandbox.config.SandboxBindMounts {
		mountDest := filepath.Join(sandboxMountDir, filepath.Base(m))
		// bind-mount each sandbox mount that's defined into the sandbox mounts dir
		if err := bindMount(ctx, m, mountDest, true, "private"); err != nil {
			return fmt.Errorf("Mounting sandbox directory: %v to %v: %w", m, mountDest, err)
		}
		mountedList = append(mountedList, mountDest)

		mountDest = filepath.Join(sandboxShareDir, filepath.Base(m))
		if err := remountRo(ctx, mountDest); err != nil {
			return fmt.Errorf("remount sandbox directory: %v to %v: %w", m, mountDest, err)
		}
	}

	return nil
}

func (f *FilesystemShare) cleanupBindMounts(ctx context.Context) error {
	if f.sandbox.config == nil || len(f.sandbox.config.SandboxBindMounts) == 0 {
		return nil
	}

	var retErr error
	bindmountShareDir := filepath.Join(getMountPath(f.sandbox.ID()), sandboxMountsDir)
	for _, m := range f.sandbox.config.SandboxBindMounts {
		mountPath := filepath.Join(bindmountShareDir, filepath.Base(m))
		if err := unmountNoFollow(mountPath); err != nil {
			if retErr == nil {
				retErr = err
			}
			f.Logger().WithError(err).Errorf("Failed to unmount sandbox bindmount: %v", mountPath)
		}
	}
	if err := os.RemoveAll(bindmountShareDir); err != nil {
		if retErr == nil {
			retErr = err
		}
		f.Logger().WithError(err).Errorf("Failed to remove sandbox bindmount directory: %s", bindmountShareDir)
	}

	return retErr
}

func (f *FilesystemShare) Prepare(ctx context.Context) error {
	var err error

	span, ctx := katatrace.Trace(ctx, f.Logger(), "prepare", fsShareTracingTags)
	defer span.End()

	f.Lock()
	defer f.Unlock()

	// Prepare is idempotent, i.e. can be called multiple times in a row, without failing
	// and without modifying the filesystem state after the first call.
	if f.prepared {
		f.Logger().Warn("Calling Prepare() on an already prepared filesystem")
		return nil
	}

	// Toggle prepared to true if everything went fine.
	defer func() {
		if err == nil {
			f.prepared = true
		}
	}()

	// create shared path structure
	sharePath := GetSharePath(f.sandbox.ID())
	mountPath := getMountPath(f.sandbox.ID())
	if err = os.MkdirAll(sharePath, sharedDirMode); err != nil {
		return err
	}
	if err = os.MkdirAll(mountPath, DirMode); err != nil {
		return err
	}

	// slave mount so that future mountpoints under mountPath are shown in sharePath as well
	if err = bindMount(ctx, mountPath, sharePath, true, "slave"); err != nil {
		return err
	}
	defer func() {
		if err != nil {
			if umountErr := unmountNoFollow(sharePath); umountErr != nil {
				f.Logger().WithError(umountErr).Errorf("failed to unmount vm share path %s", sharePath)
			}
		}
	}()

	// Setup sandbox bindmounts, if specified.
	if err = f.prepareBindMounts(ctx); err != nil {
		return err
	}

	return nil
}

func (f *FilesystemShare) Cleanup(ctx context.Context) error {
	var err error

	f.Lock()
	defer f.Unlock()

	// Cleanup is idempotent, i.e. can be called multiple times in a row, without failing
	// and without modifying the filesystem state after the first call.
	if !f.prepared {
		f.Logger().Warn("Calling Cleanup() on an already cleaned up filesystem")
		return nil
	}

	// Toggle prepared to false if everything went fine.
	defer func() {
		if err == nil {
			f.prepared = false
		}
	}()

	// Unmount all the sandbox bind mounts.
	if err = f.cleanupBindMounts(ctx); err != nil {
		return err
	}

	// Unmount shared path
	path := GetSharePath(f.sandbox.ID())
	f.Logger().WithField("path", path).Infof("Cleanup agent")
	if err = unmountNoFollow(path); err != nil {
		f.Logger().WithError(err).Errorf("failed to unmount vm share path %s", path)
		return err
	}

	// Unmount mount path
	path = getMountPath(f.sandbox.ID())
	if err = bindUnmountAllRootfs(ctx, path, f.sandbox); err != nil {
		f.Logger().WithError(err).Errorf("failed to unmount vm mount path %s", path)
		return err
	}
	if err = os.RemoveAll(getSandboxPath(f.sandbox.ID())); err != nil {
		f.Logger().WithError(err).Errorf("failed to Cleanup vm path %s", getSandboxPath(f.sandbox.ID()))
		return err
	}

	return nil
}

func (f *FilesystemShare) ShareFile(ctx context.Context, c *Container, m *Mount) (*SharedFile, error) {
	randBytes, err := utils.GenerateRandomBytes(8)
	if err != nil {
		return nil, err
	}

	filename := fmt.Sprintf("%s-%s-%s", c.id, hex.EncodeToString(randBytes), filepath.Base(m.Destination))
	guestPath := filepath.Join(kataGuestSharedDir(), filename)

	// copy file to container's rootfs if filesystem sharing is not supported, otherwise
	// bind mount it in the shared directory.
	caps := f.sandbox.hypervisor.Capabilities(ctx)
	if !caps.IsFsSharingSupported() {
		f.Logger().Debug("filesystem sharing is not supported, files will be copied")

		fileInfo, err := os.Stat(m.Source)
		if err != nil {
			return nil, err
		}

		// Ignore the mount if this is not a regular file (excludes
		// directory, socket, device, ...) as it cannot be handled by
		// a simple copy. But this should not be treated as an error,
		// only as a limitation.
		if !fileInfo.Mode().IsRegular() {
			f.Logger().WithField("ignored-file", m.Source).Debug("Ignoring non-regular file as FS sharing not supported")
			return nil, nil
		}

		if err := f.sandbox.agent.copyFile(ctx, m.Source, guestPath); err != nil {
			return nil, err
		}
	} else {
		// These mounts are created in the shared dir
		mountDest := filepath.Join(getMountPath(f.sandbox.ID()), filename)
		if !m.ReadOnly {
			if err := bindMount(ctx, m.Source, mountDest, false, "private"); err != nil {
				return nil, err
			}
		} else {
			// For RO mounts, bindmount remount event is not propagated to mount subtrees,
			// and it doesn't present in the virtiofsd standalone mount namespace either.
			// So we end up a bit tricky:
			// 1. make a private ro bind mount to the mount source
			// 2. duplicate the ro mount we create in step 1 to mountDest, by making a bind mount. No need to remount with MS_RDONLY here.
			// 3. umount the private bind mount created in step 1
			privateDest := filepath.Join(getPrivatePath(f.sandbox.ID()), filename)

			if err := bindMount(ctx, m.Source, privateDest, true, "private"); err != nil {
				return nil, err
			}
			defer func() {
				unmountNoFollow(privateDest)
			}()

			if err := bindMount(ctx, privateDest, mountDest, false, "private"); err != nil {
				return nil, err
			}
		}

		// Save HostPath mount value into the passed mount
		m.HostPath = mountDest
	}

	return &SharedFile{
		guestPath: guestPath,
	}, nil
}

func (f *FilesystemShare) UnshareFile(ctx context.Context, c *Container, m *Mount) error {
	if err := unmountNoFollow(m.HostPath); err != nil {
		return err
	}

	if m.Type == "bind" {
		s, err := os.Stat(m.HostPath)
		if err != nil {
			return errors.Wrapf(err, "Could not stat host-path %v", m.HostPath)
		}
		// Remove the empty file or directory
		if s.Mode().IsRegular() && s.Size() == 0 {
			os.Remove(m.HostPath)
		}
		if s.Mode().IsDir() {
			syscall.Rmdir(m.HostPath)
		}
	}

	return nil
}

func (f *FilesystemShare) shareRootFilesystemWithNydus(ctx context.Context, c *Container) (*SharedFile, error) {
	rootfsGuestPath := filepath.Join(kataGuestSharedDir(), c.id, c.rootfsSuffix)
	virtiofsDaemon, err := getVirtiofsDaemonForNydus(f.sandbox)
	if err != nil {
		return nil, err
	}
	extraOption, err := parseExtraOption(c.rootFs.Options)
	if err != nil {
		return nil, err
	}
	f.Logger().Infof("Nydus option: %v", extraOption)
	mountOpt := &MountOption{
		mountpoint: rafsMountPath(c.id),
		source:     extraOption.Source,
		config:     extraOption.Config,
	}

	// mount lowerdir to guest /run/kata-containers/shared/images/<cid>/lowerdir
	if err := virtiofsDaemon.Mount(*mountOpt); err != nil {
		return nil, err
	}
	rootfs := &grpc.Storage{}
	containerShareDir := filepath.Join(getMountPath(f.sandbox.ID()), c.id)

	// mkdir rootfs, guest at /run/kata-containers/shared/containers/<cid>/rootfs
	rootfsDir := filepath.Join(containerShareDir, c.rootfsSuffix)
	if err := os.MkdirAll(rootfsDir, DirMode); err != nil {
		return nil, err
	}

	// bindmount snapshot dir which snapshotter allocated
	// to guest /run/kata-containers/shared/containers/<cid>/snapshotdir
	snapshotShareDir := filepath.Join(containerShareDir, snapshotDir)
	if err := bindMount(ctx, extraOption.Snapshotdir, snapshotShareDir, true, "slave"); err != nil {
		return nil, err
	}

	// so rootfs = overlay(upperdir, workerdir, lowerdir)
	rootfs.MountPoint = rootfsGuestPath
	rootfs.Source = typeOverlayFS
	rootfs.Fstype = typeOverlayFS
	rootfs.Driver = kataOverlayDevType
	rootfs.Options = append(rootfs.Options, fmt.Sprintf("%s=%s", upperDir, filepath.Join(kataGuestSharedDir(), c.id, snapshotDir, "fs")))
	rootfs.Options = append(rootfs.Options, fmt.Sprintf("%s=%s", workDir, filepath.Join(kataGuestSharedDir(), c.id, snapshotDir, "work")))
	rootfs.Options = append(rootfs.Options, fmt.Sprintf("%s=%s", lowerDir, filepath.Join(kataGuestNydusImageDir(), c.id, lowerDir)))
	rootfs.Options = append(rootfs.Options, "index=off")
	f.Logger().Infof("Nydus rootfs info: %#v\n", rootfs)

	return &SharedFile{
		storage:   rootfs,
		guestPath: rootfsGuestPath,
	}, nil
}

// func (c *Container) shareRootfs(ctx context.Context) (*grpc.Storage, string, error) {
func (f *FilesystemShare) ShareRootFilesystem(ctx context.Context, c *Container) (*SharedFile, error) {
	if c.rootFs.Type == NydusRootFSType {
		return f.shareRootFilesystemWithNydus(ctx, c)
	}
	rootfsGuestPath := filepath.Join(kataGuestSharedDir(), c.id, c.rootfsSuffix)

	if c.state.Fstype != "" && c.state.BlockDeviceID != "" {
		// The rootfs storage volume represents the container rootfs
		// mount point inside the guest.
		// It can be a block based device (when using block based container
		// overlay on the host) mount or a 9pfs one (for all other overlay
		// implementations).
		rootfsStorage := &grpc.Storage{}

		// This is a block based device rootfs.
		device := f.sandbox.devManager.GetDeviceByID(c.state.BlockDeviceID)
		if device == nil {
			f.Logger().WithField("device", c.state.BlockDeviceID).Error("failed to find device by id")
			return nil, fmt.Errorf("failed to find device by id %q", c.state.BlockDeviceID)
		}

		blockDrive, ok := device.GetDeviceInfo().(*config.BlockDrive)
		if !ok || blockDrive == nil {
			f.Logger().Error("malformed block drive")
			return nil, fmt.Errorf("malformed block drive")
		}
		switch {
		case f.sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioMmio:
			rootfsStorage.Driver = kataMmioBlkDevType
			rootfsStorage.Source = blockDrive.VirtPath
		case f.sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioBlockCCW:
			rootfsStorage.Driver = kataBlkCCWDevType
			rootfsStorage.Source = blockDrive.DevNo
		case f.sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioBlock:
			rootfsStorage.Driver = kataBlkDevType
			rootfsStorage.Source = blockDrive.PCIPath.String()
		case f.sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioSCSI:
			rootfsStorage.Driver = kataSCSIDevType
			rootfsStorage.Source = blockDrive.SCSIAddr
		default:
			return nil, fmt.Errorf("Unknown block device driver: %s", f.sandbox.config.HypervisorConfig.BlockDeviceDriver)
		}

		// We can't use filepath.Dir(rootfsGuestPath) (The rootfs parent) because
		// with block devices the rootfsSuffix may not be set.
		// So we have to build the bundle path explicitly.
		rootfsStorage.MountPoint = filepath.Join(kataGuestSharedDir(), c.id)
		rootfsStorage.Fstype = c.state.Fstype

		if c.state.Fstype == "xfs" {
			rootfsStorage.Options = []string{"nouuid"}
		}

		// Ensure container mount destination exists
		// TODO: remove dependency on shared fs path. shared fs is just one kind of storage source.
		// we should not always use shared fs path for all kinds of storage. Instead, all storage
		// should be bind mounted to a tmpfs path for containers to use.
		if err := os.MkdirAll(filepath.Join(getMountPath(f.sandbox.ID()), c.id, c.rootfsSuffix), DirMode); err != nil {
			return nil, err
		}

		return &SharedFile{
			storage:   rootfsStorage,
			guestPath: rootfsGuestPath,
		}, nil
	}

	// This is not a block based device rootfs. We are going to bind mount it into the shared drive
	// between the host and the guest.
	// With virtiofs/9pfs we don't need to ask the agent to mount the rootfs as the shared directory
	// (kataGuestSharedDir) is already mounted in the guest. We only need to mount the rootfs from
	// the host and it will show up in the guest.
	if err := bindMountContainerRootfs(ctx, getMountPath(f.sandbox.ID()), c.id, c.rootFs.Target, false); err != nil {
		return nil, err
	}

	return &SharedFile{
		storage:   nil,
		guestPath: rootfsGuestPath,
	}, nil
}

func (f *FilesystemShare) UnshareRootFilesystem(ctx context.Context, c *Container) error {
	if c.rootFs.Type == NydusRootFSType {
		if err2 := nydusContainerCleanup(ctx, getMountPath(c.sandbox.id), c); err2 != nil {
			f.Logger().WithError(err2).Error("rollback failed nydusContainerCleanup")
		}
	} else {
		if err := bindUnmountContainerRootfs(ctx, getMountPath(f.sandbox.ID()), c.id); err != nil {
			return err
		}
	}

	// Remove the shared directory for this container.
	shareDir := filepath.Join(getMountPath(f.sandbox.ID()), c.id)
	if err := syscall.Rmdir(shareDir); err != nil {
		f.Logger().WithError(err).WithField("share-dir", shareDir).Warn("Could not remove container share dir")
	}

	return nil

}
