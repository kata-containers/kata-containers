package ext4

import (
	"bytes"
	"encoding/binary"
	"fmt"
)

const (
	directoryHashTreeRootMinSize = 0x28
	directoryHashTreeNodeMinSize = 0x12
)

// Directory represents a single directory in an ext4 filesystem
type Directory struct {
	directoryEntry
	root    bool
	entries []*directoryEntry
}

// toBytes convert our entries to raw bytes. Provides checksum as well. Final returned byte slice will be a multiple of bytesPerBlock.
func (d *Directory) toBytes(bytesPerBlock uint32, checksumFunc checksumAppender) []byte {
	b := make([]byte, 0)
	var (
		previousLength int
		previousEntry  *directoryEntry
		lastEntryCount int
		block          []byte
	)
	if len(d.entries) == 0 {
		return b
	}
	lastEntryCount = len(d.entries) - 1
	for i, de := range d.entries {
		b2 := de.toBytes(0)
		switch {
		case len(block)+len(b2) > int(bytesPerBlock)-minDirEntryLength:
			// if adding this one will go past the end of the block, pad out the previous
			block = block[:len(block)-previousLength]
			previousB := previousEntry.toBytes(uint16(int(bytesPerBlock) - len(block) - minDirEntryLength))
			block = append(block, previousB...)
			// add the checksum
			block = checksumFunc(block)
			b = append(b, block...)
			// start a new block
			block = make([]byte, 0)
		case i == lastEntryCount:
			// if this is the last one, pad it out
			b2 = de.toBytes(uint16(int(bytesPerBlock) - len(block) - minDirEntryLength))
			block = append(block, b2...)
			// add the checksum
			block = checksumFunc(block)
			b = append(b, block...)
			// start a new block
			block = make([]byte, 0)
		default:
			block = append(block, b2...)
		}
		previousLength = len(b2)
		previousEntry = de
	}
	remainder := len(b) % int(bytesPerBlock)
	if remainder > 0 {
		extra := int(bytesPerBlock) - remainder
		zeroes := make([]byte, extra)
		b = append(b, zeroes...)
	}
	return b
}

type directoryHashEntry struct {
	hash  uint32
	block uint32
}

type dxNode interface {
	entries() []directoryHashEntry
}

type directoryHashNode struct {
	childEntries []directoryHashEntry
}

func (d *directoryHashNode) entries() []directoryHashEntry {
	return d.childEntries
}

type directoryHashRoot struct {
	inodeDir      uint32
	inodeParent   uint32
	hashVersion   hashVersion
	depth         uint8
	hashAlgorithm hashAlgorithm
	childEntries  []directoryHashEntry
	dotEntry      *directoryEntry
	dotDotEntry   *directoryEntry
}

func (d *directoryHashRoot) entries() []directoryHashEntry {
	return d.childEntries
}

// parseDirectoryTreeRoot parses the directory hash tree root from the given byte slice. Reads only the root node.
func parseDirectoryTreeRoot(b []byte, largeDir bool) (node *directoryHashRoot, err error) {
	// min size
	if len(b) < directoryHashTreeRootMinSize {
		return nil, fmt.Errorf("directory hash tree root is too small")
	}

	// dot parameters
	dotInode := binary.LittleEndian.Uint32(b[0x0:0x4])
	dotSize := binary.LittleEndian.Uint16(b[0x4:0x6])
	if dotSize != 12 {
		return nil, fmt.Errorf("directory hash tree root dot size is %d and not 12", dotSize)
	}
	dotNameSize := b[0x6]
	if dotNameSize != 1 {
		return nil, fmt.Errorf("directory hash tree root dot name length is %d and not 1", dotNameSize)
	}
	dotFileType := directoryFileType(b[0x7])
	if dotFileType != dirFileTypeDirectory {
		return nil, fmt.Errorf("directory hash tree root dot file type is %d and not %v", dotFileType, dirFileTypeDirectory)
	}
	dotName := b[0x8:0xc]
	if !bytes.Equal(dotName, []byte{'.', 0, 0, 0}) {
		return nil, fmt.Errorf("directory hash tree root dot name is %s and not '.'", dotName)
	}

	// dotdot parameters
	dotdotInode := binary.LittleEndian.Uint32(b[0xc:0x10])
	dotdotNameSize := b[0x12]
	if dotdotNameSize != 2 {
		return nil, fmt.Errorf("directory hash tree root dotdot name length is %d and not 2", dotdotNameSize)
	}
	dotdotFileType := directoryFileType(b[0x13])
	if dotdotFileType != dirFileTypeDirectory {
		return nil, fmt.Errorf("directory hash tree root dotdot file type is %d and not %v", dotdotFileType, dirFileTypeDirectory)
	}
	dotdotName := b[0x14:0x18]
	if !bytes.Equal(dotdotName, []byte{'.', '.', 0, 0}) {
		return nil, fmt.Errorf("directory hash tree root dotdot name is %s and not '..'", dotdotName)
	}

	treeInformation := b[0x1d]
	if treeInformation != 8 {
		return nil, fmt.Errorf("directory hash tree root tree information is %d and not 8", treeInformation)
	}
	treeDepth := b[0x1e]
	// there are maximums for this
	maxTreeDepth := uint8(2)
	if largeDir {
		maxTreeDepth = 3
	}
	if treeDepth > maxTreeDepth {
		return nil, fmt.Errorf("directory hash tree root tree depth is %d and not between 0 and %d", treeDepth, maxTreeDepth)
	}

	dxEntriesCount := binary.LittleEndian.Uint16(b[0x22:0x24])

	node = &directoryHashRoot{
		inodeDir:      binary.LittleEndian.Uint32(b[0x0:0x4]),
		inodeParent:   binary.LittleEndian.Uint32(b[0xC:0x10]),
		hashAlgorithm: hashAlgorithm(b[0x1c]), // what hashing algorithm is used?
		depth:         treeDepth,
		childEntries:  make([]directoryHashEntry, 0, int(dxEntriesCount)),
		dotEntry: &directoryEntry{
			inode:    dotInode,
			fileType: dotFileType,
			filename: ".",
		},
		dotDotEntry: &directoryEntry{
			inode:    dotdotInode,
			fileType: dotdotFileType,
			filename: "..",
		},
	}

	// remove 1, because the count includes the one in the dx_root itself
	node.childEntries = append(node.childEntries, directoryHashEntry{hash: 0, block: binary.LittleEndian.Uint32(b[0x24:0x28])})
	for i := 0; i < int(dxEntriesCount)-1; i++ {
		entryOffset := 0x28 + (i * 8)
		hash := binary.LittleEndian.Uint32(b[entryOffset : entryOffset+4])
		block := binary.LittleEndian.Uint32(b[entryOffset+4 : entryOffset+8])
		node.childEntries = append(node.childEntries, directoryHashEntry{hash: hash, block: block})
	}

	return node, nil
}

// parseDirectoryTreeNode parses an internal directory hash tree node from the given byte slice. Reads only the node.
func parseDirectoryTreeNode(b []byte) (node *directoryHashNode, err error) {
	// min size
	if len(b) < directoryHashTreeNodeMinSize {
		return nil, fmt.Errorf("directory hash tree root is too small")
	}

	dxEntriesCount := binary.LittleEndian.Uint16(b[0xa:0xc])

	node = &directoryHashNode{
		childEntries: make([]directoryHashEntry, 0, int(dxEntriesCount)),
	}
	node.childEntries = append(node.childEntries, directoryHashEntry{hash: 0, block: binary.LittleEndian.Uint32(b[0xc:0x10])})
	for i := 0; i < int(dxEntriesCount)-1; i++ {
		entryOffset := 0x10 + (i * 8)
		hash := binary.LittleEndian.Uint32(b[entryOffset : entryOffset+4])
		block := binary.LittleEndian.Uint32(b[entryOffset+4 : entryOffset+8])
		node.childEntries = append(node.childEntries, directoryHashEntry{hash: hash, block: block})
	}

	return node, nil
}
