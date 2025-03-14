package mbr

import (
	"bytes"
	"encoding/binary"
	"fmt"

	"github.com/diskfs/go-diskfs/backend"
	"github.com/diskfs/go-diskfs/partition/part"
)

// Table represents an MBR partition table to be applied to a disk or read from a disk
type Table struct {
	Partitions         []*Partition
	LogicalSectorSize  int // logical size of a sector
	PhysicalSectorSize int // physical size of the sector
	partitionTableUUID string
}

const (
	mbrSize               = 512
	logicalSectorSize     = 512
	physicalSectorSize    = 512
	partitionEntriesStart = 446
	partitionEntriesCount = 4
	signatureStart        = 510
	// the partition table UUID is stored in 4 bytes in the MBR
	partitionTableUUIDStart = 440
	partitionTableUUIDEnd   = 444
)

// partitionEntrySize standard size of an MBR partition
const partitionEntrySize = 16

func getMbrSignature() []byte {
	return []byte{0x55, 0xaa}
}

// compare 2 partition arrays
func comparePartitionArray(p1, p2 []*Partition) bool {
	if (p1 == nil && p2 != nil) || (p2 == nil && p1 != nil) {
		return false
	}
	if p1 == nil && p2 == nil {
		return true
	}
	// neither is nil, so now we need to compare
	if len(p1) != len(p2) {
		return false
	}
	matches := true
	for i, p := range p1 {
		if p == nil && p2 != nil || !p.Equal(p2[i]) {
			matches = false
			break
		}
	}
	return matches
}

// Equal check if another table is equal to this one, ignoring the partition table UUID and CHS start and end for the partitions
func (t *Table) Equal(t2 *Table) bool {
	if t2 == nil {
		return false
	}
	// neither is nil, so now we need to compare
	basicMatch := t.LogicalSectorSize == t2.LogicalSectorSize &&
		t.PhysicalSectorSize == t2.PhysicalSectorSize
	partMatch := comparePartitionArray(t.Partitions, t2.Partitions)
	return basicMatch && partMatch
}

// tableFromBytes read a partition table from a byte slice
func tableFromBytes(b []byte) (*Table, error) {
	// check length
	if len(b) != mbrSize {
		return nil, fmt.Errorf("data for partition was %d bytes instead of expected %d", len(b), mbrSize)
	}

	// validate signature
	mbrSignature := b[signatureStart:]
	if !bytes.Equal(mbrSignature, getMbrSignature()) {
		return nil, fmt.Errorf("invalid MBR Signature %v", mbrSignature)
	}

	ptUUID := readPartitionTableUUID(b)
	parts := make([]*Partition, 0, partitionEntriesCount)
	count := int(partitionEntriesCount)
	for i := 0; i < count; i++ {
		// write the primary partition entry
		start := partitionEntriesStart + i*partitionEntrySize
		end := start + partitionEntrySize
		p, err := partitionFromBytes(b[start:end], logicalSectorSize, physicalSectorSize)
		if err != nil {
			return nil, fmt.Errorf("error reading partition entry %d: %v", i, err)
		}
		p.partitionUUID = formatPartitionUUID(ptUUID, i+1)
		parts = append(parts, p)
	}

	table := &Table{
		Partitions:         parts,
		LogicalSectorSize:  logicalSectorSize,
		PhysicalSectorSize: 512,
		partitionTableUUID: ptUUID,
	}

	return table, nil
}

func readPartitionTableUUID(b []byte) string {
	ptUUID := b[partitionTableUUIDStart:partitionTableUUIDEnd]
	return fmt.Sprintf("%x", binary.LittleEndian.Uint32(ptUUID))
}

// UUID returns the partition table UUID used to identify disks
func (t *Table) UUID() string {
	return t.partitionTableUUID
}

// formatPartitionUUID creates the partition UUID which is created by using the
// partition table UUID and the partition index.
// Format string taken from libblkid:
// https://github.com/util-linux/util-linux/blob/master/libblkid/src/partitions/partitions.c#L1387C42-L1387C52
func formatPartitionUUID(ptUUID string, index int) string {
	return fmt.Sprintf("%.33s-%02x", ptUUID, index)
}

// Type report the type of table, always the string "mbr"
func (t *Table) Type() string {
	return "mbr"
}

// Read read a partition table from a disk, given the logical block size and physical block size
//
//nolint:unused,revive // not used in MBR, but it is important to implement the interface
func Read(f backend.File, logicalBlockSize, physicalBlockSize int) (*Table, error) {
	// read the data off of the disk
	b := make([]byte, mbrSize)
	read, err := f.ReadAt(b, 0)
	if err != nil {
		return nil, fmt.Errorf("error reading MBR from file: %v", err)
	}
	if read != len(b) {
		return nil, fmt.Errorf("read only %d bytes of MBR from file instead of expected %d", read, len(b))
	}
	return tableFromBytes(b)
}

// ToBytes convert Table to byte slice suitable to be flashed to a disk
// If successful, always will return a byte slice of size exactly 512
func (t *Table) toBytes() []byte {
	b := make([]byte, 0, mbrSize-partitionEntriesStart)

	// write the partitions
	for i := 0; i < partitionEntriesCount; i++ {
		if i < len(t.Partitions) {
			btmp := t.Partitions[i].toBytes()
			b = append(b, btmp...)
		} else {
			b = append(b, []byte{0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00}...)
		}
	}

	// signature
	b = append(b, getMbrSignature()...)
	return b
}

// Write writes a given MBR Table to disk.
// Must be passed the backend.WritableFile to write to and the size of the disk
//
//nolint:unused,revive // not used in MBR, but it is important to implement the interface
func (t *Table) Write(f backend.WritableFile, size int64) error {
	b := t.toBytes()

	written, err := f.WriteAt(b, partitionEntriesStart)
	if err != nil {
		return fmt.Errorf("error writing partition table to disk: %v", err)
	}
	if written != len(b) {
		return fmt.Errorf("partition table wrote %d bytes to disk instead of the expected %d", written, len(b))
	}
	return nil
}

func (t *Table) GetPartitions() []part.Partition {
	// each Partition matches the part.Partition interface, but golang does not accept passing them in a slice
	parts := make([]part.Partition, len(t.Partitions))
	for i, p := range t.Partitions {
		parts[i] = p
	}
	return parts
}

// Verify will attempt to evaluate the headers
//
//nolint:unused,revive // not used in MBR, but it is important to implement the interface
func (t *Table) Verify(f backend.File, diskSize uint64) error {
	return nil
}

// Repair will attempt to repair a broken Master Boot Record
//
//nolint:unused,revive // not used in MBR, but it is important to implement the interface
func (t *Table) Repair(diskSize uint64) error {
	return nil
}
