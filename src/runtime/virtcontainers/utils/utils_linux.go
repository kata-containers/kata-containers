// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"bufio"
	"crypto/rand"
	"fmt"
	"io"
	"math/big"
	"os"
	"strings"
	"syscall"
	"time"
	"unsafe"

	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
)

var ioctlFunc = Ioctl

// maxUInt represents the maximum valid value for the context ID.
// The upper 32 bits of the CID are reserved and zeroed.
// See http://stefanha.github.io/virtio/
var maxUInt uint64 = 1<<32 - 1

func Ioctl(fd uintptr, request, data uintptr) error {
	if _, _, errno := unix.Syscall(unix.SYS_IOCTL, fd, request, data); errno != 0 {
		return os.NewSyscallError("ioctl", fmt.Errorf("%d", int(errno)))
	}

	return nil
}

// FindContextID finds a unique context ID by generating a random number between 3 and max unsigned int (maxUint).
// Using the ioctl VHOST_VSOCK_SET_GUEST_CID, findContextID asks to the kernel if the given
// context ID (N) is available, when the context ID is not available, incrementing by 1 findContextID
// iterates from N to maxUint until an available context ID is found, otherwise decrementing by 1
// findContextID iterates from N to 3 until an available context ID is found, this is the last chance
// to find a context ID available.
// On success vhost file and a context ID greater or equal than 3 are returned, otherwise 0 and an error are returned.
// vhost file can be used to send vhost file decriptor to QEMU. It's the caller's responsibility to
// close vhost file descriptor.
//
// Benefits of using random context IDs:
//   - Reduce the probability of a *DoS attack*, since other processes don't know whatis the initial context ID
//     used by findContextID to find a context ID available
func FindContextID() (*os.File, uint64, error) {
	// context IDs 0x0, 0x1 and 0x2 are reserved, 0x3 is the first context ID usable.
	var firstContextID uint64 = 0x3
	var contextID = firstContextID

	// Generate a random number
	n, err := rand.Int(rand.Reader, big.NewInt(int64(maxUInt)))
	if err == nil && n.Int64() >= int64(firstContextID) {
		contextID = uint64(n.Int64())
	}

	// Open vhost-vsock device to check what context ID is available.
	// This file descriptor holds/locks the context ID and it should be
	// inherited by QEMU process.
	vsockFd, err := os.OpenFile(VHostVSockDevicePath, syscall.O_RDWR, 0666)
	if err != nil {
		return nil, 0, err
	}

	// Looking for the first available context ID.
	for cid := contextID; cid <= maxUInt; cid++ {
		if err = ioctlFunc(vsockFd.Fd(), ioctlVhostVsockSetGuestCid, uintptr(unsafe.Pointer(&cid))); err == nil {
			return vsockFd, cid, nil
		}
	}

	// Last chance to get a free context ID.
	for cid := contextID - 1; cid >= firstContextID; cid-- {
		if err = ioctlFunc(vsockFd.Fd(), ioctlVhostVsockSetGuestCid, uintptr(unsafe.Pointer(&cid))); err == nil {
			return vsockFd, cid, nil
		}
	}

	vsockFd.Close()
	return nil, 0, fmt.Errorf("Could not get a unique context ID for the vsock : %s", err)
}

const (
	procMountsFile = "/proc/mounts"

	fieldsPerLine = 6
)

const (
	procDeviceIndex = iota
	procPathIndex
	procTypeIndex
	procOptionIndex
)

// GetDevicePathAndFsTypeOptions gets the device for the mount point, the file system type
// and mount options
func GetDevicePathAndFsTypeOptions(mountPoint string) (devicePath, fsType string, fsOptions []string, err error) {
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
			fsOptions = strings.Split(fields[procOptionIndex], ",")
			return
		}
	}
}

func waitProcessUsingPidfd(pid int, timeoutSecs uint, logger *logrus.Entry) (bool, error) {
	pidfd, err := unix.PidfdOpen(pid, 0)

	if err != nil {
		if err == unix.ESRCH {
			return false, nil
		}

		return true, err
	}

	defer unix.Close(pidfd)
	var n int

	maxDelay := time.Duration(timeoutSecs) * time.Second
	end := time.Now().Add(maxDelay)

	for {
		remaining := time.Until(end).Milliseconds()
		if remaining < 0 {
			remaining = 0
		}

		n, err = unix.Poll([]unix.PollFd{{Fd: int32(pidfd), Events: unix.POLLIN}}, int(remaining))
		if err != unix.EINTR {
			break
		}
	}

	if err != nil || n != 1 {
		logger.Warnf("process %v still running after waiting %ds", pid, timeoutSecs)
		return true, err
	}

	for {
		err := unix.Waitid(unix.P_PIDFD, pidfd, nil, unix.WEXITED, nil)
		if err == unix.EINVAL {
			err = unix.Waitid(unix.P_PID, pid, nil, unix.WEXITED, nil)
		}

		if err != unix.EINTR {
			break
		}
	}
	return false, nil
}

func waitForProcessCompletion(pid int, timeoutSecs uint, logger *logrus.Entry) bool {
	pidRunning, err := waitProcessUsingPidfd(pid, timeoutSecs, logger)

	if err == unix.ENOSYS {
		pidRunning = waitProcessUsingWaitLoop(pid, timeoutSecs, logger)
	}

	return pidRunning
}
