// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"

	volume "github.com/kata-containers/kata-containers/src/runtime/pkg/direct-volume"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
)

// DefaultShmSize is the default shm size to be used in case host
// IPC is used.
const DefaultShmSize = 65536 * 1024

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

func isSystemMount(m string) bool {
	m = filepath.Clean(m)
	for _, p := range systemMountPrefixes {
		if m == p || strings.HasPrefix(m, p+"/") {
			return true
		}
	}

	return false
}

// isHostDevice returns whether the given path is a non-regular file
// under /dev (or /dev itself) on the host. If os.Stat fails on the
// file, it returns false plus the error from os.Stat.
func isHostDevice(m string) (bool, error) {
	m = filepath.Clean(m)
	if m == "/dev" {
		return true, nil
	}

	if strings.HasPrefix(m, "/dev/") {
		s, err := os.Stat(m)
		if err != nil {
			return false, err
		}

		if s.Mode().IsRegular() {
			return false, nil
		}

		// This is not a regular file in /dev. It is either a
		// device file, directory or any other special file which is
		// specific to the host system.
		return true, nil
	}

	return false, nil
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

	if isDevice, _ := isHostDevice(path); isDevice {
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

var blockFormatTemplate = "/sys/dev/block/%d:%d/"

var checkStorageDriver = isBlockDevice

// isBlockDevice checks if the device with the major and minor numbers is a block device
func isBlockDevice(major, minor int) (bool, error) {

	//Check if /sys/dev/block/${major}-${minor}/ exists
	sysPath := fmt.Sprintf(blockFormatTemplate, major, minor)

	_, err := os.Stat(sysPath)
	if err == nil {
		return true, nil
	} else if os.IsNotExist(err) {
		return false, nil
	} else {
		return false, err
	}
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
				return 0, err
			}
			// exceeded limit
			if inc == -1 {
				return -1, nil
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

func HasOption(options []string, option string) bool {
	for _, o := range options {
		if o == option {
			return true
		}
	}
	return false
}

func HasOptionPrefix(options []string, prefix string) bool {
	for _, o := range options {
		if strings.HasPrefix(o, prefix) {
			return true
		}
	}
	return false
}

// KubeletEmptyDirSubpathInfo describes a kubelet-managed subPath bind mount
// that ultimately points into an emptyDir volume on the host.
type KubeletEmptyDirSubpathInfo struct {
	// VolumeName is the k8s volume name under volume-subpaths/<vol>/...
	VolumeName string

	// SubPath is the path *inside* the emptyDir volume root.
	// Example: "app-logs" or "a/b/c". Empty means volume root.
	SubPath string

	// TargetPath is the resolved host path the bind mount points to
	// (e.g. /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~empty-dir/<vol>/<subpath>).
	TargetPath string
}

// indirection points for unit tests (override in *_test.go).
var (
	findMountSourceForMountPointFn = findMountSourceForMountPoint
	resolveMountSourcePathFn       = resolveMountSourcePath
)

// IsKubeletEmptyDirSubpath returns true when mountPoint is a kubelet volume-subpaths mount
// and the bind-mount ultimately targets an emptyDir volume path.
// It returns false when mountPoint is not such a mount.
func IsKubeletEmptyDirSubpath(mountPoint string) bool {
	info := GetKubeletEmptyDirSubpathInfo(mountPoint)
	return info != nil
}

// GetKubeletEmptyDirSubpathInfo returns nil, when mountPoint is not a kubelet emptyDir subPath mount.
func GetKubeletEmptyDirSubpathInfo(mountPoint string) *KubeletEmptyDirSubpathInfo {
	volName, ok := parseKubeletVolumeSubpathsMountPoint(mountPoint)
	if !ok {
		return nil
	}

	src, err := findMountSourceForMountPointFn(mountPoint)
	if err != nil {
		mountLogger().WithError(err).Debug("Failed to find mountSource for mountPoint")
		return nil
	}

	target, err := resolveMountSourcePathFn(src)
	if err != nil {
		mountLogger().WithError(err).Debug("Failed to resolve mountSource path")
		return nil
	}

	_, subRel, ok := splitKubeletEmptyDirTarget(target, volName)
	if !ok {
		// It's a kubelet subPath mount, but not for emptyDir (could be other volume types).
		return nil
	}

	subRel, err = sanitizeSubRel(subRel)
	if err != nil {
		mountLogger().WithError(err).Debug("Failed to sanitize subRel")
		return nil
	}

	return &KubeletEmptyDirSubpathInfo{
		VolumeName: volName,
		SubPath:    subRel,
		TargetPath: target,
	}
}

// parseKubeletVolumeSubpathsMountPoint extracts <vol-name> from
// .../pods/<uid>/volume-subpaths/<vol-name>/<container-name>/<index>
func parseKubeletVolumeSubpathsMountPoint(p string) (string, bool) {
	// Fast path to avoid any heavy work.
	if !strings.Contains(p, string(filepath.Separator)+"volume-subpaths"+string(filepath.Separator)) {
		return "", false
	}

	elems := splitPathElems(p)
	for i := 0; i < len(elems)-1; i++ {
		if elems[i] == "volume-subpaths" {
			vol := elems[i+1]
			if vol == "" {
				return "", false
			}
			return vol, true
		}
	}
	return "", false
}

// splitKubeletEmptyDirTarget checks whether targetAbs contains
// .../volumes/kubernetes.io~empty-dir/<volumeName>/...
// and if so returns (volumeRootAbs, subRel, true).
func splitKubeletEmptyDirTarget(targetAbs, volumeName string) (string, string, bool) {
	targetAbs = filepath.Clean(targetAbs)
	if !filepath.IsAbs(targetAbs) {
		// kubelet paths should be absolute; if not, treat as non-match.
		return "", "", false
	}

	elems := splitPathElems(targetAbs)
	for i := 0; i+2 < len(elems); i++ {
		if elems[i] == "volumes" && elems[i+1] == "kubernetes.io~empty-dir" && elems[i+2] == volumeName {
			volRoot := string(filepath.Separator) + filepath.Join(elems[:i+3]...)
			subRel := ""
			if i+3 < len(elems) {
				subRel = filepath.Join(elems[i+3:]...)
			}
			if subRel == "." {
				subRel = ""
			}
			return volRoot, subRel, true
		}
	}
	return "", "", false
}

func sanitizeSubRel(subRel string) (string, error) {
	if subRel == "" {
		return "", nil
	}
	cleaned := filepath.Clean(subRel)
	if cleaned == "." {
		return "", nil
	}
	if filepath.IsAbs(cleaned) {
		return "", fmt.Errorf("invalid subPath (absolute): %q", subRel)
	}
	if cleaned == ".." || strings.HasPrefix(cleaned, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("invalid subPath (traversal): %q", subRel)
	}
	return cleaned, nil
}

func splitPathElems(p string) []string {
	p = filepath.Clean(p)
	p = strings.TrimPrefix(p, string(filepath.Separator))
	if p == "" {
		return nil
	}
	return strings.Split(p, string(filepath.Separator))
}

// findMountSourceForMountPoint reads /proc/self/mountinfo and returns the mount source
func findMountSourceForMountPoint(mountPoint string) (string, error) {
	data, err := os.ReadFile("/proc/self/mountinfo")
	if err != nil {
		return "", err
	}
	return findMountSourceForMountPointFromData(mountPoint, data)
}

func findMountSourceForMountPointFromData(mountPoint string, data []byte) (string, error) {
	mp, err := filepath.EvalSymlinks(mountPoint)
	if err != nil {
		return "", fmt.Errorf("mountpoint %q EvalSymlinks err: %v", mountPoint, err)
	}

	sc := bufio.NewScanner(bytes.NewReader(data))
	for sc.Scan() {
		line := sc.Text()
		parts := strings.SplitN(line, " - ", 2)
		if len(parts) != 2 {
			continue
		}

		left := strings.Fields(parts[0])
		if len(left) < 5 {
			continue
		}

		mountpointField := unescapeMountinfoField(left[4])
		if filepath.Clean(mountpointField) != mp {
			continue
		}

		return unescapeMountinfoField(left[3]), nil
	}

	if err := sc.Err(); err != nil {
		return "", err
	}
	return "", fmt.Errorf("mountpoint %q not found in /proc/self/mountinfo", mp)
}

// unescapeMountinfoField converts mountinfo octal escapes (e.g. "\040") back to bytes.
func unescapeMountinfoField(s string) string {
	var b strings.Builder
	b.Grow(len(s))

	for i := 0; i < len(s); {
		if s[i] == '\\' && i+3 < len(s) {
			if v, err := strconv.ParseInt(s[i+1:i+4], 8, 8); err == nil {
				b.WriteByte(byte(v))
				i += 4
				continue
			}
		}
		b.WriteByte(s[i])
		i++
	}

	return b.String()
}

// resolveMountSourcePath resolves sources like /proc/<pid>/fd/<fd> to the real path.
func resolveMountSourcePath(source string) (string, error) {
	src := filepath.Clean(source)

	// kubelet subPath bind mounts typically use /proc/<kubeletpid>/fd/<fd> as source.
	if strings.HasPrefix(src, "/proc/") && strings.Contains(src, "/fd/") {
		target, err := os.Readlink(src)
		if err != nil {
			return "", err
		}
		// Sometimes readlink may include " (deleted)" suffix.
		target = strings.TrimSuffix(target, " (deleted)")
		return filepath.Clean(target), nil
	}

	return src, nil
}
