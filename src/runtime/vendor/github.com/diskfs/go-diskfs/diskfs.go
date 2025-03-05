// Package diskfs implements methods for creating and manipulating disks and filesystems
//
// methods for creating and manipulating disks and filesystems, whether block devices
// in /dev or direct disk images. This does **not**
// mount any disks or filesystems, neither directly locally nor via a VM. Instead, it manipulates the
// bytes directly.
//
// This is not intended as a replacement for operating system filesystem and disk drivers. Instead,
// it is intended to make it easy to work with partitions, partition tables and filesystems directly
// without requiring operating system mounts.
package diskfs

import (
	"errors"
	"fmt"
	"os"

	log "github.com/sirupsen/logrus"

	"github.com/diskfs/go-diskfs/backend"
	"github.com/diskfs/go-diskfs/backend/file"
	"github.com/diskfs/go-diskfs/disk"
)

// when we use a disk image with a GPT, we cannot get the logical sector size from the disk via the kernel
//
//	so we use the default sector size of 512, per Rod Smith
const (
	defaultBlocksize int = 512
	// firstblock                       = 2048
	// blksszGet                        = 0x1268
	// blkpbszGet                       = 0x127b
)

// OpenModeOption represents file open modes
type OpenModeOption int

const (
	// ReadOnly open file in read only mode
	ReadOnly OpenModeOption = iota
	// ReadWriteExclusive open file in read-write exclusive mode
	ReadWriteExclusive
	// ReadWrite open file in read-write mode
	ReadWrite
)

// OpenModeOption.String()
func (m OpenModeOption) String() string {
	switch m {
	case ReadOnly:
		return "read-only"
	case ReadWriteExclusive:
		return "read-write exclusive"
	case ReadWrite:
		return "read-write"
	default:
		return "unknown"
	}
}

var openModeOptions = map[OpenModeOption]int{
	ReadOnly:           os.O_RDONLY,
	ReadWriteExclusive: os.O_RDWR | os.O_EXCL,
	ReadWrite:          os.O_RDWR,
}

// SectorSize represents the sector size to use
type SectorSize int

const (
	// SectorSizeDefault default behavior, defaulting to defaultBlocksize
	SectorSizeDefault SectorSize = 0
	// SectorSize512 override sector size to 512
	SectorSize512 SectorSize = 512
	// SectorSize4k override sector size to 4k
	SectorSize4k SectorSize = 4096
)

func writableMode(mode OpenModeOption) bool {
	m, ok := openModeOptions[mode]
	if ok {
		if m&os.O_RDWR != 0 || m&os.O_WRONLY != 0 {
			return true
		}
	}

	return false
}

func initDisk(b backend.Storage, sectorSize SectorSize) (*disk.Disk, error) {
	log.Debug("initDisk(): start")

	var (
		lblksize = int64(defaultBlocksize)
		pblksize = int64(defaultBlocksize)
	)

	if sectorSize != SectorSizeDefault {
		lblksize = int64(sectorSize)
		pblksize = int64(sectorSize)
	}

	// get device information
	devInfo, err := b.Stat()
	if err != nil {
		return nil, fmt.Errorf("could not get info for device %s: %v", devInfo.Name(), err)
	}

	newDisk := &disk.Disk{
		Backend:           b,
		Size:              devInfo.Size(),
		LogicalBlocksize:  lblksize,
		PhysicalBlocksize: pblksize,
		DefaultBlocks:     true,
	}

	mode := devInfo.Mode()
	switch {
	case mode.IsRegular():
		log.Debug("initDisk(): regular file")
		if newDisk.Size <= 0 {
			return nil, fmt.Errorf("could not get file size for device %s", devInfo.Name())
		}
	case mode&os.ModeDevice != 0:
		log.Debug("initDisk(): block device")
		osFile, err := newDisk.Backend.Sys()
		if err != nil {
			return nil, backend.ErrNotSuitable
		}

		if size, err := getBlockDeviceSize(osFile); err != nil {
			return nil, fmt.Errorf("error getting block device %s size: %s", devInfo.Name(), err)
		} else {
			newDisk.Size = size
		}

		if lblksize, pblksize, err = getSectorSizes(osFile); err != nil {
			return nil, fmt.Errorf("unable to get block sizes for device %s: %v", devInfo.Name(), err)
		} else {
			log.Debugf("initDisk(): logical block size %d, physical block size %d", lblksize, pblksize)

			newDisk.LogicalBlocksize = lblksize
			newDisk.PhysicalBlocksize = pblksize
			newDisk.DefaultBlocks = false
		}

	default:
		return nil, fmt.Errorf("device %s is neither a block device nor a regular file", devInfo.Name())
	}

	// how many good blocks do we have?
	//    var goodBlocks, orphanedBlocks int
	//    goodBlocks = size / lblksize

	// try to initialize the partition table.
	//nolint:errcheck // we ignore errors, because it is perfectly fine to open a disk and use it before it has a
	// partition table. This is solely a convenience.
	newDisk.GetPartitionTable()

	return newDisk, nil
}

func checkDevice(device string) error {
	if device == "" {
		return errors.New("must pass device name")
	}
	if _, err := os.Stat(device); os.IsNotExist(err) {
		return fmt.Errorf("provided device %s does not exist", device)
	}

	return nil
}

type openOpts struct {
	mode       OpenModeOption
	sectorSize SectorSize
}

func openOptsDefaults() *openOpts {
	return &openOpts{
		mode:       ReadWriteExclusive,
		sectorSize: SectorSizeDefault,
	}
}

// OpenOpt func that process Open options
type OpenOpt func(o *openOpts) error

// WithOpenMode sets the opening mode to the requested mode of type OpenModeOption.
// Default is ReadWriteExclusive, i.e. os.O_RDWR | os.O_EXCL
func WithOpenMode(mode OpenModeOption) OpenOpt {
	return func(o *openOpts) error {
		o.mode = mode
		return nil
	}
}

// WithSectorSize opens the disk file or block device with the provided sector size.
// Defaults to the physical block size.
func WithSectorSize(sectorSize SectorSize) OpenOpt {
	return func(o *openOpts) error {
		o.sectorSize = sectorSize
		return nil
	}
}

// Might be deprecated in future: use <backend>.New + diskfs.OpenBackend
// Open a Disk from a path to a device in read-write exclusive mode
// Should pass a path to a block device e.g. /dev/sda or a path to a file /tmp/foo.img
// The provided device must exist at the time you call Open().
// Use OpenOpt to control options, such as sector size or open mode.
func Open(device string, opts ...OpenOpt) (*disk.Disk, error) {
	err := checkDevice(device)
	if err != nil {
		return nil, err
	}

	opt := openOptsDefaults()
	for _, o := range opts {
		if err := o(opt); err != nil {
			return nil, err
		}
	}

	m, ok := openModeOptions[opt.mode]
	if !ok {
		return nil, errors.New("unsupported file open mode")
	}

	f, err := os.OpenFile(device, m, 0o600)
	if err != nil {
		return nil, fmt.Errorf("could not open device %s with mode %v: %w", device, m, err)
	}

	// return our disk
	return initDisk(file.New(f, !writableMode(opt.mode)), opt.sectorSize)
}

// Open a Disk using provided fs.File to a device in read-only mode
// Use OpenOpt to control options, such as sector size or open mode.
func OpenBackend(b backend.Storage, opts ...OpenOpt) (*disk.Disk, error) {
	opt := &openOpts{
		mode:       ReadOnly,
		sectorSize: SectorSizeDefault,
	}

	for _, o := range opts {
		if err := o(opt); err != nil {
			return nil, err
		}
	}

	return initDisk(b, opt.sectorSize)
}

// Might be deprecated in future: use <backend>.CreateFromPath + diskfs.OpenBackend
// Create a Disk from a path to a device
// Should pass a path to a block device e.g. /dev/sda or a path to a file /tmp/foo.img
// The provided device must not exist at the time you call Create()
func Create(device string, size int64, sectorSize SectorSize) (*disk.Disk, error) {
	rawBackend, err := file.CreateFromPath(device, size)
	if err != nil {
		return nil, err
	}
	// return our disk
	return initDisk(rawBackend, sectorSize)
}
