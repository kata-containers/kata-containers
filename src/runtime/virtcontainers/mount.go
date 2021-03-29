// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	merr "github.com/hashicorp/go-multierror"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/sirupsen/logrus"
)

// DefaultShmSize is the default shm size to be used in case host
// IPC is used.
const DefaultShmSize = 65536 * 1024

// Sadly golang/sys doesn't have UmountNoFollow although it's there since Linux 2.6.34
const UmountNoFollow = 0x8

var rootfsDir = "rootfs"

var systemMountPrefixes = []string{"/proc", "/sys"}

func mountLogger() *logrus.Entry {
	return virtLog.WithField("subsystem", "mount")
}

var propagationTypes = map[string]uintptr{
	"shared":  syscall.MS_SHARED,
	"private": syscall.MS_PRIVATE,
	"slave":   syscall.MS_SLAVE,
	"ubind":   syscall.MS_UNBINDABLE,
}

func isSystemMount(m string) bool {
	for _, p := range systemMountPrefixes {
		if m == p || strings.HasPrefix(m, p+"/") {
			return true
		}
	}

	return false
}

func isHostDevice(m string) bool {
	if m == "/dev" {
		return true
	}

	if strings.HasPrefix(m, "/dev/") {
		// Check if regular file
		s, err := os.Stat(m)

		// This should not happen. In case file does not exist let the
		// error be handled by the agent, simply return false here.
		if err != nil {
			return false
		}

		if s.Mode().IsRegular() {
			return false
		}

		// This is not a regular file in /dev. It is either a
		// device file, directory or any other special file which is
		// specific to the host system.
		return true
	}

	return false
}

func major(dev uint64) int {
	return int((dev >> 8) & 0xfff)
}

func minor(dev uint64) int {
	return int((dev & 0xff) | ((dev >> 12) & 0xfff00))
}

type device struct {
	major      int
	minor      int
	mountPoint string
}

var errMountPointNotFound = errors.New("Mount point not found")

// getDeviceForPath gets the underlying device containing the file specified by path.
// The device type constitutes the major-minor number of the device and the dest mountPoint for the device
//
// eg. if /dev/sda1 is mounted on /a/b/c, a call to getDeviceForPath("/a/b/c/file") would return
//
//	device {
//		major : major(/dev/sda1)
//		minor : minor(/dev/sda1)
//		mountPoint: /a/b/c
//	}
//
//	if the path is a device path file such as /dev/sda1, it would return
//
//	device {
//		major : major(/dev/sda1)
//		minor : minor(/dev/sda1)
//		mountPoint:

func getDeviceForPath(path string) (device, error) {
	var devMajor int
	var devMinor int

	if path == "" {
		return device{}, fmt.Errorf("Path cannot be empty")
	}

	stat := syscall.Stat_t{}
	err := syscall.Stat(path, &stat)
	if err != nil {
		return device{}, err
	}

	if isHostDevice(path) {
		// stat.Rdev describes the device that this file (inode) represents.
		devMajor = major(stat.Rdev)
		devMinor = minor(stat.Rdev)

		return device{
			major:      devMajor,
			minor:      devMinor,
			mountPoint: "",
		}, nil
	}
	// stat.Dev points to the underlying device containing the file
	devMajor = major(stat.Dev)
	devMinor = minor(stat.Dev)

	path, err = filepath.Abs(path)
	if err != nil {
		return device{}, err
	}

	mountPoint := path

	if path == "/" {
		return device{
			major:      devMajor,
			minor:      devMinor,
			mountPoint: mountPoint,
		}, nil
	}

	// We get the mount point by recursively peforming stat on the path
	// The point where the device changes indicates the mountpoint
	for {
		if mountPoint == "/" {
			return device{}, errMountPointNotFound
		}

		parentStat := syscall.Stat_t{}
		parentDir := filepath.Dir(path)

		err := syscall.Lstat(parentDir, &parentStat)
		if err != nil {
			return device{}, err
		}

		if parentStat.Dev != stat.Dev {
			break
		}

		mountPoint = parentDir
		stat = parentStat
		path = parentDir
	}

	dev := device{
		major:      devMajor,
		minor:      devMinor,
		mountPoint: mountPoint,
	}

	return dev, nil
}

var blockFormatTemplate = "/sys/dev/block/%d:%d/dm"

var checkStorageDriver = isDeviceMapper

// isDeviceMapper checks if the device with the major and minor numbers is a devicemapper block device
func isDeviceMapper(major, minor int) (bool, error) {

	//Check if /sys/dev/block/${major}-${minor}/dm exists
	sysPath := fmt.Sprintf(blockFormatTemplate, major, minor)

	_, err := os.Stat(sysPath)
	if err == nil {
		return true, nil
	} else if os.IsNotExist(err) {
		return false, nil
	}

	return false, err
}

const mountPerm = os.FileMode(0755)

func evalMountPath(source, destination string) (string, string, error) {
	if source == "" {
		return "", "", fmt.Errorf("source must be specified")
	}
	if destination == "" {
		return "", "", fmt.Errorf("destination must be specified")
	}

	absSource, err := filepath.EvalSymlinks(source)
	if err != nil {
		return "", "", fmt.Errorf("Could not resolve symlink for source %v", source)
	}

	if err := ensureDestinationExists(absSource, destination); err != nil {
		return "", "", fmt.Errorf("Could not create destination mount point %v: %v", destination, err)
	}

	return absSource, destination, nil
}

// moveMount moves a mountpoint to another path with some bookkeeping:
// * evaluate all symlinks
// * ensure the source exists
// * recursively create the destination
func moveMount(ctx context.Context, source, destination string) error {
	span, _ := trace(ctx, "moveMount")
	defer span.End()

	source, destination, err := evalMountPath(source, destination)
	if err != nil {
		return err
	}

	return syscall.Mount(source, destination, "move", syscall.MS_MOVE, "")
}

// bindMount bind mounts a source in to a destination. This will
// do some bookkeeping:
// * evaluate all symlinks
// * ensure the source exists
// * recursively create the destination
// pgtypes stands for propagation types, which are shared, private, slave, and ubind.
func bindMount(ctx context.Context, source, destination string, readonly bool, pgtypes string) error {
	span, _ := trace(ctx, "bindMount")
	defer span.End()

	absSource, destination, err := evalMountPath(source, destination)
	if err != nil {
		return err
	}

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
	absSrc, err := filepath.EvalSymlinks(src)
	if err != nil {
		return fmt.Errorf("Could not resolve symlink for %s", src)
	}

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
	span, _ := trace(ctx, "bindMountContainerRootfs")
	defer span.End()

	rootfsDest := filepath.Join(shareDir, cid, rootfsDir)

	return bindMount(ctx, cRootFs, rootfsDest, readonly, "private")
}

// Mount describes a container mount.
type Mount struct {
	Source      string
	Destination string

	// Type specifies the type of filesystem to mount.
	Type string

	// Options list all the mount options of the filesystem.
	Options []string

	// HostPath used to store host side bind mount path
	HostPath string

	// ReadOnly specifies if the mount should be read only or not
	ReadOnly bool

	// BlockDeviceID represents block device that is attached to the
	// VM in case this mount is a block device file or a directory
	// backed by a block device.
	BlockDeviceID string
}

func isSymlink(path string) bool {
	stat, err := os.Stat(path)
	if err != nil {
		return false
	}
	return stat.Mode()&os.ModeSymlink != 0
}

func bindUnmountContainerRootfs(ctx context.Context, sharedDir, cID string) error {
	span, _ := trace(ctx, "bindUnmountContainerRootfs")
	defer span.End()

	rootfsDest := filepath.Join(sharedDir, cID, rootfsDir)
	if isSymlink(filepath.Join(sharedDir, cID)) || isSymlink(rootfsDest) {
		mountLogger().WithField("container", cID).Warnf("container dir is a symlink, malicious guest?")
		return nil
	}

	err := syscall.Unmount(rootfsDest, syscall.MNT_DETACH|UmountNoFollow)
	if err == syscall.ENOENT {
		mountLogger().WithError(err).WithField("rootfs-dir", rootfsDest).Warn()
		return nil
	}
	if err := syscall.Rmdir(rootfsDest); err != nil {
		mountLogger().WithError(err).WithField("rootfs-dir", rootfsDest).Warn("Could not remove container rootfs dir")
	}

	return err
}

func bindUnmountAllRootfs(ctx context.Context, sharedDir string, sandbox *Sandbox) error {
	span, ctx := trace(ctx, "bindUnmountAllRootfs")
	defer span.End()

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
			errors = merr.Append(errors, bindUnmountContainerRootfs(ctx, sharedDir, c.id))
		}
	}
	return errors.ErrorOrNil()
}

const (
	dockerVolumePrefix = "/var/lib/docker/volumes"
	dockerVolumeSuffix = "_data"
)

// IsDockerVolume returns true if the given source path is
// a docker volume.
// This uses a very specific path that is used by docker.
func IsDockerVolume(path string) bool {
	if strings.HasPrefix(path, dockerVolumePrefix) && filepath.Base(path) == dockerVolumeSuffix {
		return true
	}
	return false
}

const (
	// K8sEmptyDir is the k8s specific path for `empty-dir` volumes
	K8sEmptyDir = "kubernetes.io~empty-dir"
)

// IsEphemeralStorage returns true if the given path
// to the storage belongs to kubernetes ephemeral storage
//
// This method depends on a specific path used by k8s
// to detect if it's of type ephemeral. As of now,
// this is a very k8s specific solution that works
// but in future there should be a better way for this
// method to determine if the path is for ephemeral
// volume type
func IsEphemeralStorage(path string) bool {
	if !isEmptyDir(path) {
		return false
	}

	if _, fsType, _ := utils.GetDevicePathAndFsType(path); fsType == "tmpfs" {
		return true
	}

	return false
}

// Isk8sHostEmptyDir returns true if the given path
// to the storage belongs to kubernetes empty-dir of medium "default"
// i.e volumes that are directories on the host.
func Isk8sHostEmptyDir(path string) bool {
	if !isEmptyDir(path) {
		return false
	}

	if _, fsType, _ := utils.GetDevicePathAndFsType(path); fsType != "tmpfs" {
		return true
	}
	return false
}

func isEmptyDir(path string) bool {
	splitSourceSlice := strings.Split(path, "/")
	if len(splitSourceSlice) > 1 {
		storageType := splitSourceSlice[len(splitSourceSlice)-2]
		if storageType == K8sEmptyDir {
			return true
		}
	}
	return false
}
