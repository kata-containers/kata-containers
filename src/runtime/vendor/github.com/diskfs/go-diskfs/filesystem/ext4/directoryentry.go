package ext4

import (
	"encoding/binary"
	"fmt"
)

// directoryFileType uses different constants than the file type property in the inode
type directoryFileType uint8

const (
	minDirEntryLength int = 12 // actually 9 for 1-byte file length, but must be multiple of 4 bytes
	maxDirEntryLength int = 263

	// directory file types
	dirFileTypeUnknown   directoryFileType = 0x0
	dirFileTypeRegular   directoryFileType = 0x1
	dirFileTypeDirectory directoryFileType = 0x2
	dirFileTypeCharacter directoryFileType = 0x3
	dirFileTypeBlock     directoryFileType = 0x4
	dirFileTypeFifo      directoryFileType = 0x5
	dirFileTypeSocket    directoryFileType = 0x6
	dirFileTypeSymlink   directoryFileType = 0x7
)

// directoryEntry is a single directory entry
type directoryEntry struct {
	inode    uint32
	filename string
	fileType directoryFileType
}

func (de *directoryEntry) equal(other *directoryEntry) bool {
	return de.inode == other.inode && de.filename == other.filename && de.fileType == other.fileType
}

func directoryEntryFromBytes(b []byte) (*directoryEntry, error) {
	if len(b) < minDirEntryLength {
		return nil, fmt.Errorf("directory entry of length %d is less than minimum %d", len(b), minDirEntryLength)
	}
	if len(b) > maxDirEntryLength {
		b = b[:maxDirEntryLength]
	}

	//nolint:gocritic // keep this here for future reference
	// length := binary.LittleEndian.Uint16(b[0x4:0x6])
	nameLength := b[0x6]
	name := b[0x8 : 0x8+nameLength]
	de := directoryEntry{
		inode:    binary.LittleEndian.Uint32(b[0x0:0x4]),
		fileType: directoryFileType(b[0x7]),
		filename: string(name),
	}
	return &de, nil
}

func directoryEntriesChecksumFromBytes(b []byte) (checksum uint32, err error) {
	if len(b) != minDirEntryLength {
		return checksum, fmt.Errorf("directory entry checksum of length %d is not required %d", len(b), minDirEntryLength)
	}
	inode := binary.LittleEndian.Uint32(b[0x0:0x4])
	if inode != 0 {
		return checksum, fmt.Errorf("directory entry checksum inode is not 0")
	}
	length := binary.LittleEndian.Uint16(b[0x4:0x6])
	if int(length) != minDirEntryLength {
		return checksum, fmt.Errorf("directory entry checksum length is not %d", minDirEntryLength)
	}
	nameLength := b[0x6]
	if nameLength != 0 {
		return checksum, fmt.Errorf("directory entry checksum name length is not 0")
	}
	fileType := b[0x7]
	if fileType != 0xde {
		return checksum, fmt.Errorf("directory entry checksum file type is not set to reserved 0xde")
	}
	return binary.LittleEndian.Uint32(b[0x8:0xc]), nil
}

// toBytes convert a directoryEntry to bytes. If isLast, then the size recorded is the number of bytes
// from beginning of directory entry to end of block, minus the amount left for the checksum.
func (de *directoryEntry) toBytes(withSize uint16) []byte {
	// it must be the header length + filename length rounded up to nearest multiple of 4
	nameLength := uint8(len(de.filename))
	entryLength := uint16(nameLength) + 8
	if leftover := entryLength % 4; leftover > 0 {
		entryLength += (4 - leftover)
	}

	if withSize > 0 {
		entryLength = withSize
	}
	b := make([]byte, entryLength)
	binary.LittleEndian.PutUint32(b[0x0:0x4], de.inode)
	binary.LittleEndian.PutUint16(b[0x4:0x6], entryLength)
	b[0x6] = nameLength
	b[0x7] = byte(de.fileType)
	copy(b[0x8:], de.filename)

	return b
}

func parseDirEntriesLinear(b []byte, withChecksums bool, blocksize, inodeNumber, inodeGeneration, checksumSeed uint32) ([]*directoryEntry, error) {
	// checksum if needed
	if withChecksums {
		var (
			newb                []byte
			checksumEntryOffset = int(blocksize) - minDirEntryLength
			checksumOffset      = int(blocksize) - 4
		)
		checksummer := directoryChecksummer(checksumSeed, inodeNumber, inodeGeneration)
		for i := 0; i < len(b); i += int(blocksize) {
			block := b[i : i+int(blocksize)]
			inBlockChecksum := block[checksumOffset:]
			block = block[:checksumEntryOffset]
			// save everything except the checksum
			newb = append(newb, block...)
			// checksum the entire block
			checksumValue := binary.LittleEndian.Uint32(inBlockChecksum)
			// checksum the block
			actualChecksum := checksummer(block)
			if actualChecksum != checksumValue {
				return nil, fmt.Errorf("directory block checksum mismatch: expected %x, got %x", checksumValue, actualChecksum)
			}
		}
		b = newb
	}

	// convert into directory entries
	entries := make([]*directoryEntry, 0, 4)
	count := 0
	for i := 0; i < len(b); count++ {
		// read the length of the entry
		length := binary.LittleEndian.Uint16(b[i+0x4 : i+0x6])
		de, err := directoryEntryFromBytes(b[i : i+int(length)])
		if err != nil {
			return nil, fmt.Errorf("failed to parse directory entry %d: %v", count, err)
		}
		entries = append(entries, de)
		i += int(length)
	}
	return entries, nil
}

// parseDirEntriesHashed parse hashed data blocks to get directory entries.
// If hashedName is 0, returns all directory entries; otherwise, returns a slice with a single entry with the given name.
func parseDirEntriesHashed(b []byte, depth uint8, node dxNode, blocksize uint32, withChecksums bool, inodeNumber, inodeGeneration, checksumSeed uint32) (dirEntries []*directoryEntry, err error) {
	for _, entry := range node.entries() {
		var (
			addDirEntries []*directoryEntry
			start         = entry.block * blocksize
			end           = start + blocksize
		)

		nextBlock := b[start:end]
		if depth == 0 {
			addDirEntries, err = parseDirEntriesLinear(nextBlock, withChecksums, blocksize, inodeNumber, inodeGeneration, checksumSeed)
			if err != nil {
				return nil, fmt.Errorf("error parsing linear directory entries: %w", err)
			}
		} else {
			// recursively parse the next level of the tree
			// read the next level down
			node, err := parseDirectoryTreeNode(nextBlock)
			if err != nil {
				return nil, fmt.Errorf("error parsing directory tree node: %w", err)
			}
			addDirEntries, err = parseDirEntriesHashed(b, depth-1, node, blocksize, withChecksums, inodeNumber, inodeGeneration, checksumSeed)
			if err != nil {
				return nil, fmt.Errorf("error parsing hashed directory entries: %w", err)
			}
		}
		dirEntries = append(dirEntries, addDirEntries...)
	}
	return dirEntries, nil
}
