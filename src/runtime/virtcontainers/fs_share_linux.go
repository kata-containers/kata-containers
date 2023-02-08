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
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"syscall"

	"github.com/fsnotify/fsnotify"
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
	sandbox            *Sandbox
	watcher            *fsnotify.Watcher
	srcDstMap          map[string]string
	watcherDoneChannel chan bool
	sync.Mutex
	prepared bool
}

func NewFilesystemShare(s *Sandbox) (FilesystemSharer, error) {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, fmt.Errorf("Creating watcher returned error %w", err)
	}

	return &FilesystemShare{
		prepared:           false,
		sandbox:            s,
		watcherDoneChannel: make(chan bool),
		srcDstMap:          make(map[string]string),
		watcher:            watcher,
	}, nil
}

// Logger returns a logrus logger appropriate for logging filesystem sharing messages
func (f *FilesystemShare) Logger() *logrus.Entry {
	return virtLog.WithFields(logrus.Fields{
		"subsystem": "fs_share",
		"sandbox":   f.sandbox.ID(),
	})
}

func (f *FilesystemShare) prepareBindMounts(ctx context.Context) error {
	span, ctx := katatrace.Trace(ctx, f.Logger(), "prepareBindMounts", fsShareTracingTags)
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

		var ignored bool
		srcRoot := filepath.Clean(m.Source)

		walk := func(srcPath string, d fs.DirEntry, err error) error {

			if err != nil {
				return err
			}

			info, err := d.Info()
			if err != nil {
				return err
			}

			if !(info.Mode().IsRegular() || info.Mode().IsDir() || (info.Mode()&os.ModeSymlink) == os.ModeSymlink) {
				f.Logger().WithField("ignored-file", srcPath).Debug("Ignoring non-regular file as FS sharing not supported")
				if srcPath == srcRoot {
					// Ignore the mount if this is not a regular file (excludes socket, device, ...) as it cannot be handled by
					// a simple copy. But this should not be treated as an error, only as a limitation.
					ignored = true
					return filepath.SkipDir
				}
				return nil
			}

			dstPath := filepath.Join(guestPath, srcPath[len(srcRoot):])
			f.Logger().Infof("ShareFile: Copying file from src (%s) to dest (%s)", srcPath, dstPath)
			//TODO: Improve the agent protocol, to handle the case for existing symlink.
			// Currently for an existing symlink, this will fail with EEXIST.
			err = f.sandbox.agent.copyFile(ctx, srcPath, dstPath)
			if err != nil {
				f.Logger().WithError(err).Error("Failed to copy file")
				return err
			}

			// Add fsNotify watcher for volume mounts
			if strings.Contains(srcPath, "kubernetes.io~configmap") ||
				strings.Contains(srcPath, "kubernetes.io~secrets") ||
				strings.Contains(srcPath, "kubernetes.io~projected") ||
				strings.Contains(srcPath, "kubernetes.io~downward-api") {

				// fsNotify doesn't add watcher recursively.
				// So we need to add the watcher for directories under kubernetes.io~configmap, kubernetes.io~secrets,
				// kubernetes.io~downward-api and kubernetes.io~projected
				if info.Mode().IsDir() {
					// The cm dir is of the form /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~configmap/foo/{..data, key1, key2,...}
					// The secrets dir is of the form /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~secrets/foo/{..data, key1, key2,...}
					// The projected dir is of the form /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~projected/foo/{..data, key1, key2,...}
					// The downward-api dir is of the form /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~downward-api/foo/{..data, key1, key2,...}
					f.Logger().Infof("ShareFile: srcPath(%s) is a directory", srcPath)
					err := f.watchDir(srcPath)
					if err != nil {
						f.Logger().WithError(err).Error("Failed to watch directory")
						return err
					}
				} else {
					f.Logger().Infof("ShareFile: srcPath(%s) is not a directory", srcPath)
				}
				// Add the source and destination to the global map which will be used by the event loop
				// to copy the modified content to the destination
				f.Logger().Infof("ShareFile: Adding srcPath(%s) dstPath(%s) to srcDstMap", srcPath, dstPath)
				f.srcDstMap[srcPath] = dstPath

			}

			return nil
		}

		if err := filepath.WalkDir(srcRoot, walk); err != nil {
			c.Logger().WithField("failed-file", m.Source).Debugf("failed to copy file to sandbox: %v", err)
			return nil, err
		}
		if ignored {
			return nil, nil
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
			if f.sandbox.config.HypervisorType == AcrnHypervisor {
				rootfsStorage.Source = blockDrive.VirtPath
			} else {
				rootfsStorage.Source = blockDrive.PCIPath.String()
			}
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

func (f *FilesystemShare) watchDir(source string) error {

	// Add a watcher for the configmap, secrets, projected-volumes and downwar-api directories
	// /var/lib/kubelet/pods/<uid>/volumes/{kubernetes.io~configmap, kubernetes.io~secrets, kubernetes.io~downward-api, kubernetes.io~projected-volume}

	// Note: From fsNotify docs - https://pkg.go.dev/github.com/fsnotify/fsnotify
	// Watching individual files (rather than directories) is generally not
	// recommended as many tools update files atomically. Instead of "just"
	// writing to the file a temporary file will be written to first, and if
	// successful the temporary file is moved to to destination removing the
	// original, or some variant thereof. The watcher on the original file is
	// now lost, as it no longer exists.
	// Instead, watch the parent directory and use Event.Name to filter out files
	// you're not interested in.

	// Also fsNotify doesn't add watcher recursively. So we need to walk the root directory and add the required watches

	f.Logger().Infof("watchDir: Add fsnotify watcher for dir (%s)", source)
	watchList := f.watcher.WatchList()

	for _, v := range watchList {
		if v == source {
			f.Logger().Infof("watchDir: Watcher for dir(%s) is already present", source)
			return nil
		}
	}

	err := f.watcher.Add(source)
	if err != nil {
		f.Logger().WithError(err).Error("watchDir: Failed to add watcher to list")
		return err
	}

	return nil

}

func (f *FilesystemShare) StartFileEventWatcher(ctx context.Context) error {

	// Start event loop if watchList is not empty
	if (f.watcher == nil) || len(f.watcher.WatchList()) == 0 {
		f.Logger().Info("StartFileEventWatcher: No watches found, returning")
		return nil
	}
	// Regex for the temp directory with timestamp that is used to handle the updates by K8s
	var re = regexp.MustCompile(`(?m)\s*[0-9]{4}_[0-9]{2}_[0-9]{2}_[0-9]{2}_[0-9]{2}_[0-9]{2}.[0-9]{10}$`)

	f.Logger().Debugf("StartFileEventWatcher: srcDstMap dump %v", f.srcDstMap)

	// This is the event loop to watch for fsNotify events and copy the contents to the guest
	for {
		select {
		case event, ok := <-f.watcher.Events:
			if !ok {
				return fmt.Errorf("StartFileEventWatcher: Error in receiving events")
			}
			f.Logger().Infof("StartFileEventWatcher: got an event %s %s", event.Op, event.Name)
			if event.Op&fsnotify.Remove == fsnotify.Remove {
				// Ref: (kubernetes) pkg/volume/util/atomic_writer.go to understand the configmap/secrets update algo
				//
				// Write does an atomic projection of the given payload into the writer's target
				// directory.  Input paths must not begin with '..'.
				//
				// The Write algorithm is:
				//
				//  1.  The payload is validated; if the payload is invalid, the function returns
				//  2.  The current timestamped directory is detected by reading the data directory
				//      symlink
				//  3.  The old version of the volume is walked to determine whether any
				//      portion of the payload was deleted and is still present on disk.
				//  4.  The data in the current timestamped directory is compared to the projected
				//      data to determine if an update is required.
				//  5.  A new timestamped dir is created
				//  6.  The payload is written to the new timestamped directory
				//  7.  Symlinks and directory for new user-visible files are created (if needed).
				//
				//      For example, consider the files:
				//        <target-dir>/podName
				//        <target-dir>/user/labels
				//        <target-dir>/k8s/annotations
				//
				//      The user visible files are symbolic links into the internal data directory:
				//        <target-dir>/podName         -> ..data/podName
				//        <target-dir>/usr -> ..data/usr
				//        <target-dir>/k8s -> ..data/k8s
				//
				//      The data directory itself is a link to a timestamped directory with
				//      the real data:
				//        <target-dir>/..data          -> ..2016_02_01_15_04_05.12345678/
				//  8.  A symlink to the new timestamped directory ..data_tmp is created that will
				//      become the new data directory
				//  9.  The new data directory symlink is renamed to the data directory; rename is atomic
				// 10.  Old paths are removed from the user-visible portion of the target directory
				// 11.  The previous timestamped directory is removed, if it exists

				// In this code, we are relying on the REMOVE event to initate a copy of the updated data.
				// This ensures that the required data is updated and available for copying.
				// For REMOVE event, the event.Name (source) will be of the form:
				// /var/lib/kubelet/pods/<uid>/volumes/<k8s-special-dir>/foo/..2023_02_11_09_21_08.2202253910
				// For example, the event.Name (source) for configmap update will like this:
				// /var/lib/kubelet/pods/b44e3261-7cf0-48d3-83b4-6094bba95dc8/volumes/kubernetes.io~configmap/foo/..2023_02_11_09_21_08.2202253910

				source := event.Name
				f.Logger().Infof("StartFileEventWatcher: source for the event: %s", source)
				if re.FindString(source) != "" {
					// This block will be entered when the timestamped directory is removed.
					// This also indicates that foo/..data contains the updated info

					volumeDir := filepath.Dir(source)
					f.Logger().Infof("StartFileEventWatcher: volumeDir (%s)", volumeDir)
					// eg. volumDir = /var/lib/kubelet/pods/b44e3261-7cf0-48d3-83b4-6094bba95dc8/volumes/kubernetes.io~configmap/foo

					dataDir := filepath.Join(volumeDir, "..data")
					f.Logger().Infof("StartFileEventWatcher: dataDir (%s)", dataDir)
					// eg. dataDir = /var/lib/kubelet/pods/b44e3261-7cf0-48d3-83b4-6094bba95dc8/volumes/kubernetes.io~configmap/foo/..data

					destination := f.srcDstMap[dataDir]
					f.Logger().Infof("StartFileEventWatcher: Copy file from src (%s) to dst (%s)", dataDir, destination)
					err := f.copyFilesFromDataDir(dataDir, destination)
					if err != nil {
						f.Logger().Infof("StartFileEventWatcher: got an error (%v) when copying file from src (%s) to dst (%s)", err, dataDir, destination)
						return err
					}
				}
			}
		case err, ok := <-f.watcher.Errors:
			if !ok {
				return fmt.Errorf("StartFileEventWatcher: Error (%v) in receiving error events", err)
			}
			f.Logger().Infof("StartFileEventWatcher: got an error event (%v)", err)
			return err
		case <-f.watcherDoneChannel:
			f.Logger().Info("StartFileEventWatcher: watcher closed")
			f.watcher.Close()
			return nil
		}
	}
}

func (f *FilesystemShare) copyFilesFromDataDir(src, dst string) error {

	// The src is a symlink and is of the following form:
	// /var/lib/kubelet/pods/<uid>/volumes/<k8s-special-dir>/foo/..data
	// eg, for configmap, src = /var/lib/kubelet/pods/b44e3261-7cf0-48d3-83b4-6094bba95dc8/volumes/kubernetes.io~configmap/foo/..data
	// The dst is of the following form:
	// /run/kata-containers/shared/containers/<cid>-<volume>/..data
	// eg. dst = /run/kata-containers/shared/containers/e70739a6cc38daf15de916b4d22aad035d42bc977024f2c8cae6b0b607251d44-39407b03e4b448f1-config-volume/..data

	// Get the symlink target
	// eg. srcdir = ..2023_02_09_06_40_51.2326009790
	srcdir, err := os.Readlink(src)
	if err != nil {
		f.Logger().Infof("copyFilesFromDataDir: Reading data symlink returned error (%v)", err)
		return err
	}

	// Get the base directory path of src
	volumeDir := filepath.Dir(src)
	// eg. volumeDir = /var/lib/kubelet/pods/b44e3261-7cf0-48d3-83b4-6094bba95dc8/volumes/kubernetes.io~configmap/foo

	dataDir := filepath.Join(volumeDir, srcdir)
	// eg. dataDir = /var/lib/kubelet/pods/b44e3261-7cf0-48d3-83b4-6094bba95dc8/volumes/kubernetes.io~configmap/foo/..2023_02_09_06_40_51.2326009790

	f.Logger().Infof("copyFilesFromDataDir: full path to data symlink (%s)", dataDir)

	// Using WalkDir is more efficient than Walk
	err = filepath.WalkDir(dataDir,
		func(path string, d fs.DirEntry, err error) error {
			if err != nil {
				f.Logger().Infof("copyFilesFromDataDir: Error in file walk %v", err)
				return err
			}

			// eg. path = /var/lib/kubelet/pods/b44e3261-7cf0-48d3-83b4-6094bba95dc8/volumes/kubernetes.io~configmap/foo/..2023_02_09_06_40_51.2326009790/{key1, key2, ...}
			f.Logger().Infof("copyFilesFromDataDir: path (%s)", path)
			if !d.IsDir() {
				// Using filePath.Rel to handle these cases
				// /var/lib/kubelet/pods/2481b69e-9ac8-475a-9e11-88af1daca60e/volumes/kubernetes.io~projected/all-in-one/..2023_02_13_12_35_49.1380323032/config-dir1/config.file1
				// /var/lib/kubelet/pods/2481b69e-9ac8-475a-9e11-88af1daca60e/volumes/kubernetes.io~projected/all-in-one/..2023_02_13_12_35_49.1380323032/config.file2
				rel, err := filepath.Rel(dataDir, path)
				if err != nil {
					f.Logger().Infof("copyFilesFromDataDir: Unable to get relative path")
					return err
				}
				f.Logger().Debugf("copyFilesFromDataDir: dataDir(%s), path(%s), rel(%s)", dataDir, path, rel)
				// Form the destination path in the guest
				dstFile := filepath.Join(dst, rel)
				f.Logger().Infof("copyFilesFromDataDir: Copying file %s to dst %s", path, dstFile)
				err = f.sandbox.agent.copyFile(context.Background(), path, dstFile)
				if err != nil {
					f.Logger().Infof("copyFilesFromDataDir: Error in copying file %v", err)
					return err
				}
				f.Logger().Infof("copyFilesFromDataDir: Successfully copied file (%s)", path)
			}
			return nil
		})

	if err != nil {
		f.Logger().Infof("copyFilesFromDataDir: Error in filepath.WalkDir (%v)", err)
		return err
	}

	f.Logger().Infof("copyFilesFromDataDir: Done")
	return nil
}

func (f *FilesystemShare) StopFileEventWatcher(ctx context.Context) {

	f.Logger().Info("StopFileEventWatcher: Closing watcher")
	close(f.watcherDoneChannel)

}
