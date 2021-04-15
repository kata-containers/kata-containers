// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"crypto/rand"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"syscall"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"

	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
)

const cpBinaryName = "cp"

const fileMode0755 = os.FileMode(0755)

// MibToBytesShift the number to shift needed to convert MiB to Bytes
const MibToBytesShift = 20

// MaxSocketPathLen is the effective maximum Unix domain socket length.
//
// See unix(7).
const MaxSocketPathLen = 107

// VHostVSockDevicePath path to vhost-vsock device
var VHostVSockDevicePath = "/dev/vhost-vsock"

// sysModuleDir is the directory where system modules locate.
var sysModuleDir = "/sys/module"

// FileCopy copys files from srcPath to dstPath
func FileCopy(srcPath, dstPath string) error {
	if srcPath == "" {
		return fmt.Errorf("Source path cannot be empty")
	}

	if dstPath == "" {
		return fmt.Errorf("Destination path cannot be empty")
	}

	binPath, err := exec.LookPath(cpBinaryName)
	if err != nil {
		return err
	}

	cmd := exec.Command(binPath, srcPath, dstPath)

	return cmd.Run()
}

// GenerateRandomBytes generate n random bytes
func GenerateRandomBytes(n int) ([]byte, error) {
	b := make([]byte, n)
	_, err := rand.Read(b)

	if err != nil {
		return nil, err
	}

	return b, nil
}

// ReverseString reverses whole string
func ReverseString(s string) string {
	r := []rune(s)

	length := len(r)
	for i, j := 0, length-1; i < length/2; i, j = i+1, j-1 {
		r[i], r[j] = r[j], r[i]
	}

	return string(r)
}

// CleanupFds closed bundles of open fds in batch
func CleanupFds(fds []*os.File, numFds int) {
	maxFds := len(fds)

	if numFds < maxFds {
		maxFds = numFds
	}

	for i := 0; i < maxFds; i++ {
		_ = fds[i].Close()
	}
}

// WriteToFile opens a file in write only mode and writes bytes to it
func WriteToFile(path string, data []byte) error {
	f, err := os.OpenFile(path, os.O_WRONLY, fileMode0755)
	if err != nil {
		return err
	}

	defer f.Close()

	if _, err := f.Write(data); err != nil {
		return err
	}

	return nil
}

//CalculateMilliCPUs converts CPU quota and period to milli-CPUs
func CalculateMilliCPUs(quota int64, period uint64) uint32 {

	// If quota is -1, it means the CPU resource request is
	// unconstrained.  In that case, we don't currently assign
	// additional CPUs.
	if quota >= 0 && period != 0 {
		return uint32((uint64(quota) * 1000) / period)
	}

	return 0
}

//CalculateVCpusFromMilliCpus converts from mCPU to CPU, taking the ceiling
// value when necessary
func CalculateVCpusFromMilliCpus(mCPU uint32) uint32 {
	return (mCPU + 999) / 1000
}

// ConstraintsToVCPUs converts CPU quota and period to vCPUs
func ConstraintsToVCPUs(quota int64, period uint64) uint {
	if quota != 0 && period != 0 {
		// Use some math magic to round up to the nearest whole vCPU
		// (that is, a partial part of a quota request ends up assigning
		// a whole vCPU, for instance, a request of 1.5 'cpu quotas'
		// will give 2 vCPUs).
		// This also has the side effect that we will always allocate
		// at least 1 vCPU.
		return uint((uint64(quota) + (period - 1)) / period)
	}

	return 0
}

// GetVirtDriveName returns the disk name format for virtio-blk
// Reference: https://github.com/torvalds/linux/blob/master/drivers/block/virtio_blk.c @c0aa3e0916d7e531e69b02e426f7162dfb1c6c0
func GetVirtDriveName(index int) (string, error) {
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

	diskName := prefix + ReverseString(string(diskLetters[:i]))
	return diskName, nil
}

const maxSCSIDevices = 65535

// GetSCSIIdLun gets the SCSI id and lun, based on the index of the drive being inserted.
// qemu code suggests that scsi-id can take values from 0 to 255 inclusive, while lun can
// take values from 0 to 16383 inclusive. But lun values over 255 do not seem to follow
// consistent SCSI addressing. Hence we limit to 255.
func GetSCSIIdLun(index int) (int, int, error) {
	if index < 0 {
		return -1, -1, fmt.Errorf("Index cannot be negative")
	}

	if index > maxSCSIDevices {
		return -1, -1, fmt.Errorf("Index cannot be greater than %d, maximum of %d devices are supported", maxSCSIDevices, maxSCSIDevices)
	}

	return index / 256, index % 256, nil
}

// GetSCSIAddress gets scsiID and lun from index, and combined them into a scsi ID
func GetSCSIAddress(index int) (string, error) {
	scsiID, lun, err := GetSCSIIdLun(index)
	if err != nil {
		return "", err
	}

	return fmt.Sprintf("%d:%d", scsiID, lun), nil
}

// MakeNameID is generic function for creating a named-id for passing on the hypervisor commandline
func MakeNameID(namedType, id string, maxLen int) string {
	nameID := fmt.Sprintf("%s-%s", namedType, id)
	if len(nameID) > maxLen {
		nameID = nameID[:maxLen]
	}

	return nameID
}

// BuildSocketPath concatenates the provided elements into a path and returns
// it. If the resulting path is longer than the maximum permitted socket path
// on Linux, it will return an error.
func BuildSocketPath(elements ...string) (string, error) {
	result := filepath.Join(elements...)

	if result == "" {
		return "", errors.New("empty path")
	}

	l := len(result)

	if l > MaxSocketPathLen {
		return "", fmt.Errorf("path too long (got %v, max %v): %s", l, MaxSocketPathLen, result)
	}

	return result, nil
}

// SupportsVsocks returns true if vsocks are supported, otherwise false
func SupportsVsocks() (bool, error) {
	if _, err := os.Stat(VHostVSockDevicePath); err != nil {
		return false, fmt.Errorf("host system doesn't support vsock: %v", err)
	}

	return true, nil
}

// SupportsIfb returns true if ifb are supported, otherwise false
func SupportsIfb() (bool, error) {
	ifbModule := "ifb"
	// First, check to see if the ifb module is already loaded
	path := filepath.Join(sysModuleDir, ifbModule)
	if _, err := os.Stat(path); err == nil {
		return true, nil
	}

	// Try to load the ifb module.
	// When inserting the ifb module, tell it the number of virtual interfaces you need, here, it's zero.
	// The default is 2.
	cmd := exec.Command("modprobe", ifbModule, "numifbs=0")
	if output, err := cmd.CombinedOutput(); err != nil {
		return false, fmt.Errorf("modprobe insert ifb module failed: %s", string(output))
	}
	return true, nil
}

// StartCmd pointer to a function to start a command.
// Defined this way to allow mock testing.
var StartCmd = func(c *exec.Cmd) error {
	return c.Start()
}

// AlignMem align memory provided to a block size
func (m MemUnit) AlignMem(blockSize MemUnit) MemUnit {
	memSize := m
	if m < blockSize {
		memSize = blockSize

	}

	remainder := memSize % blockSize

	if remainder != 0 {
		// Align memory to memoryBlockSizeMB
		memSize += blockSize - remainder

	}
	return memSize
}

type MemUnit uint64

func (m MemUnit) ToMiB() uint64 {
	return m.ToBytes() / (1 * MiB).ToBytes()
}

func (m MemUnit) ToBytes() uint64 {
	return uint64(m)
}

const (
	Byte MemUnit = 1
	KiB          = Byte << 10
	MiB          = KiB << 10
	GiB          = MiB << 10
)

func ConvertNetlinkFamily(netlinkFamily int32) pbTypes.IPFamily {
	switch netlinkFamily {
	case netlink.FAMILY_V6:
		return pbTypes.IPFamily_v6
	case netlink.FAMILY_V4:
		fallthrough
	default:
		return pbTypes.IPFamily_v4
	}
}

// WaitLocalProcess waits for the specified process for up to timeoutSecs seconds.
//
// Notes:
//
// - If the initial signal is zero, the specified process is assumed to be
//   attempting to stop itself.
// - If the initial signal is not zero, it will be sent to the process before
//   checking if it is running.
// - If the process has not ended after the timeout value, it will be forcibly killed.
func WaitLocalProcess(pid int, timeoutSecs uint, initialSignal syscall.Signal, logger *logrus.Entry) error {
	var err error

	// Don't support process groups
	if pid <= 0 {
		return errors.New("can only wait for a single process")
	}

	if initialSignal != syscall.Signal(0) {
		if err = syscall.Kill(pid, initialSignal); err != nil {
			if err == syscall.ESRCH {
				return nil
			}

			return fmt.Errorf("Failed to send initial signal %v to process %v: %v", initialSignal, pid, err)
		}
	}

	pidRunning := true

	secs := time.Duration(timeoutSecs)
	timeout := time.After(secs * time.Second)

	// Wait for the VM process to terminate
outer:
	for {
		select {
		case <-time.After(50 * time.Millisecond):
			// Check if the process is running periodically to avoid a busy loop

			var _status syscall.WaitStatus
			var _rusage syscall.Rusage
			var waitedPid int

			// "A watched pot never boils" and an unwaited-for process never appears to die!
			waitedPid, err = syscall.Wait4(pid, &_status, syscall.WNOHANG, &_rusage)

			if waitedPid == pid && err == nil {
				pidRunning = false
				break outer
			}

			if err = syscall.Kill(pid, syscall.Signal(0)); err != nil {
				pidRunning = false
				break outer
			}

			break

		case <-timeout:
			logger.Warnf("process %v still running after waiting %ds", pid, timeoutSecs)

			break outer
		}
	}

	if pidRunning {
		// Force process to die
		if err = syscall.Kill(pid, syscall.SIGKILL); err != nil {
			return fmt.Errorf("Failed to stop process %v: %s", pid, err)
		}
	}

	return nil
}
