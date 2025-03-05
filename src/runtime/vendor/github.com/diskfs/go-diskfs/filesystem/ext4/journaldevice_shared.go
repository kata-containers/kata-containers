//go:build linux || unix || freebsd || netbsd || openbsd || darwin

package ext4

import (
	"fmt"
	"math"

	"golang.org/x/sys/unix"
)

func journalDevice(devicePath string) (deviceNumber uint32, err error) {
	// Use unix.Stat to get file status
	var stat unix.Stat_t
	err = unix.Stat(devicePath, &stat)
	if err != nil {
		return deviceNumber, err
	}

	// Extract major and minor device numbers
	//nolint:unconvert,nolintlint // lint stumbles on this, thinks it is an unnecessary conversion, which is true
	// on Linux, but not on others. So we will be explicit about this, and add a nolint flag
	major := unix.Major(uint64(stat.Rdev))
	//nolint:unconvert,nolintlint // lint stumbles on this, thinks it is an unnecessary conversion, which is true
	// on Linux, but not on others. So we will be explicit about this, and add a nolint flag
	minor := unix.Minor(uint64(stat.Rdev))

	// Combine major and minor numbers using unix.Mkdev
	// interestingly, this does not 100% align with what I read about linux mkdev works, which would be:
	// const minorbits = 20
	//    func mkdev(major, minor uint32) uint32 {
	//	     return (((major) << minorbits) | (minor))
	//    }
	// we leave this here for a future potential fix
	journalDeviceNumber64 := unix.Mkdev(major, minor)
	if journalDeviceNumber64 > math.MaxUint32 {
		return deviceNumber, fmt.Errorf("journal device number %d is too large", journalDeviceNumber64)
	}
	return uint32(journalDeviceNumber64), nil
}
