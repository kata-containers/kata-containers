package ext4

import (
	"cmp"
	"encoding/binary"
	"fmt"
	"slices"

	"github.com/diskfs/go-diskfs/filesystem/ext4/crc"
)

type blockGroupFlag uint16
type gdtChecksumType uint8

func (b blockGroupFlag) included(a uint16) bool {
	return a&uint16(b) == uint16(b)
}

//nolint:unused // will be used in the future, not yet
func (g gdtChecksumType) included(a uint8) bool {
	return a&uint8(g) == uint8(g)
}

const (
	groupDescriptorSize                    uint16          = 32
	groupDescriptorSize64Bit               uint16          = 64
	blockGroupFlagInodesUninitialized      blockGroupFlag  = 0x1
	blockGroupFlagBlockBitmapUninitialized blockGroupFlag  = 0x2
	blockGroupFlagInodeTableZeroed         blockGroupFlag  = 0x4
	gdtChecksumNone                        gdtChecksumType = 0
	gdtChecksumGdt                         gdtChecksumType = 1
	gdtChecksumMetadata                    gdtChecksumType = 2
)

type blockGroupFlags struct {
	inodesUninitialized      bool
	blockBitmapUninitialized bool
	inodeTableZeroed         bool
}

// groupdescriptors is a structure holding all of the group descriptors for all of the block groups
type groupDescriptors struct {
	descriptors []groupDescriptor
}

// groupDescriptor is a structure holding the data about a single block group
type groupDescriptor struct {
	blockBitmapLocation             uint64
	inodeBitmapLocation             uint64
	inodeTableLocation              uint64
	freeBlocks                      uint32
	freeInodes                      uint32
	usedDirectories                 uint32
	flags                           blockGroupFlags
	snapshotExclusionBitmapLocation uint64
	blockBitmapChecksum             uint32
	inodeBitmapChecksum             uint32
	unusedInodes                    uint32
	size                            uint16
	number                          uint16
}

func (gd *groupDescriptor) equal(other *groupDescriptor) bool {
	if other == nil {
		return gd == nil
	}
	return *gd == *other
}

func (gds *groupDescriptors) equal(a *groupDescriptors) bool {
	if gds == nil && a == nil {
		return true
	}
	if (gds == nil && a != nil) || (a == nil && gds != nil) || len(gds.descriptors) != len(a.descriptors) {
		return false
	}

	// both not nil, same size, so compare them
	for i, g := range gds.descriptors {
		if g != a.descriptors[i] {
			return false
		}
	}
	// if we made it this far, all the same
	return true
}

// groupDescriptorsFromBytes create a groupDescriptors struct from bytes
func groupDescriptorsFromBytes(b []byte, gdSize uint16, hashSeed uint32, checksumType gdtChecksumType) (*groupDescriptors, error) {
	gds := groupDescriptors{}
	gdSlice := make([]groupDescriptor, 0, 10)

	count := len(b) / int(gdSize)

	// go through them gdSize bytes at a time
	for i := 0; i < count; i++ {
		start := i * int(gdSize)
		end := start + int(gdSize)
		gd, err := groupDescriptorFromBytes(b[start:end], gdSize, i, checksumType, hashSeed)
		if err != nil || gd == nil {
			return nil, fmt.Errorf("error creating group descriptor from bytes: %w", err)
		}
		gdSlice = append(gdSlice, *gd)
	}
	gds.descriptors = gdSlice

	return &gds, nil
}

// toBytes returns groupDescriptors ready to be written to disk
func (gds *groupDescriptors) toBytes(checksumType gdtChecksumType, hashSeed uint32) []byte {
	b := make([]byte, 0, 10*groupDescriptorSize)
	for _, gd := range gds.descriptors {
		b2 := gd.toBytes(checksumType, hashSeed)
		b = append(b, b2...)
	}

	return b
}

// byFreeBlocks provides a sorted list of groupDescriptors by free blocks, descending.
// If you want them ascending, sort if.
func (gds *groupDescriptors) byFreeBlocks() []groupDescriptor {
	// make a copy of the slice
	gdSlice := make([]groupDescriptor, len(gds.descriptors))
	copy(gdSlice, gds.descriptors)

	// sort the slice
	slices.SortFunc(gdSlice, func(a, b groupDescriptor) int {
		return cmp.Compare(a.freeBlocks, b.freeBlocks)
	})

	return gdSlice
}

// groupDescriptorFromBytes create a groupDescriptor struct from bytes
func groupDescriptorFromBytes(b []byte, gdSize uint16, number int, checksumType gdtChecksumType, hashSeed uint32) (*groupDescriptor, error) {
	// block count, reserved block count and free blocks depends on whether the fs is 64-bit or not
	blockBitmapLocation := make([]byte, 8)
	inodeBitmapLocation := make([]byte, 8)
	inodeTableLocation := make([]byte, 8)
	freeBlocks := make([]byte, 4)
	freeInodes := make([]byte, 4)
	usedirectories := make([]byte, 4)
	snapshotExclusionBitmapLocation := make([]byte, 8)
	blockBitmapChecksum := make([]byte, 4)
	inodeBitmapChecksum := make([]byte, 4)
	unusedInodes := make([]byte, 4)

	copy(blockBitmapLocation[0:4], b[0x0:0x4])
	copy(inodeBitmapLocation[0:4], b[0x4:0x8])
	copy(inodeTableLocation[0:4], b[0x8:0xc])
	copy(freeBlocks[0:2], b[0xc:0xe])
	copy(freeInodes[0:2], b[0xe:0x10])
	copy(usedirectories[0:2], b[0x10:0x12])
	copy(snapshotExclusionBitmapLocation[0:4], b[0x14:0x18])
	copy(blockBitmapChecksum[0:2], b[0x18:0x1a])
	copy(inodeBitmapChecksum[0:2], b[0x1a:0x1c])
	copy(unusedInodes[0:2], b[0x1c:0x1e])

	if gdSize == 64 {
		copy(blockBitmapLocation[4:8], b[0x20:0x24])
		copy(inodeBitmapLocation[4:8], b[0x24:0x28])
		copy(inodeTableLocation[4:8], b[0x28:0x2c])
		copy(freeBlocks[2:4], b[0x2c:0x2e])
		copy(freeInodes[2:4], b[0x2e:0x30])
		copy(usedirectories[2:4], b[0x30:0x32])
		copy(unusedInodes[2:4], b[0x32:0x34])
		copy(snapshotExclusionBitmapLocation[4:8], b[0x34:0x38])
		copy(blockBitmapChecksum[2:4], b[0x38:0x3a])
		copy(inodeBitmapChecksum[2:4], b[0x3a:0x3c])
	}

	gdNumber := uint16(number)
	// only bother with checking the checksum if it was not type none (pre-checksums)
	if checksumType != gdtChecksumNone {
		checksum := binary.LittleEndian.Uint16(b[0x1e:0x20])
		actualChecksum := groupDescriptorChecksum(b[0x0:0x40], hashSeed, gdNumber, checksumType)
		if checksum != actualChecksum {
			return nil, fmt.Errorf("checksum mismatch, passed %x, actual %x", checksum, actualChecksum)
		}
	}

	gd := groupDescriptor{
		size:                            gdSize,
		number:                          gdNumber,
		blockBitmapLocation:             binary.LittleEndian.Uint64(blockBitmapLocation),
		inodeBitmapLocation:             binary.LittleEndian.Uint64(inodeBitmapLocation),
		inodeTableLocation:              binary.LittleEndian.Uint64(inodeTableLocation),
		freeBlocks:                      binary.LittleEndian.Uint32(freeBlocks),
		freeInodes:                      binary.LittleEndian.Uint32(freeInodes),
		usedDirectories:                 binary.LittleEndian.Uint32(usedirectories),
		snapshotExclusionBitmapLocation: binary.LittleEndian.Uint64(snapshotExclusionBitmapLocation),
		blockBitmapChecksum:             binary.LittleEndian.Uint32(blockBitmapChecksum),
		inodeBitmapChecksum:             binary.LittleEndian.Uint32(inodeBitmapChecksum),
		unusedInodes:                    binary.LittleEndian.Uint32(unusedInodes),
		flags:                           parseBlockGroupFlags(binary.LittleEndian.Uint16(b[0x12:0x14])),
	}

	return &gd, nil
}

// toBytes returns a groupDescriptor ready to be written to disk
func (gd *groupDescriptor) toBytes(checksumType gdtChecksumType, hashSeed uint32) []byte {
	gdSize := gd.size

	b := make([]byte, gdSize)

	blockBitmapLocation := make([]byte, 8)
	inodeBitmapLocation := make([]byte, 8)
	inodeTableLocation := make([]byte, 8)
	freeBlocks := make([]byte, 4)
	freeInodes := make([]byte, 4)
	usedirectories := make([]byte, 4)
	snapshotExclusionBitmapLocation := make([]byte, 8)
	blockBitmapChecksum := make([]byte, 4)
	inodeBitmapChecksum := make([]byte, 4)
	unusedInodes := make([]byte, 4)

	binary.LittleEndian.PutUint64(blockBitmapLocation, gd.blockBitmapLocation)
	binary.LittleEndian.PutUint64(inodeTableLocation, gd.inodeTableLocation)
	binary.LittleEndian.PutUint64(inodeBitmapLocation, gd.inodeBitmapLocation)
	binary.LittleEndian.PutUint32(freeBlocks, gd.freeBlocks)
	binary.LittleEndian.PutUint32(freeInodes, gd.freeInodes)
	binary.LittleEndian.PutUint32(usedirectories, gd.usedDirectories)
	binary.LittleEndian.PutUint64(snapshotExclusionBitmapLocation, gd.snapshotExclusionBitmapLocation)
	binary.LittleEndian.PutUint32(blockBitmapChecksum, gd.blockBitmapChecksum)
	binary.LittleEndian.PutUint32(inodeBitmapChecksum, gd.inodeBitmapChecksum)
	binary.LittleEndian.PutUint32(unusedInodes, gd.unusedInodes)

	// copy the lower 32 bytes in
	copy(b[0x0:0x4], blockBitmapLocation[0:4])
	copy(b[0x4:0x8], inodeBitmapLocation[0:4])
	copy(b[0x8:0xc], inodeTableLocation[0:4])
	copy(b[0xc:0xe], freeBlocks[0:2])
	copy(b[0xe:0x10], freeInodes[0:2])
	copy(b[0x10:0x12], usedirectories[0:2])
	binary.LittleEndian.PutUint16(b[0x12:0x14], gd.flags.toInt())
	copy(b[0x14:0x18], snapshotExclusionBitmapLocation[0:4])
	copy(b[0x18:0x1a], blockBitmapChecksum[0:2])
	copy(b[0x1a:0x1c], inodeBitmapChecksum[0:2])
	copy(b[0x1c:0x1e], unusedInodes[0:2])

	// now for the upper 32 bytes
	if gd.size == 64 {
		copy(b[0x20:0x24], blockBitmapLocation[4:8])
		copy(b[0x24:0x28], inodeBitmapLocation[4:8])
		copy(b[0x28:0x2c], inodeTableLocation[4:8])
		copy(b[0x2c:0x2e], freeBlocks[2:4])
		copy(b[0x2e:0x30], freeInodes[2:4])
		copy(b[0x30:0x32], usedirectories[2:4])
		copy(b[0x32:0x34], unusedInodes[2:4])
		copy(b[0x34:0x38], snapshotExclusionBitmapLocation[4:8])
		copy(b[0x38:0x3a], blockBitmapChecksum[2:4])
		copy(b[0x3a:0x3c], inodeBitmapChecksum[2:4])
	}

	checksum := groupDescriptorChecksum(b[0x0:0x40], hashSeed, gd.number, checksumType)
	binary.LittleEndian.PutUint16(b[0x1e:0x20], checksum)

	return b
}

func parseBlockGroupFlags(flags uint16) blockGroupFlags {
	f := blockGroupFlags{
		inodeTableZeroed:         blockGroupFlagInodeTableZeroed.included(flags),
		inodesUninitialized:      blockGroupFlagInodesUninitialized.included(flags),
		blockBitmapUninitialized: blockGroupFlagBlockBitmapUninitialized.included(flags),
	}

	return f
}

func (f *blockGroupFlags) toInt() uint16 {
	var (
		flags uint16
	)

	// compatible flags
	if f.inodeTableZeroed {
		flags |= uint16(blockGroupFlagInodeTableZeroed)
	}
	if f.inodesUninitialized {
		flags |= uint16(blockGroupFlagInodesUninitialized)
	}
	if f.blockBitmapUninitialized {
		flags |= uint16(blockGroupFlagBlockBitmapUninitialized)
	}
	return flags
}

// groupDescriptorChecksum calculate the checksum for a block group descriptor
// NOTE: we are assuming that the block group number is uint64, but we do not know that to be true
//
//	it might be uint32 or uint64, and it might be in BigEndian as opposed to LittleEndian
//	just have to start with this and see
//	we do know that the maximum number of block groups in 32-bit mode is 2^19, which must be uint32
//	and in 64-bit mode it is 2^51 which must be uint64
//	So we start with uint32 = [4]byte{} for regular mode and [8]byte{} for mod32
func groupDescriptorChecksum(b []byte, hashSeed uint32, groupNumber uint16, checksumType gdtChecksumType) uint16 {
	var checksum uint16

	numBytes := make([]byte, 4)
	binary.LittleEndian.PutUint16(numBytes, groupNumber)
	switch checksumType {
	case gdtChecksumNone:
		checksum = 0
	case gdtChecksumMetadata:
		// metadata checksum applies groupNumber to seed, then zeroes out checksum bytes from entire descriptor, then applies descriptor bytes
		crcResult := crc.CRC32c(hashSeed, numBytes)
		b2 := make([]byte, len(b))
		copy(b2, b)
		b2[0x1e] = 0
		b2[0x1f] = 0
		crcResult = crc.CRC32c(crcResult, b2)
		checksum = uint16(crcResult & 0xffff)
	case gdtChecksumGdt:
		hashSeed16 := uint16(hashSeed & 0xffff)
		crcResult := crc.CRC16(hashSeed16, numBytes)
		b2 := make([]byte, len(b))
		copy(b2, b)
		b2[0x1e] = 0
		b2[0x1f] = 0
		checksum = crc.CRC16(crcResult, b)
	}
	return checksum
}
