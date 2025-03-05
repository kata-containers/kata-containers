package diskfs

import (
	"fmt"
	"os"

	"golang.org/x/sys/unix"
)

// this constants should be part of "golang.org/x/sys/unix", but aren't, yet
const (
	DKIOCGETBLOCKSIZE         = 0x40046418
	DKIOCGETPHYSICALBLOCKSIZE = 0x4004644D
	DKIOCGETBLOCKCOUNT        = 0x40086419
)

// getBlockDeviceSize get the size of an opened block device in Bytes.
func getBlockDeviceSize(f *os.File) (int64, error) {
	fd := f.Fd()

	blockSize, err := unix.IoctlGetInt(int(fd), DKIOCGETBLOCKSIZE)
	if err != nil {
		return 0, fmt.Errorf("unable to get device logical sector size: %v", err)
	}

	blockCount, err := unix.IoctlGetInt(int(fd), DKIOCGETBLOCKCOUNT)
	if err != nil {
		return 0, fmt.Errorf("unable to get device block count: %v", err)
	}
	return int64(blockSize) * int64(blockCount), nil
}

// getSectorSizes get the logical and physical sector sizes for a block device
func getSectorSizes(f *os.File) (logicalSectorSize, physicalSectorSize int64, err error) {
	fd := f.Fd()

	logicalSectorSizeInt, err := unix.IoctlGetInt(int(fd), DKIOCGETBLOCKSIZE)
	if err != nil {
		return 0, 0, fmt.Errorf("unable to get device logical sector size: %v", err)
	}
	physicalSectorSizeInt, err := unix.IoctlGetInt(int(fd), DKIOCGETPHYSICALBLOCKSIZE)
	if err != nil {
		return 0, 0, fmt.Errorf("unable to get device physical sector size: %v", err)
	}
	return int64(logicalSectorSizeInt), int64(physicalSectorSizeInt), nil
}
