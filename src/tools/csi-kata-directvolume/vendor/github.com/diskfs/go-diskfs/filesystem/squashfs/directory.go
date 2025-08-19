package squashfs

import (
	"encoding/binary"
	"fmt"
)

const (
	maxDirEntries   = 256
	dirHeaderSize   = 12
	dirEntryMinSize = 8
	dirNameMaxSize  = 256
)

// directory represents a contiguous directory on disk, composed of one header
// and one or more entries under that header, i.e. directoryEntryRaw. An entire
// directory may be composed of one or more of these "directory", depending
// on how many headers it requires
type directory struct {
	inodeIndex uint32
	entries    []*directoryEntryRaw
}

type directoryEntryRaw struct {
	offset         uint16
	inodeNumber    uint32
	inodeType      inodeType
	name           string
	isSubdirectory bool
	startBlock     uint32
}

func (d *directoryEntryRaw) toBytes(inodeIndex uint32) []byte {
	b := make([]byte, 8)
	nameBytes := []byte(d.name)
	binary.LittleEndian.PutUint16(b[0:2], d.offset)
	binary.LittleEndian.PutUint16(b[2:4], uint16(d.inodeNumber-inodeIndex))
	binary.LittleEndian.PutUint16(b[4:6], uint16(d.inodeType))
	binary.LittleEndian.PutUint16(b[6:8], uint16(len(nameBytes)-1))
	b = append(b, nameBytes...)
	return b
}

type directoryHeader struct {
	count      uint32
	startBlock uint32
	inode      uint32
}

type directoryEntryGroup struct {
	header  *directoryHeader
	entries []*directoryEntryRaw
}

// parse raw bytes of a directory to get the contents
func parseDirectory(b []byte) (*directory, error) {
	// must have at least one header
	if _, err := parseDirectoryHeader(b); err != nil {
		return nil, fmt.Errorf("could not parse directory header: %v", err)
	}
	entries := make([]*directoryEntryRaw, 0)
	for pos := 0; pos+dirHeaderSize < len(b); {
		directoryHeader, err := parseDirectoryHeader(b[pos:])
		if err != nil {
			return nil, fmt.Errorf("could not parse directory header: %v", err)
		}
		if directoryHeader.count+1 > maxDirEntries {
			return nil, fmt.Errorf("corrupted directory, had %d entries instead of max %d", directoryHeader.count+1, maxDirEntries)
		}
		pos += dirHeaderSize
		for count := uint32(0); count < directoryHeader.count; count++ {
			entry, size, err := parseDirectoryEntry(b[pos:], directoryHeader.inode)
			if err != nil {
				return nil, fmt.Errorf("unable to parse entry at position %d: %v", pos, err)
			}
			entry.startBlock = directoryHeader.startBlock
			entries = append(entries, entry)
			// increment the position
			pos += size
		}
	}

	return &directory{
		entries: entries,
	}, nil
}

func (d *directory) toBytes(in uint32) []byte {
	// need to group these into chunks that would share a header
	var (
		b      []byte
		groups []*directoryEntryGroup
		group  *directoryEntryGroup
	)
	for _, e := range d.entries {
		// we need a new header if one of the following:
		// - we don't have one yet
		// - inode block changes
		// - inode offset > +/- 32k from the inode in the header,
		if group == nil || group.header.startBlock != e.startBlock {
			group = &directoryEntryGroup{
				header: &directoryHeader{
					startBlock: e.startBlock,
					inode:      in,
				},
			}
			groups = append(groups, group)
		}
		group.header.count++
		group.entries = append(group.entries, e)
	}
	// now convert all of the chunks to bytes
	for _, group := range groups {
		b = append(b, group.header.toBytes()...)
		for _, e := range group.entries {
			b = append(b, e.toBytes(in)...)
		}
	}
	return b
}

func (d *directory) equal(b *directory) bool {
	if d == nil && b == nil {
		return true
	}
	if (d == nil && b != nil) || (d != nil && b == nil) {
		return false
	}
	// entries
	if len(d.entries) != len(b.entries) {
		return false
	}
	for i, e := range d.entries {
		if *e != *b.entries[i] {
			return false
		}
	}
	return true
}

// parse the header of a directory
func parseDirectoryHeader(b []byte) (*directoryHeader, error) {
	if len(b) < dirHeaderSize {
		return nil, fmt.Errorf("header was %d bytes, less than minimum %d", len(b), dirHeaderSize)
	}
	return &directoryHeader{
		count:      binary.LittleEndian.Uint32(b[0:4]) + 1,
		startBlock: binary.LittleEndian.Uint32(b[4:8]),
		inode:      binary.LittleEndian.Uint32(b[8:12]),
	}, nil
}
func (d *directoryHeader) toBytes() []byte {
	b := make([]byte, dirHeaderSize)

	binary.LittleEndian.PutUint32(b[0:4], d.count-1)
	binary.LittleEndian.PutUint32(b[4:8], d.startBlock)
	binary.LittleEndian.PutUint32(b[8:12], d.inode)
	return b
}

// parse a raw directory entry
func parseDirectoryEntry(b []byte, in uint32) (*directoryEntryRaw, int, error) {
	// ensure we have enough bytes to parse
	if len(b) < dirEntryMinSize {
		return nil, 0, fmt.Errorf("directory entry was %d bytes, less than minimum %d", len(b), dirEntryMinSize)
	}

	offset := binary.LittleEndian.Uint16(b[0:2])
	inode := uint32(binary.LittleEndian.Uint16(b[2:4])) + in
	entryType := binary.LittleEndian.Uint16(b[4:6])
	nameSize := binary.LittleEndian.Uint16(b[6:8])
	realNameSize := nameSize + 1

	// make sure name is legitimate size
	if nameSize > dirNameMaxSize {
		return nil, 0, fmt.Errorf("name size was %d bytes, greater than maximum %d", nameSize, dirNameMaxSize)
	}
	if int(realNameSize+dirEntryMinSize) > len(b) {
		return nil, 0, fmt.Errorf("dir entry plus size of name is %d, larger than available bytes %d", nameSize+dirEntryMinSize, len(b))
	}

	// read in the name
	name := string(b[8 : 8+realNameSize])
	iType := inodeType(entryType)
	return &directoryEntryRaw{
		offset:         offset,
		inodeNumber:    inode,
		inodeType:      iType,
		isSubdirectory: iType == inodeBasicDirectory || iType == inodeExtendedDirectory,
		name:           name,
	}, int(8 + realNameSize), nil
}
