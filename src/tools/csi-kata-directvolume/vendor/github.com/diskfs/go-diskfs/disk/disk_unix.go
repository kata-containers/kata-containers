//go:build aix || darwin || dragonfly || freebsd || linux || netbsd || openbsd || solaris
// +build aix darwin dragonfly freebsd linux netbsd openbsd solaris

package disk

import (
	"fmt"

	"golang.org/x/sys/unix"
)

const (
	blkrrpart = 0x125f
)

// ReReadPartitionTable forces the kernel to re-read the partition table
// on the disk.
//
// It is done via an ioctl call with request as BLKRRPART.
func (d *Disk) ReReadPartitionTable() error {
	fd := d.File.Fd()
	_, err := unix.IoctlGetInt(int(fd), blkrrpart)
	if err != nil {
		return fmt.Errorf("unable to re-read partition table: %v", err)
	}
	return nil
}
