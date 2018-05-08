// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
)

var rootfsDir = "rootfs"

var systemMountPrefixes = []string{"/proc", "/sys"}

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
//		manor : minor(/dev/sda1)
//		mountPoint: /a/b/c
//	}
func getDeviceForPath(path string) (device, error) {
	if path == "" {
		return device{}, fmt.Errorf("Path cannot be empty")
	}

	stat := syscall.Stat_t{}
	err := syscall.Stat(path, &stat)
	if err != nil {
		return device{}, err
	}

	// stat.Dev points to the underlying device containing the file
	major := major(stat.Dev)
	minor := minor(stat.Dev)

	path, err = filepath.Abs(path)
	if err != nil {
		return device{}, err
	}

	mountPoint := path

	if path == "/" {
		return device{
			major:      major,
			minor:      minor,
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
		major:      major,
		minor:      minor,
		mountPoint: mountPoint,
	}

	return dev, nil
}

const (
	procMountsFile = "/proc/mounts"

	fieldsPerLine = 6
)

const (
	procDeviceIndex = iota
	procPathIndex
	procTypeIndex
)

func getDevicePathAndFsType(mountPoint string) (devicePath, fsType string, err error) {
	if mountPoint == "" {
		err = fmt.Errorf("Mount point cannot be empty")
		return
	}

	var file *os.File

	file, err = os.Open(procMountsFile)
	if err != nil {
		return
	}

	defer file.Close()

	reader := bufio.NewReader(file)
	for {
		var line string

		line, err = reader.ReadString('\n')
		if err == io.EOF {
			err = fmt.Errorf("Mount %s not found", mountPoint)
			return
		}

		fields := strings.Fields(line)
		if len(fields) != fieldsPerLine {
			err = fmt.Errorf("Incorrect no of fields (expected %d, got %d)) :%s", fieldsPerLine, len(fields), line)
			return
		}

		if mountPoint == fields[procPathIndex] {
			devicePath = fields[procDeviceIndex]
			fsType = fields[procTypeIndex]
			return
		}
	}
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

// bindMount bind mounts a source in to a destination. This will
// do some bookkeeping:
// * evaluate all symlinks
// * ensure the source exists
// * recursively create the destination
func bindMount(source, destination string, readonly bool) error {
	if source == "" {
		return fmt.Errorf("source must be specified")
	}
	if destination == "" {
		return fmt.Errorf("destination must be specified")
	}

	absSource, err := filepath.EvalSymlinks(source)
	if err != nil {
		return fmt.Errorf("Could not resolve symlink for source %v", source)
	}

	if err := ensureDestinationExists(absSource, destination); err != nil {
		return fmt.Errorf("Could not create destination mount point %v: %v", destination, err)
	} else if err := syscall.Mount(absSource, destination, "bind", syscall.MS_BIND, ""); err != nil {
		return fmt.Errorf("Could not bind mount %v to %v: %v", absSource, destination, err)
	}

	// For readonly bind mounts, we need to remount with the readonly flag.
	// This is needed as only very recent versions of libmount/util-linux support "bind,ro"
	if readonly {
		return syscall.Mount(absSource, destination, "bind", uintptr(syscall.MS_BIND|syscall.MS_REMOUNT|syscall.MS_RDONLY), "")
	}

	return nil
}

// bindMountContainerRootfs bind mounts a container rootfs into a 9pfs shared
// directory between the guest and the host.
func bindMountContainerRootfs(sharedDir, sandboxID, cID, cRootFs string, readonly bool) error {
	rootfsDest := filepath.Join(sharedDir, sandboxID, cID, rootfsDir)

	return bindMount(cRootFs, rootfsDest, readonly)
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

	// BlockDevice represents block device that is attached to the
	// VM in case this mount is a block device file or a directory
	// backed by a block device.
	BlockDevice *drivers.BlockDevice
}

func bindUnmountContainerRootfs(sharedDir, sandboxID, cID string) error {
	rootfsDest := filepath.Join(sharedDir, sandboxID, cID, rootfsDir)
	syscall.Unmount(rootfsDest, 0)

	return nil
}

func bindUnmountAllRootfs(sharedDir string, sandbox *Sandbox) {
	for _, c := range sandbox.containers {
		c.unmountHostMounts()
		if c.state.Fstype == "" {
			// Need to check for error returned by this call.
			// See: https://github.com/containers/virtcontainers/issues/295
			bindUnmountContainerRootfs(sharedDir, sandbox.id, c.id)
		}
	}
}
