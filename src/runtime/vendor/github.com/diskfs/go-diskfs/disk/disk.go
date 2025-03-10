// Package disk provides utilities for working directly with a disk
//
// Most of the provided functions are intelligent wrappers around implementations of
// github.com/diskfs/go-diskfs/partition and github.com/diskfs/go-diskfs/filesystem
package disk

import (
	"errors"
	"fmt"
	"io"

	"github.com/diskfs/go-diskfs/backend"
	"github.com/diskfs/go-diskfs/filesystem"
	"github.com/diskfs/go-diskfs/filesystem/ext4"
	"github.com/diskfs/go-diskfs/filesystem/fat32"
	"github.com/diskfs/go-diskfs/filesystem/iso9660"
	"github.com/diskfs/go-diskfs/filesystem/squashfs"
	"github.com/diskfs/go-diskfs/partition"
	log "github.com/sirupsen/logrus"
)

// Disk is a reference to a single disk block device or image that has been Create() or Open()
type Disk struct {
	Backend           backend.Storage
	Size              int64
	LogicalBlocksize  int64
	PhysicalBlocksize int64
	Table             partition.Table
	DefaultBlocks     bool
}

// Type represents the type of disk this is
type Type int

const (
	// File is a file-based disk image
	File Type = iota
	// Device is an OS-managed block device
	Device
)

// GetPartitionTable retrieves a PartitionTable for a Disk
//
// If the table is able to be retrieved from the disk, it is saved in the instance.
//
// returns an error if the Disk is invalid or does not exist, or the partition table is unknown
func (d *Disk) GetPartitionTable() (partition.Table, error) {
	t, err := partition.Read(d.Backend, int(d.LogicalBlocksize), int(d.PhysicalBlocksize))
	if err != nil {
		return nil, err
	}
	d.Table = t
	return t, nil
}

// Partition applies a partition.Table implementation to a Disk
//
// The Table can have zero, one or more Partitions, each of which is unique to its
// implementation. E.g. MBR partitions in mbr.Table look different from GPT partitions in gpt.Table
//
// Actual writing of the table is delegated to the individual implementation
func (d *Disk) Partition(table partition.Table) error {
	rwBackingFile, err := d.Backend.Writable()
	if err != nil {
		return err
	}

	// fill in the uuid
	err = table.Write(rwBackingFile, d.Size)
	if err != nil {
		return fmt.Errorf("failed to write partition table: %v", err)
	}
	d.Table = table

	return d.ReReadPartitionTable()
}

// WritePartitionContents writes the contents of an io.Reader to a given partition
//
// if successful, returns the number of bytes written
//
// returns an error if there was an error writing to the disk, reading from the reader, the table
// is invalid, or the partition is invalid
func (d *Disk) WritePartitionContents(part int, reader io.Reader) (int64, error) {
	backingRwFile, err := d.Backend.Writable()

	if err != nil {
		return -1, err
	}
	if d.Table == nil {
		return -1, fmt.Errorf("cannot write contents of a partition on a disk without a partition table")
	}
	if part < 0 {
		return -1, fmt.Errorf("cannot write contents of a partition without specifying a partition")
	}
	partitions := d.Table.GetPartitions()
	// API indexes from 1, but slice from 0
	if part > len(partitions) {
		return -1, fmt.Errorf("cannot write contents of partition %d which is greater than max partition %d", part, len(partitions))
	}
	written, err := partitions[part-1].WriteContents(backingRwFile, reader)
	return int64(written), err
}

// ReadPartitionContents reads the contents of a partition to an io.Writer
//
// if successful, returns the number of bytes read
//
// returns an error if there was an error reading from the disk, writing to the writer, the table
// is invalid, or the partition is invalid
func (d *Disk) ReadPartitionContents(part int, writer io.Writer) (int64, error) {
	if d.Table == nil {
		return -1, fmt.Errorf("cannot read contents of a partition on a disk without a partition table")
	}
	if part < 0 {
		return -1, fmt.Errorf("cannot read contents of a partition without specifying a partition")
	}
	partitions := d.Table.GetPartitions()
	// API indexes from 1, but slice from 0
	if part > len(partitions) {
		return -1, fmt.Errorf("cannot read contents of partition %d which is greater than max partition %d", part, len(partitions))
	}
	return partitions[part-1].ReadContents(d.Backend, writer)
}

// FilesystemSpec represents the specification of a filesystem to be created
type FilesystemSpec struct {
	Partition   int
	FSType      filesystem.Type
	VolumeLabel string
	WorkDir     string
}

// CreateFilesystem creates a filesystem on a disk image, the equivalent of mkfs.
//
// Required:
//   - desired partition number, or 0 to create the filesystem on the entire block device or
//     disk image,
//   - the filesystem type from github.com/diskfs/go-diskfs/filesystem
//
// Optional:
//   - volume label for those filesystems that support it; under Linux this shows
//     in '/dev/disks/by-label/<label>'
//
// if successful, returns a filesystem-implementing structure for the given filesystem type
//
// returns error if there was an error creating the filesystem, or the partition table is invalid and did not
// request the entire disk.
func (d *Disk) CreateFilesystem(spec FilesystemSpec) (filesystem.FileSystem, error) {
	// find out where the partition starts and ends, or if it is the entire disk
	var (
		size, start int64
	)

	switch {
	case spec.Partition == 0:
		size = d.Size
		start = 0
	case d.Table == nil:
		return nil, fmt.Errorf("cannot create filesystem on a partition without a partition table")
	default:
		partitions := d.Table.GetPartitions()
		// API indexes from 1, but slice from 0
		part := spec.Partition - 1
		if spec.Partition > len(partitions) {
			return nil, fmt.Errorf("cannot create filesystem on partition %d greater than maximum partition %d", spec.Partition, len(partitions))
		}
		size = partitions[part].GetSize()
		start = partitions[part].GetStart()
	}

	switch spec.FSType {
	case filesystem.TypeFat32:
		return fat32.Create(d.Backend, size, start, d.LogicalBlocksize, spec.VolumeLabel)
	case filesystem.TypeISO9660:
		return iso9660.Create(d.Backend, size, start, d.LogicalBlocksize, spec.WorkDir)
	case filesystem.TypeExt4:
		return ext4.Create(d.Backend, size, start, d.LogicalBlocksize, nil)
	case filesystem.TypeSquashfs:
		return nil, filesystem.ErrReadonlyFilesystem
	default:
		return nil, errors.New("unknown filesystem type requested")
	}
}

// GetFilesystem gets the filesystem that already exists on a disk image
//
// pass the desired partition number, or 0 to create the filesystem on the entire block device / disk image,
//
// if successful, returns a filesystem-implementing structure for the given filesystem type
//
// returns error if there was an error reading the filesystem, or the partition table is invalid and did not
// request the entire disk.
func (d *Disk) GetFilesystem(part int) (filesystem.FileSystem, error) {
	// find out where the partition starts and ends, or if it is the entire disk
	var (
		size, start int64
		err         error
	)

	switch {
	case part == 0:
		size = d.Size
		start = 0
	case d.Table == nil:
		return nil, fmt.Errorf("cannot read filesystem on a partition without a partition table")
	default:
		partitions := d.Table.GetPartitions()
		// API indexes from 1, but slice from 0
		if part > len(partitions) {
			return nil, fmt.Errorf("cannot get filesystem on partition %d greater than maximum partition %d", part, len(partitions))
		}
		size = partitions[part-1].GetSize()
		start = partitions[part-1].GetStart()
	}

	// just try each type
	log.Debug("trying fat32")
	fat32FS, err := fat32.Read(d.Backend, size, start, d.LogicalBlocksize)
	if err == nil {
		return fat32FS, nil
	}
	log.Debugf("fat32 failed: %v", err)
	pbs := d.PhysicalBlocksize
	if d.DefaultBlocks {
		pbs = 0
	}
	log.Debugf("trying iso9660 with physical block size %d", pbs)
	iso9660FS, err := iso9660.Read(d.Backend, size, start, pbs)
	if err == nil {
		return iso9660FS, nil
	}
	log.Debugf("iso9660 failed: %v", err)
	squashFS, err := squashfs.Read(d.Backend, size, start, d.LogicalBlocksize)
	if err == nil {
		return squashFS, nil
	}
	log.Debug("trying ext4")
	ext4FS, err := ext4.Read(d.Backend, size, start, d.LogicalBlocksize)
	if err == nil {
		return ext4FS, nil
	}
	log.Debugf("ext4 failed: %v", err)
	return nil, fmt.Errorf("unknown filesystem on partition %d", part)
}

// Close the disk. Once successfully closed, it can no longer be used.
func (d *Disk) Close() error {
	if err := d.Backend.Close(); err != nil {
		return err
	}
	*d = Disk{}
	return nil
}
