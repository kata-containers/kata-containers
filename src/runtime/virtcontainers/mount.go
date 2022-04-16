// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	merr "github.com/hashicorp/go-multierror"
	volume "github.com/kata-containers/kata-containers/src/runtime/pkg/direct-volume"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	otelLabel "go.opentelemetry.io/otel/attribute"
)

// DefaultShmSize is the default shm size to be used in case host
// IPC is used.
const DefaultShmSize = 65536 * 1024

// Sadly golang/sys doesn't have UmountNoFollow although it's there since Linux 2.6.34
const UmountNoFollow = 0x8

const (
	rootfsDir   = "rootfs"
	lowerDir    = "lowerdir"
	upperDir    = "upperdir"
	workDir     = "workdir"
	snapshotDir = "snapshotdir"
)

var systemMountPrefixes = []string{"/proc", "/sys"}

// mountTracingTags defines tags for the trace span
var mountTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "mount",
}

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
	mountPoint string
	major      int
	minor      int
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
		devMajor = major(uint64(stat.Rdev))
		devMinor = minor(uint64(stat.Rdev))

		return device{
			major:      devMajor,
			minor:      devMinor,
			mountPoint: "",
		}, nil
	}
	// stat.Dev points to the underlying device containing the file
	devMajor = major(uint64(stat.Dev))
	devMinor = minor(uint64(stat.Dev))

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

	// We get the mount point by recursively performing stat on the path
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

// Mount describes a container mount.
// nolint: govet
type Mount struct {
	// Source is the source of the mount.
	Source string
	// Destination is the destination of the mount (within the container).
	Destination string

	// Type specifies the type of filesystem to mount.
	Type string

	// HostPath used to store host side bind mount path
	HostPath string

	// GuestDeviceMount represents the path within the VM that the device
	// is mounted. Only relevant for block devices. This is tracked in the event
	// runtime wants to query the agent for mount stats.
	GuestDeviceMount string

	// BlockDeviceID represents block device that is attached to the
	// VM in case this mount is a block device file or a directory
	// backed by a block device.
	BlockDeviceID string

	// Options list all the mount options of the filesystem.
	Options []string

	// ReadOnly specifies if the mount should be read only or not
	ReadOnly bool

	// FSGroup a group ID that the group ownership of the files for the mounted volume
	// will need to be changed when set.
	FSGroup *int

	// FSGroupChangePolicy specifies the policy that will be used when applying
	// group id ownership change for a volume.
	FSGroupChangePolicy volume.FSGroupChangePolicy
}

func isSymlink(path string) bool {
	stat, err := os.Stat(path)
	if err != nil {
		return false
	}
	return stat.Mode()&os.ModeSymlink != 0
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
			if c.rootFs.Type == NydusRootFSType {
				errors = merr.Append(errors, nydusContainerCleanup(ctx, sharedDir, c))
			} else {
				errors = merr.Append(errors, bindUnmountContainerRootfs(ctx, sharedDir, c.id))
			}
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
	K8sEmptyDir  = "kubernetes.io~empty-dir"
	K8sConfigMap = "kubernetes.io~configmap"
	K8sSecret    = "kubernetes.io~secret"
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

	if _, fsType, _, _ := utils.GetDevicePathAndFsTypeOptions(path); fsType == "tmpfs" {
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

	if _, fsType, _, _ := utils.GetDevicePathAndFsTypeOptions(path); fsType != "tmpfs" {
		return true
	}
	return false
}

func checkKubernetesVolume(path, volumeType string) bool {
	splitSourceSlice := strings.Split(path, "/")
	if len(splitSourceSlice) > 1 {
		storageType := splitSourceSlice[len(splitSourceSlice)-2]
		if storageType == volumeType {
			return true
		}
	}

	return false
}

func isEmptyDir(path string) bool {
	return checkKubernetesVolume(path, K8sEmptyDir)
}

func isConfigMap(path string) bool {
	return checkKubernetesVolume(path, K8sConfigMap)
}

func isSecret(path string) bool {
	return checkKubernetesVolume(path, K8sSecret)
}

// countFiles will return the number of files within a given path. If the total number of
// files observed is greater than limit, break and return -1
func countFiles(path string, limit int) (numFiles int, err error) {

	// First, Check to see if the path exists
	file, err := os.Stat(path)
	if os.IsNotExist(err) {
		return 0, err
	}

	// Special case if this is just a file, not a directory:
	if !file.IsDir() {
		return 1, nil
	}

	files, err := os.ReadDir(path)
	if err != nil {
		return 0, err
	}

	for _, file := range files {
		if file.IsDir() {
			inc, err := countFiles(filepath.Join(path, file.Name()), (limit - numFiles))
			if err != nil {
				return numFiles, err
			}
			numFiles = numFiles + inc
		} else {
			numFiles++
		}
		if numFiles > limit {
			return -1, nil
		}
	}
	return numFiles, nil
}

func isWatchableMount(path string) bool {
	if isSecret(path) || isConfigMap(path) {
		// we have a cap on number of FDs which can be present in mount
		// to determine if watchable. A similar Check exists within the agent,
		// which may or may not help handle case where extra files are added to
		// a mount after the fact
		count, _ := countFiles(path, 8)
		if count > 0 {
			return true
		}
	}

	return false
}
