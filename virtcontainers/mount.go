//
// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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
)

var rootfsDir = "rootfs"

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

// getVirtBlockDriveName returns the disk name format for virtio-blk
// Reference: https://github.com/torvalds/linux/blob/master/drivers/block/virtio_blk.c @c0aa3e0916d7e531e69b02e426f7162dfb1c6c0
func getVirtDriveName(index int) (string, error) {
	if index < 0 {
		return "", fmt.Errorf("Index cannot be negative for drive")
	}

	// Prefix used for virtio-block devices
	const prefix = "vd"

	//Refer to DISK_NAME_LEN: https://github.com/torvalds/linux/blob/08c521a2011ff492490aa9ed6cc574be4235ce2b/include/linux/genhd.h#L61
	diskNameLen := 32
	base := 26

	suffLen := diskNameLen - len(prefix)
	diskLetters := make([]byte, suffLen)

	var i int

	for i = 0; i < suffLen && index >= 0; i++ {
		letter := byte('a' + (index % base))
		diskLetters[i] = letter
		index = index/base - 1
	}

	if index >= 0 {
		return "", fmt.Errorf("Index not supported")
	}

	diskName := prefix + reverseString(string(diskLetters[:i]))
	return diskName, nil
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
func bindMountContainerRootfs(sharedDir, podID, cID, cRootFs string, readonly bool) error {
	rootfsDest := filepath.Join(sharedDir, podID, cID, rootfsDir)

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
}

func bindUnmountContainerRootfs(sharedDir, podID, cID string) error {
	rootfsDest := filepath.Join(sharedDir, podID, cID, rootfsDir)
	syscall.Unmount(rootfsDest, 0)

	return nil
}

func bindUnmountAllRootfs(sharedDir string, pod Pod) {
	for _, c := range pod.containers {
		c.unmountHostMounts()
		if c.state.Fstype == "" {
			// Need to check for error returned by this call.
			// See: https://github.com/containers/virtcontainers/issues/295
			bindUnmountContainerRootfs(sharedDir, pod.id, c.id)
		}
	}
}

const maxSCSIDevices = 65535

// getSCSIIdLun gets the SCSI id and lun, based on the index of the drive being inserted.
// qemu code suggests that scsi-id can take values from 0 to 255 inclusive, while lun can
// take values from 0 to 16383 inclusive. But lun values over 255 do not seem to follow
// consistent SCSI addressing. Hence we limit to 255.
func getSCSIIdLun(index int) (int, int, error) {
	if index < 0 {
		return -1, -1, fmt.Errorf("Index cannot be negative")
	}

	if index > maxSCSIDevices {
		return -1, -1, fmt.Errorf("Index cannot be greater than %d, maximum of %d devices are supported", maxSCSIDevices, maxSCSIDevices)
	}

	return index / 256, index % 256, nil
}

func getSCSIAddress(index int) (string, error) {
	scsiID, lun, err := getSCSIIdLun(index)
	if err != nil {
		return "", err
	}

	return fmt.Sprintf("%d:%d", scsiID, lun), nil
}
