package mbr

import (
	"bytes"
	"fmt"

	"github.com/diskfs/go-diskfs/partition/part"
	"github.com/diskfs/go-diskfs/util"
)

// Table represents an MBR partition table to be applied to a disk or read from a disk
type Table struct {
	Partitions         []*Partition
	LogicalSectorSize  int // logical size of a sector
	PhysicalSectorSize int // physical size of the sector
	initialized        bool
}

const (
	mbrSize               = 512
	logicalSectorSize     = 512
	physicalSectorSize    = 512
	partitionEntriesStart = 446
	partitionEntriesCount = 4
	signatureStart        = 510
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

// ensure that a blank table is initialized
func (t *Table) initTable() {
	// default settings
	if t.LogicalSectorSize == 0 {
		t.LogicalSectorSize = 512
	}
	if t.PhysicalSectorSize == 0 {
		t.PhysicalSectorSize = 512
	}

	t.initialized = true
}

// Equal check if another table is equal to this one, ignoring CHS start and end for the partitions
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
	mbrSignature := b[signatureStart:]

	// validate signature
	if !bytes.Equal(mbrSignature, getMbrSignature()) {
		return nil, fmt.Errorf("invalid MBR Signature %v", mbrSignature)
	}

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
		parts = append(parts, p)
	}

	table := &Table{
		Partitions:         parts,
		LogicalSectorSize:  logicalSectorSize,
		PhysicalSectorSize: 512,
	}

	return table, nil
}

// Type report the type of table, always the string "mbr"
func (t *Table) Type() string {
	return "mbr"
}

// Read read a partition table from a disk, given the logical block size and physical block size
func Read(f util.File, logicalBlockSize, physicalBlockSize int) (*Table, error) {
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
// Must be passed the util.File to write to and the size of the disk
func (t *Table) Write(f util.File, size int64) error {
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
