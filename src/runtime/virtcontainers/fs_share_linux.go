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
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// Splitting Regex pattern:
// Use regex for strict matching instead of strings.Contains
// match for kubernetes.io~configmap, kubernetes.io~secret, kubernetes.io~projected, kubernetes.io~downward-api
// as recommended in review comments for PR #7211

// Default K8S root directory
var defaultKubernetesRootDir = "/var/lib/kubelet"

// Example directory structure for the volume mounts.
// /var/lib/kubelet/pods/f51ae853-557e-4ce1-b60b-a1101b555612/volumes/kubernetes.io~configmap
// /var/lib/kubelet/pods/f51ae853-557e-4ce1-b60b-a1101b555612/volumes/kubernetes.io~secret
// /var/lib/kubelet/pods/f51ae853-557e-4ce1-b60b-a1101b555612/volumes/kubernetes.io~projected
// /var/lib/kubelet/pods/f51ae853-557e-4ce1-b60b-a1101b555612/volumes/kubernetes.io~downward-api
var configVolRegexString = "/pods/[a-fA-F0-9\\-]{36}/volumes/kubernetes\\.io~(configmap|secret|projected|downward-api)"

// Regex for the temp directory with timestamp that is used to handle the updates by K8s
// Examples
// /var/lib/kubelet/pods/e33907eb-54c7-4113-a3dc-447f247084cc/volumes/kubernetes.io~secret/foosecret/..2023_07_27_07_13_00.1257228
// /var/lib/kubelet/pods/e33907eb-54c7-4113-a3dc-447f247084cc/volumes/kubernetes.io~downward-api/fooinfo/..2023_07_27_07_13_00.3704578339
// The timestamp is of the format 2023_07_27_07_13_00.3704578339 or 2023_07_27_07_13_00.1257228
var timestampDirRegexString = ".*[0-9]{4}_[0-9]{2}_[0-9]{2}_[0-9]{2}_[0-9]{2}_[0-9]{2}.[0-9]+$"

func unmountNoFollow(path string) error {
	return syscall.Unmount(path, syscall.MNT_DETACH|UmountNoFollow)
}

// Resolve the K8S root dir if it is a symbolic link
func resolveRootDir() string {
	rootDir, err := os.Readlink(defaultKubernetesRootDir)
	if err != nil {
		// Use the default root dir in case of any errors resolving the root dir symlink
		return defaultKubernetesRootDir
	}
	// Make root dir an absolute path if needed
	if !filepath.IsAbs(rootDir) {
		rootDir, err = filepath.Abs(filepath.Join(filepath.Dir(defaultKubernetesRootDir), rootDir))
		if err != nil {
			// Use the default root dir in case of any errors resolving the root dir symlink
			return defaultKubernetesRootDir
		}
	}
	return rootDir
}

type FilesystemShare struct {
	sandbox *Sandbox
	watcher *fsnotify.Watcher
	// Regex to match directory structure for k8's volume mounts.
	configVolRegex *regexp.Regexp
	// Regex to match only the timestamped directory inside the k8's volume mount
	timestampDirRegex *regexp.Regexp
	// The same volume mount can be shared by multiple containers in the same sandbox (pod)
	srcDstMap            map[string][]string
	srcDstMapLock        sync.Mutex
	eventLoopStarted     bool
	eventLoopStartedLock sync.Mutex
	watcherDoneChannel   chan bool
	sync.Mutex
	prepared bool
}

func NewFilesystemShare(s *Sandbox) (FilesystemSharer, error) {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, fmt.Errorf("Creating watcher returned error %w", err)
	}

	kubernetesRootDir := resolveRootDir()
	configVolRegex := regexp.MustCompile("^" + kubernetesRootDir + configVolRegexString)
	timestampDirRegex := regexp.MustCompile("^" + kubernetesRootDir + configVolRegexString + timestampDirRegexString)

	return &FilesystemShare{
		prepared:           false,
		sandbox:            s,
		watcherDoneChannel: make(chan bool),
		srcDstMap:          make(map[string][]string),
		watcher:            watcher,
		configVolRegex:     configVolRegex,
		timestampDirRegex:  timestampDirRegex,
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
				f.Logger().WithField("ignored-file", srcPath).Debug("Ignoring file as FS sharing not supported")
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

			if f.configVolRegex.MatchString(srcPath) {
				// fsNotify doesn't add watcher recursively.
				// So we need to add the watcher for directories under kubernetes.io~configmap, kubernetes.io~secret,
				// kubernetes.io~downward-api and kubernetes.io~projected

				// Add watcher only to the timestamped directory containing secrets to prevent
				// multiple events received from also watching the parent directory.
				if info.Mode().IsDir() && f.timestampDirRegex.MatchString(srcPath) {
					// The cm dir is of the form /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~configmap/foo/{..data, key1, key2,...}
					// The secret dir is of the form /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~secret/foo/{..data, key1, key2,...}
					// The projected dir is of the form /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~projected/foo/{..data, key1, key2,...}
					// The downward-api dir is of the form /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~downward-api/foo/{..data, key1, key2,...}
					f.Logger().Infof("ShareFile: srcPath(%s) is a directory", srcPath)
					err := f.watchDir(srcPath)
					if err != nil {
						f.Logger().WithError(err).Error("Failed to watch directory")
						return err
					}
				} else {
					f.Logger().Infof("ShareFile: srcPath(%s) is not a timestamped directory", srcPath)
				}
				// Add the source and destination to the global map which will be used by the event loop
				// to copy the modified content to the destination
				f.Logger().Infof("ShareFile: Adding srcPath(%s) dstPath(%s) to srcDstMap", srcPath, dstPath)
				// Lock the map before adding the entry
				f.srcDstMapLock.Lock()
				defer f.srcDstMapLock.Unlock()
				f.srcDstMap[srcPath] = append(f.srcDstMap[srcPath], dstPath)
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
		containerStorages: []*grpc.Storage{rootfs},
		guestPath:         rootfsGuestPath,
	}, nil
}

// handleVirtualVolume processes all `io.katacontainers.volume=` messages in rootFs.Options,
// creating storage, and then aggregates all storages  into an array.
func handleVirtualVolume(c *Container) ([]*grpc.Storage, string, error) {
	var volumes []*grpc.Storage
	var volumeType string

	for _, o := range c.rootFs.Options {
		if strings.HasPrefix(o, VirtualVolumePrefix) {
			virtVolume, err := types.ParseKataVirtualVolume(strings.TrimPrefix(o, VirtualVolumePrefix))
			if err != nil {
				return nil, "", err
			}

			volumeType = virtVolume.VolumeType
			var vol *grpc.Storage
			if volumeType == types.KataVirtualVolumeImageGuestPullType {
				vol, err = handleVirtualVolumeStorageObject(c, "", virtVolume)
				if err != nil {
					return nil, "", err
				}
			}

			if vol != nil {
				volumes = append(volumes, vol)
			}
		}
	}

	return volumes, volumeType, nil
}

func (f *FilesystemShare) shareRootFilesystemWithVirtualVolume(ctx context.Context, c *Container) (*SharedFile, error) {
	guestPath := filepath.Join("/run/kata-containers/", c.id, c.rootfsSuffix)
	rootFsStorages, _, err := handleVirtualVolume(c)
	if err != nil {
		return nil, err
	}

	return &SharedFile{
		containerStorages: rootFsStorages,
		guestPath:         guestPath,
	}, nil
}

// func (c *Container) shareRootfs(ctx context.Context) (*grpc.Storage, string, error) {
func (f *FilesystemShare) ShareRootFilesystem(ctx context.Context, c *Container) (*SharedFile, error) {

	if HasOptionPrefix(c.rootFs.Options, VirtualVolumePrefix) {
		return f.shareRootFilesystemWithVirtualVolume(ctx, c)
	}

	if IsNydusRootFSType(c.rootFs.Type) {
		return f.shareRootFilesystemWithNydus(ctx, c)
	}
	rootfsGuestPath := filepath.Join(kataGuestSharedDir(), c.id, c.rootfsSuffix)

	if HasOptionPrefix(c.rootFs.Options, annotations.FileSystemLayer) {
		path := filepath.Join("/run/kata-containers", c.id, "rootfs")
		return &SharedFile{
			containerStorages: []*grpc.Storage{{
				MountPoint: path,
				Source:     "none",
				Fstype:     c.rootFs.Type,
				Driver:     kataOverlayDevType,
				Options:    c.rootFs.Options,
			}},
			guestPath: path,
		}, nil
	}

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
			containerStorages: []*grpc.Storage{rootfsStorage},
			guestPath:         rootfsGuestPath,
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
		containerStorages: nil,
		guestPath:         rootfsGuestPath,
	}, nil
}

func (f *FilesystemShare) UnshareRootFilesystem(ctx context.Context, c *Container) error {
	if IsNydusRootFSType(c.rootFs.Type) {
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

	// Add a watcher for the configmap, secret, projected-volumes and downwar-api directories
	// /var/lib/kubelet/pods/<uid>/volumes/{kubernetes.io~configmap, kubernetes.io~secret, kubernetes.io~downward-api, kubernetes.io~projected-volume}

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

	// Acquire lock and check if eventLoopStarted
	// If not started set the event loop started flag
	f.eventLoopStartedLock.Lock()

	// Check if the event loop is already started
	if f.eventLoopStarted {
		f.Logger().Info("StartFileEventWatcher: Event loop already started, returning")
		f.eventLoopStartedLock.Unlock()
		return nil
	}

	f.Logger().Infof("StartFileEventWatcher: starting the event loop")

	f.eventLoopStarted = true
	f.eventLoopStartedLock.Unlock()

	f.Logger().Debugf("StartFileEventWatcher: srcDstMap dump %v", f.srcDstMap)

	for {
		select {
		case event, ok := <-f.watcher.Events:
			if !ok {
				return fmt.Errorf("StartFileEventWatcher: watcher events channel closed")
			}
			f.Logger().Infof("StartFileEventWatcher: got an event %s %s", event.Op, event.Name)
			if event.Op&fsnotify.Remove == fsnotify.Remove {
				// Ref: (kubernetes) pkg/volume/util/atomic_writer.go to understand the configmap/secret update algo
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
				if f.timestampDirRegex.FindString(source) != "" {
					// This block will be entered when the timestamped directory is removed.
					// This also indicates that foo/..data contains the updated info

					volumeDir := filepath.Dir(source)
					f.Logger().Infof("StartFileEventWatcher: volumeDir (%s)", volumeDir)
					// eg. volumDir = /var/lib/kubelet/pods/b44e3261-7cf0-48d3-83b4-6094bba95dc8/volumes/kubernetes.io~configmap/foo

					dataDir := filepath.Join(volumeDir, "..data")
					f.Logger().Infof("StartFileEventWatcher: dataDir (%s)", dataDir)
					// eg. dataDir = /var/lib/kubelet/pods/b44e3261-7cf0-48d3-83b4-6094bba95dc8/volumes/kubernetes.io~configmap/foo/..data

					// Handle different destination for the same source
					// Acquire srcDstMapLock before reading srcDstMap
					f.srcDstMapLock.Lock()
					for _, destination := range f.srcDstMap[dataDir] {
						f.Logger().Infof("StartFileEventWatcher: Copy file from src (%s) to dst (%s)", dataDir, destination)
						// We explicitly ignore any errors here. Copy will continue for other files
						// Errors are logged in the copyFilesFromDataDir method
						_ = f.copyUpdatedFiles(dataDir, destination, source)
					}
					f.srcDstMapLock.Unlock()
				}
			}
		case err, ok := <-f.watcher.Errors:
			if !ok {
				return fmt.Errorf("StartFileEventWatcher: watcher error channel closed")
			}
			// We continue explicitly here to avoid exiting the watcher loop
			f.Logger().Infof("StartFileEventWatcher: got an error event (%v)", err)
			continue
		case <-f.watcherDoneChannel:
			f.Logger().Info("StartFileEventWatcher: watcher closed")
			f.watcher.Close()
			return nil
		}
	}
}

func (f *FilesystemShare) copyUpdatedFiles(src, dst, oldtsDir string) error {
	f.Logger().Infof("copyUpdatedFiles: Copy src:%s to dst:%s from old src:%s", src, dst, oldtsDir)

	// 1. Read the symlink and get the actual data directory
	// Get the symlink target
	// eg. srcdir = ..2023_02_09_06_40_51.2326009790
	srcnewtsdir, err := os.Readlink(src)
	if err != nil {
		f.Logger().WithError(err).Errorf("copyUpdatedFiles: Reading data symlink %s returned error", src)
		return err
	}

	// 2. Construct the path to new timestamped directory in host
	srcBasePath := filepath.Dir(src)
	srcNewTsPath := filepath.Join(srcBasePath, srcnewtsdir)

	// 3. Construct the path to copy new timestamped directory in guest
	dstBasePath := filepath.Dir(dst)
	dstNewTsPath := filepath.Join(dstBasePath, srcnewtsdir)

	// 4. Create a hashmap to add newly added secrets (not present in the old ts directory)
	// for creating user visible symlinks
	newSecrets := make(map[string]string)

	f.Logger().Infof("copyUpdatedFiles: new src dir: %s && new dst dir:%s", srcNewTsPath, dstNewTsPath)

	// 5. Copy all the files from the new timestamped directory to the guest
	walk := func(srcPath string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}

		info, err := d.Info()
		if err != nil {
			return err
		}
		dstPath := dstNewTsPath
		if !info.Mode().IsDir() {
			// Construct the path for the files to be copied to.
			dstPath = filepath.Join(dstPath, filepath.Base(srcPath))

			// Determine if this secret was present in the old timestamped directory.
			// If not, add it to the newSecrets map to create user visible symlinks.
			oldSecret := filepath.Join(oldtsDir, filepath.Base(srcPath))
			if _, ok := f.srcDstMap[oldSecret]; !ok {
				// these are symlinks to '..data' inside the k8's volume
				symlinkSrc := filepath.Join(filepath.Dir(srcNewTsPath), filepath.Base(srcPath))
				symlinkDst := filepath.Join(filepath.Dir(dstNewTsPath), filepath.Base(srcPath))
				newSecrets[symlinkSrc] = symlinkDst
			}
		}

		err = f.sandbox.agent.copyFile(context.Background(), srcPath, dstPath)
		if err != nil {
			f.Logger().WithError(err).Error("Failed to copy file")
			return err
		}

		// Create a new entry in the globalMap to be used in the event loop
		f.Logger().Infof("copyUpdatedFiles: Adding srcPath(%s) dstPath(%s) to srcDstMap", srcPath, dstPath)
		f.srcDstMap[srcPath] = append(f.srcDstMap[srcPath], dstPath)
		return nil
	}

	if err := filepath.WalkDir(srcNewTsPath, walk); err != nil {
		f.Logger().WithError(err).Error("copyUpdatedFiles: failed to copy files.")
		return err
	}

	// 6. Add watcher to the new timestamped directory in host
	err = f.watchDir(srcNewTsPath)
	if err != nil {
		f.Logger().WithError(err).Error("copyUpdatedFiles: Failed to add watcher on new ts source.")
		return err
	}

	// 7. Update the '..data' symlink to fix user visible files
	srcDataPath := filepath.Join(filepath.Dir(srcNewTsPath), "..data")
	dstDataPath := filepath.Join(filepath.Dir(dstNewTsPath), "..data")
	err = f.sandbox.agent.copyFile(context.Background(), srcDataPath, dstDataPath)
	if err != nil {
		f.Logger().WithError(err).Errorf("copyUpdatedFiles: Failed to update data symlink")
		return err
	}

	// 8. Create user visible symlinks for any newly created secrets
	// For existing secrets, the update to '..data' symlink above will fix the user visible files.
	// TODO: For deleted secrets, the existing symlink will point to non-existing entity after
	// update to '..data' symlink. Since there is NO DELETE-API in agent, the symlinks will exist
	for k, v := range newSecrets {
		err = f.sandbox.agent.copyFile(context.Background(), k, v)
		if err != nil {
			f.Logger().WithError(err).Error("copyUpdatedFiles: Failed to copy newly created secret")
			return err
		}
	}

	return nil
}

func (f *FilesystemShare) StopFileEventWatcher(ctx context.Context) {

	f.Logger().Info("StopFileEventWatcher: Closing watcher")
	close(f.watcherDoneChannel)

}
