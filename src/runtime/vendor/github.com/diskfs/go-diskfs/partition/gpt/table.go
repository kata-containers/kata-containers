package gpt

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"hash/crc32"
	"strings"

	"github.com/diskfs/go-diskfs/backend"
	"github.com/diskfs/go-diskfs/partition/part"
	uuid "github.com/google/uuid"
)

// gptSize max potential size for partition array reserved 16384
const (
	mbrPartitionEntriesStart = 446
	mbrPartitionEntriesCount = 4
	mbrpartitionEntrySize    = 16
	// just defaults
	physicalSectorSize = 512
	logicalSectorSize  = 512
	gptHeaderSector    = 1
)

// Table represents a partition table to be applied to a disk or read from a disk
type Table struct {
	Partitions             []*Partition // slice of Partition
	LogicalSectorSize      int          // logical size of a sector
	PhysicalSectorSize     int          // physical size of the sector
	GUID                   string       // disk GUID, can be left blank to auto-generate
	ProtectiveMBR          bool         // whether or not a protective MBR is in place
	partitionArraySize     int          // how many entries are in the partition array size
	partitionEntrySize     uint32       // size of the partition entry in the table, usually 128 bytes
	partitionFirstLBA      uint64       // first LBA of the partition array
	partitionEntryChecksum uint32       // checksum of the partition array
	primaryHeader          uint64       // LBA of primary header, always 1
	secondaryHeader        uint64       // LBA of secondary header, always last sectors on disk
	firstDataSector        uint64       // LBA of first data sector
	lastDataSector         uint64       // LBA of last data sector
	initialized            bool
}

func getEfiSignature() []byte {
	return []byte{0x45, 0x46, 0x49, 0x20, 0x50, 0x41, 0x52, 0x54}
}
func getEfiRevision() []byte {
	return []byte{0x00, 0x00, 0x01, 0x00}
}
func getEfiHeaderSize() []byte {
	return []byte{0x5c, 0x00, 0x00, 0x00}
}
func getEfiZeroes() []byte {
	return []byte{0x00, 0x00, 0x00, 0x00}
}
func getMbrSignature() []byte {
	return []byte{0x55, 0xaa}
}

// check if a byte slice is all zeroes
func zeroMatch(b []byte) bool {
	if len(b) < 1 {
		return true
	}
	for _, val := range b {
		if val != 0 {
			return false
		}
	}
	return true
}

// ensure that a blank table is initialized
func (t *Table) initTable(size int64) {
	// default settings
	if t.LogicalSectorSize == 0 {
		t.LogicalSectorSize = 512
	}
	if t.PhysicalSectorSize == 0 {
		t.PhysicalSectorSize = 512
	}

	if t.primaryHeader == 0 {
		t.primaryHeader = 1
	}
	if t.GUID == "" {
		guid, _ := uuid.NewRandom()
		t.GUID = guid.String()
	}
	if t.partitionArraySize == 0 {
		t.partitionArraySize = 128
	}
	if t.partitionEntrySize == 0 {
		t.partitionEntrySize = 128
	}

	// how many sectors on the disk?
	diskSectors := uint64(size) / uint64(t.LogicalSectorSize)
	// how many sectors used for partition entries?
	partSectors := uint64(t.partitionArraySize) * uint64(t.partitionEntrySize) / uint64(t.LogicalSectorSize)

	if t.firstDataSector == 0 {
		t.firstDataSector = 2 + partSectors
	}

	if t.secondaryHeader == 0 {
		t.secondaryHeader = diskSectors - 1
	}
	if t.lastDataSector == 0 {
		t.lastDataSector = t.secondaryHeader - partSectors - 1
	}

	t.initialized = true
}

// Equal check if another table is functionally equal to this one
func (t *Table) Equal(t2 *Table) bool {
	if t2 == nil {
		return false
	}
	// neither is nil, so now we need to compare
	basicMatch := t.LogicalSectorSize == t2.LogicalSectorSize &&
		t.PhysicalSectorSize == t2.PhysicalSectorSize &&
		t.partitionEntrySize == t2.partitionEntrySize &&
		t.primaryHeader == t2.primaryHeader &&
		t.secondaryHeader == t2.secondaryHeader &&
		t.firstDataSector == t2.firstDataSector &&
		t.lastDataSector == t2.lastDataSector &&
		t.partitionArraySize == t2.partitionArraySize &&
		t.ProtectiveMBR == t2.ProtectiveMBR &&
		t.GUID == t2.GUID
	partMatch := comparePartitionArray(t.Partitions, t2.Partitions)
	return basicMatch && partMatch
}
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
		if p.Type == Unused && p2[i].Type == Unused {
			continue
		}
		if *p != *p2[i] {
			matches = false
			break
		}
	}
	return matches
}

// readProtectiveMBR reads whether or not a protectiveMBR exists in a byte slice
func readProtectiveMBR(b []byte, sectors uint32) bool {
	size := len(b)
	if size < 512 {
		return false
	}
	// check for MBR signature
	if !bytes.Equal(b[size-2:], getMbrSignature()) {
		return false
	}
	// get the partitions
	parts := b[mbrPartitionEntriesStart : mbrPartitionEntriesStart+mbrpartitionEntrySize*mbrPartitionEntriesCount]
	// should have all except the first partition by zeroes
	for i := 1; i < mbrPartitionEntriesCount; i++ {
		if !zeroMatch(parts[i*mbrpartitionEntrySize : (i+1)*mbrpartitionEntrySize]) {
			return false
		}
	}
	// finally the first one should be a partition of type 0xee that covers the whole disk and has non-bootable

	// non-bootable
	if parts[0] != 0x00 {
		return false
	}
	// we ignore head/cylinder/sector
	// partition type 0xee
	if parts[4] != 0xee {
		return false
	}
	if binary.LittleEndian.Uint32(parts[8:12]) != 1 {
		return false
	}
	if binary.LittleEndian.Uint32(parts[12:16]) != sectors {
		return false
	}
	return true
}

// partitionArraySector get the sector that holds the primary or secondary partition array
func (t *Table) partitionArraySector(primary bool) uint64 {
	if primary {
		return t.primaryHeader + 1
	}
	return t.secondaryHeader - uint64(t.partitionArraySize)*uint64(t.partitionEntrySize)/uint64(t.LogicalSectorSize)
}

func (t *Table) generateProtectiveMBR() []byte {
	b := make([]byte, 512)
	// we don't do anything to the first 446 bytes
	copy(b[510:], getMbrSignature())
	// create the single all disk partition
	parts := b[mbrPartitionEntriesStart : mbrPartitionEntriesStart+mbrpartitionEntrySize]
	// non-bootable
	parts[0] = 0x00
	// ignore CHS entirely
	// partition type 0xee
	parts[4] = 0xee
	// ignore CHS entirely
	// start LBA 1
	binary.LittleEndian.PutUint32(parts[8:12], 1)
	// end LBA last omne on disk
	binary.LittleEndian.PutUint32(parts[12:16], uint32(t.secondaryHeader))
	return b
}

// toPartitionArrayBytes write the bytes for the partition array
func (t *Table) toPartitionArrayBytes() ([]byte, error) {
	blocksize := uint64(t.LogicalSectorSize)
	firstblock := t.LogicalSectorSize
	nextstart := uint64(firstblock)

	// go through the partitions, make sure Start/End/Size are correct, and each has a GUID
	for i, part := range t.Partitions {
		err := part.initEntry(blocksize, nextstart)
		if err != nil {
			return nil, fmt.Errorf("could not initialize partition %d correctly: %v", i, err)
		}

		nextstart = part.End + 1
	}

	// generate the partition bytes
	partSize := t.partitionEntrySize * uint32(t.partitionArraySize)
	bpart := make([]byte, partSize)
	for i, p := range t.Partitions {
		// write the primary partition entry
		b2, err := p.toBytes()
		if err != nil {
			return nil, fmt.Errorf("error preparing partition entry %d for writing to disk: %v", i, err)
		}
		slotStart := i * int(t.partitionEntrySize)
		slotEnd := slotStart + int(t.partitionEntrySize)
		copy(bpart[slotStart:slotEnd], b2)
	}
	return bpart, nil
}

// toGPTBytes write just the gpt header to bytes
func (t *Table) toGPTBytes(primary bool) ([]byte, error) {
	b := make([]byte, t.LogicalSectorSize)

	// 8 bytes "EFI PART" signature - endianness on this?
	copy(b[0:8], getEfiSignature())
	// 4 bytes revision 1.0
	copy(b[8:12], getEfiRevision())
	// 4 bytes header size
	copy(b[12:16], getEfiHeaderSize())
	// 4 bytes CRC32/zlib of header with this field zeroed out - must calculate then come back
	copy(b[16:20], []byte{0x00, 0x00, 0x00, 0x00})
	// 4 bytes zeroes reserved
	copy(b[20:24], getEfiZeroes())

	// which LBA are we?
	if primary {
		binary.LittleEndian.PutUint64(b[24:32], t.primaryHeader)
		binary.LittleEndian.PutUint64(b[32:40], t.secondaryHeader)
	} else {
		binary.LittleEndian.PutUint64(b[24:32], t.secondaryHeader)
		binary.LittleEndian.PutUint64(b[32:40], t.primaryHeader)
	}

	// usable LBAs for partitions
	binary.LittleEndian.PutUint64(b[40:48], t.firstDataSector)
	binary.LittleEndian.PutUint64(b[48:56], t.lastDataSector)

	// 16 bytes disk GUID
	var guid uuid.UUID
	if t.GUID == "" {
		guid, _ = uuid.NewRandom()
	} else {
		var err error
		guid, err = uuid.Parse(t.GUID)
		if err != nil {
			return nil, fmt.Errorf("invalid UUID: %s", t.GUID)
		}
	}
	copy(b[56:72], bytesToUUIDBytes(guid[0:16]))

	// starting LBA of array of partition entries
	binary.LittleEndian.PutUint64(b[72:80], t.partitionArraySector(primary))

	// how many entries?
	binary.LittleEndian.PutUint32(b[80:84], uint32(t.partitionArraySize))
	// how big is a single entry?
	binary.LittleEndian.PutUint32(b[84:88], 0x80)

	// we need a CRC/zlib of the partition entries, so we do those first, then append the bytes
	bpart, err := t.toPartitionArrayBytes()
	if err != nil {
		return nil, fmt.Errorf("error converting partition array to bytes: %v", err)
	}
	checksum := crc32.ChecksumIEEE(bpart)
	binary.LittleEndian.PutUint32(b[88:92], checksum)

	// calculate checksum of entire header and place 4 bytes of offset 16 = 0x10
	checksum = crc32.ChecksumIEEE(b[0:92])
	binary.LittleEndian.PutUint32(b[16:20], checksum)

	// zeroes to the end of the sector
	for i := 92; i < t.LogicalSectorSize; i++ {
		b[i] = 0x00
	}

	return b, nil
}

func (t *Table) calculatePartitionArrayLocations() (start, size int) {
	start = int(t.partitionFirstLBA) * t.LogicalSectorSize
	size = t.partitionArraySize * int(t.partitionEntrySize)
	return
}

// readPartitionArrayBytes read the bytes for the partition array
func readPartitionArrayBytes(b []byte, entrySize, logicalSectorSize, physicalSectorSize int) ([]*Partition, error) {
	parts := make([]*Partition, 0)
	for i, c := 0, b; len(c) >= entrySize; c, i = c[entrySize:], i+1 {
		bpart := c[:entrySize]
		// write the primary partition entry
		p, err := partitionFromBytes(bpart, logicalSectorSize, physicalSectorSize)
		if err != nil {
			return nil, fmt.Errorf("error reading partition entry %d: %v", i, err)
		}
		if p == nil {
			continue
		}
		// augment partition information
		p.Size = (p.End - p.Start + 1) * uint64(logicalSectorSize)
		parts = append(parts, p)
	}
	return parts, nil
}

// readGPTHeader reads the GPT header from the given byte slice
func readGPTHeader(b []byte) (*Table, error) {
	gpt := b
	// start with fixed headers
	efiSignature := gpt[0:8]
	efiRevision := gpt[8:12]
	efiHeaderSize := gpt[12:16]
	efiHeaderCrcBytes := append(make([]byte, 0, 4), gpt[16:20]...)
	efiHeaderCrc := binary.LittleEndian.Uint32(efiHeaderCrcBytes)
	efiZeroes := gpt[20:24]
	primaryHeader := binary.LittleEndian.Uint64(gpt[24:32])
	secondaryHeader := binary.LittleEndian.Uint64(gpt[32:40])
	firstDataSector := binary.LittleEndian.Uint64(gpt[40:48])
	lastDataSector := binary.LittleEndian.Uint64(gpt[48:56])
	diskGUID, err := uuid.FromBytes(bytesToUUIDBytes(gpt[56:72]))
	if err != nil {
		return nil, fmt.Errorf("unable to read guid from disk: %v", err)
	}
	partitionEntryFirstLBA := binary.LittleEndian.Uint64(gpt[72:80])
	partitionEntryCount := binary.LittleEndian.Uint32(gpt[80:84])
	partitionEntrySize := binary.LittleEndian.Uint32(gpt[84:88])
	partitionEntryChecksum := binary.LittleEndian.Uint32(gpt[88:92])

	// once we have the header CRC, zero it out
	copy(gpt[16:20], []byte{0x00, 0x00, 0x00, 0x00})
	if !bytes.Equal(efiSignature, getEfiSignature()) {
		return nil, fmt.Errorf("invalid EFI Signature %v", efiSignature)
	}
	if !bytes.Equal(efiRevision, getEfiRevision()) {
		return nil, fmt.Errorf("invalid EFI Revision %v", efiRevision)
	}
	if !bytes.Equal(efiHeaderSize, getEfiHeaderSize()) {
		return nil, fmt.Errorf("invalid EFI Header size %v", efiHeaderSize)
	}
	if !bytes.Equal(efiZeroes, getEfiZeroes()) {
		return nil, fmt.Errorf("invalid EFI Header, expected zeroes, got %v", efiZeroes)
	}
	// get the checksum
	checksum := crc32.ChecksumIEEE(gpt[0:92])
	if efiHeaderCrc != checksum {
		return nil, fmt.Errorf("invalid EFI Header Checksum, expected %v, got %v", checksum, efiHeaderCrc)
	}

	table := Table{
		partitionEntrySize:     partitionEntrySize,
		primaryHeader:          primaryHeader,
		secondaryHeader:        secondaryHeader,
		firstDataSector:        firstDataSector,
		lastDataSector:         lastDataSector,
		partitionArraySize:     int(partitionEntryCount),
		partitionFirstLBA:      partitionEntryFirstLBA,
		GUID:                   strings.ToUpper(diskGUID.String()),
		partitionEntryChecksum: partitionEntryChecksum,
	}

	return &table, nil
}

// tableHeaderFromBytes read a partition table from a byte slice, mainly used to validate the secondary header
func tableHeaderFromBytes(b []byte, logicalBlockSize, physicalBlockSize int, skipMBR bool) (*Table, error) {
	// minimum size - gpt entries + header + LBA0 for (protective) MBR
	minSize := logicalBlockSize
	if len(b) < minSize {
		return nil, fmt.Errorf("data for partition was %d bytes instead of expected minimum %d", len(b), minSize)
	}
	gpt := b
	if skipMBR {
		gpt = b[logicalBlockSize:]
	}

	table, err := readGPTHeader(gpt)
	if err != nil {
		return nil, err
	}

	// potential protective MBR is at LBA0
	table.ProtectiveMBR = readProtectiveMBR(b[:logicalBlockSize], uint32(table.secondaryHeader))
	table.LogicalSectorSize = logicalBlockSize
	table.PhysicalSectorSize = physicalBlockSize
	table.initialized = true

	return table, nil
}

// tableFromBytes read a partition table from a byte slice
func tableFromBytes(b []byte, logicalBlockSize, physicalBlockSize int) (*Table, error) {
	// minimum size - gpt entries + header + LBA0 for (protective) MBR
	if len(b) < logicalBlockSize*2 {
		return nil, fmt.Errorf("data for partition was %d bytes instead of expected minimum %d", len(b), logicalBlockSize*2)
	}

	// GPT starts at LBA1
	gpt := b[logicalBlockSize:]

	table, err := readGPTHeader(gpt)
	if err != nil {
		return nil, err
	}

	// potential protective MBR is at LBA0
	table.ProtectiveMBR = readProtectiveMBR(b[:logicalBlockSize], uint32(table.secondaryHeader))
	table.LogicalSectorSize = logicalBlockSize
	table.PhysicalSectorSize = physicalBlockSize
	table.initialized = true

	return table, nil
}

// Type report the type of table, always "gpt"
func (t *Table) Type() string {
	return "gpt"
}

// Write writes a GPT to disk
// Must be passed the backend.WritableFile to which to write and the size of the disk
func (t *Table) Write(f backend.WritableFile, size int64) error {
	// it is possible that we are given a basic new table that we need to initialize
	if !t.initialized {
		t.initTable(size)
	}

	// write the protectiveMBR if any
	// write the primary GPT header
	// write the primary partition array
	// write the secondary partition array
	// write the secondary GPT header
	var written int
	var err error
	if t.ProtectiveMBR {
		fullMBR := t.generateProtectiveMBR()
		protectiveMBR := fullMBR[mbrPartitionEntriesStart:]
		written, err = f.WriteAt(protectiveMBR, mbrPartitionEntriesStart)
		if err != nil {
			return fmt.Errorf("error writing protective MBR to disk: %v", err)
		}
		if written != len(protectiveMBR) {
			return fmt.Errorf("wrote %d bytes of protective MBR instead of %d", written, len(protectiveMBR))
		}
	}

	primaryHeader, err := t.toGPTBytes(true)
	if err != nil {
		return fmt.Errorf("error converting primary GPT header to byte array: %v", err)
	}
	written, err = f.WriteAt(primaryHeader, int64(t.LogicalSectorSize))
	if err != nil {
		return fmt.Errorf("error writing primary GPT to disk: %v", err)
	}
	if written != len(primaryHeader) {
		return fmt.Errorf("wrote %d bytes of primary GPT header instead of %d", written, len(primaryHeader))
	}

	partitionArray, err := t.toPartitionArrayBytes()
	if err != nil {
		return fmt.Errorf("error converting primary GPT partitions to byte array: %v", err)
	}
	written, err = f.WriteAt(partitionArray, int64(t.LogicalSectorSize*int(t.partitionArraySector(true))))
	if err != nil {
		return fmt.Errorf("error writing primary partition arrayto disk: %v", err)
	}
	if written != len(partitionArray) {
		return fmt.Errorf("wrote %d bytes of primary partition array instead of %d", written, len(primaryHeader))
	}

	written, err = f.WriteAt(partitionArray, int64(t.LogicalSectorSize)*int64(t.partitionArraySector(false)))
	if err != nil {
		return fmt.Errorf("error writing secondary partition array to disk: %v", err)
	}
	if written != len(partitionArray) {
		return fmt.Errorf("wrote %d bytes of secondary partition array instead of %d", written, len(primaryHeader))
	}

	secondaryHeader, err := t.toGPTBytes(false)
	if err != nil {
		return fmt.Errorf("error converting secondary GPT header to byte array: %v", err)
	}
	written, err = f.WriteAt(secondaryHeader, int64(t.secondaryHeader)*int64(t.LogicalSectorSize))
	if err != nil {
		return fmt.Errorf("error writing secondary GPT to disk: %v", err)
	}
	if written != len(secondaryHeader) {
		return fmt.Errorf("wrote %d bytes of secondary GPT header instead of %d", written, len(secondaryHeader))
	}

	return nil
}

// Read read a partition table from a disk
// must be passed the backend.File from which to read, and the logical and physical block sizes
//
// if successful, returns a gpt.Table struct
// returns errors if fails at any stage reading the disk or processing the bytes on disk as a GPT
func Read(f backend.File, logicalBlockSize, physicalBlockSize int) (*Table, error) {
	// read the data off of the disk - first block is the compatibility MBR, ssecond is the GPT table
	b := make([]byte, logicalBlockSize*2)
	read, err := f.ReadAt(b, 0)
	if err != nil {
		return nil, fmt.Errorf("error reading GPT from file: %w", err)
	}
	if read != len(b) {
		return nil, fmt.Errorf("read only %d bytes of GPT from file instead of expected %d", read, len(b))
	}
	// get the gpt table
	gptTable, err := tableFromBytes(b, logicalBlockSize, physicalBlockSize)
	if err != nil {
		return nil, fmt.Errorf("error reading GPT table: %w", err)
	}
	start, size := gptTable.calculatePartitionArrayLocations()
	b = make([]byte, size)
	read, err = f.ReadAt(b, int64(start))
	if read != len(b) {
		return nil, fmt.Errorf("read only %d bytes of GPT from file instead of expected %d", read, len(b))
	}
	if err != nil {
		return nil, fmt.Errorf("error reading partitions from file: %w", err)
	}
	// we need a CRC/zlib of the partition entries, so we do those first, then append the bytes
	checksum := crc32.ChecksumIEEE(b)
	if gptTable.partitionEntryChecksum != checksum {
		return nil, fmt.Errorf("invalid EFI Partition Entry Checksum, expected %v, got %v", checksum, gptTable.partitionEntryChecksum)
	}

	parts, err := readPartitionArrayBytes(b, int(gptTable.partitionEntrySize), logicalBlockSize, physicalBlockSize)
	if err != nil {
		return nil, fmt.Errorf("error parsing partition data: %w", err)
	}
	gptTable.Partitions = parts
	// get the partition table
	return gptTable, nil
}

// GetPartitions get the partitions
func (t *Table) GetPartitions() []part.Partition {
	// each Partition matches the part.Partition interface, but golang does not accept passing them in a slice
	parts := make([]part.Partition, len(t.Partitions))
	for i, p := range t.Partitions {
		parts[i] = p
	}
	return parts
}

// UUID returns the partition table UUID (disk UUID)
func (t *Table) UUID() string {
	return t.GUID
}

// Verify will attempt to evaluate the headers
func (t *Table) Verify(f backend.File, diskSize uint64) error {
	if t.LogicalSectorSize == 0 {
		// Avoid divide by zero panic.
		return fmt.Errorf("table is not initialized")
	}

	// Determine the size of disk that GPT expects
	expectedDiskSize := (t.secondaryHeader + 1) * uint64(t.LogicalSectorSize)
	if diskSize != expectedDiskSize {
		return fmt.Errorf("secondary Header is not at end of the disk, expected =>  %d / actual => %d", expectedDiskSize, diskSize)
	}
	b := make([]byte, t.LogicalSectorSize)
	seekAddress := int64(t.secondaryHeader) * int64(t.LogicalSectorSize)
	_, err := f.ReadAt(b, seekAddress)
	if err != nil {
		return fmt.Errorf("error reading GPT from file at %d / disksize %d : %v", seekAddress, diskSize, err)
	}
	secondaryTable, err := tableHeaderFromBytes(b, t.LogicalSectorSize, t.PhysicalSectorSize, false)
	if err != nil {
		return fmt.Errorf("error reading GPT from file at %d / disksize %d : %v", seekAddress, diskSize, err)
	}
	if t.firstDataSector != secondaryTable.firstDataSector {
		return fmt.Errorf("error comparing GPT headers expected =>  %d / actual => %d", t.firstDataSector, secondaryTable.firstDataSector)
	}
	partSectors := uint64(t.partitionArraySize) * uint64(t.partitionEntrySize) / uint64(t.LogicalSectorSize)
	lastDataSector := t.secondaryHeader - partSectors - 1
	if t.lastDataSector != lastDataSector {
		return fmt.Errorf("error comparing GPT secondary headers expected =>  %d / actual => %d", t.lastDataSector, lastDataSector)
	}
	return nil
}

// Repair will attempt to evaluate the headers fix the header location and re-write the primary and secondary header
func (t *Table) Repair(diskSize uint64) error {
	if t.LogicalSectorSize == 0 {
		// Avoid divide by zero panic.
		return fmt.Errorf("table is not initialized")
	}

	partSectors := uint64(t.partitionArraySize) * uint64(t.partitionEntrySize) / uint64(t.LogicalSectorSize)

	t.secondaryHeader = (diskSize / uint64(t.LogicalSectorSize)) - 1
	t.lastDataSector = t.secondaryHeader - partSectors - 1

	return nil
}

// TotalSize returns the total size of the GPT in bytes.
//
// This is counted from the start of the MBR to the end of the secondary
// header.
func (t *Table) TotalSize() uint64 {
	return (t.secondaryHeader + gptHeaderSector) * uint64(t.LogicalSectorSize)
}

func (t *Table) LastDataSector() uint64 {
	return t.lastDataSector
}

// Resize changes the size of the GPT.
//
// The size argument is in bytes and must be a multiple of the logical sector
// size.
// Use this function in case a storage device is not the same as the total
// size of its GPT.
func (t *Table) Resize(size uint64) {
	// how many sectors on the disk?
	diskSectors := size / uint64(t.LogicalSectorSize)
	// how many sectors used for partition entries?
	partSectors := uint64(t.partitionArraySize) * uint64(t.partitionEntrySize) / uint64(t.LogicalSectorSize)

	t.secondaryHeader = diskSectors - 1
	t.lastDataSector = t.secondaryHeader - 1 - partSectors
}
