package squashfs

import (
	"encoding/binary"
	"fmt"
	"math"
	"time"
)

const (
	superblockMagic        uint32 = 0x73717368
	superblockMajorVersion uint16 = 4
	superblockMinorVersion uint16 = 0
)

type compression uint16

const (
	compressionNone compression = 0
	compressionGzip compression = 1
	compressionLzma compression = 2
	compressionLzo  compression = 3
	compressionXz   compression = 4
	compressionLz4  compression = 5
	compressionZstd compression = 6
)

const (
	superblockSize = 96
)

type inodeRef struct {
	block  uint32
	offset uint16
}

func (i *inodeRef) toUint64() uint64 {
	var u uint64
	u |= (uint64(i.block) << 16)
	u |= uint64(i.offset)
	return u
}
func parseRootInode(u uint64) *inodeRef {
	i := &inodeRef{
		block:  uint32((u >> 16) & 0xffffffff),
		offset: uint16(u & 0xffff),
	}
	return i
}

type superblockFlags struct {
	uncompressedInodes    bool
	uncompressedData      bool
	uncompressedFragments bool
	noFragments           bool
	alwaysFragments       bool
	dedup                 bool
	exportable            bool
	uncompressedXattrs    bool
	noXattrs              bool
	compressorOptions     bool
	uncompressedIDs       bool
}

type superblock struct {
	inodes              uint32
	modTime             time.Time
	blocksize           uint32
	fragmentCount       uint32
	compression         compression
	idCount             uint16
	versionMajor        uint16
	versionMinor        uint16
	rootInode           *inodeRef
	size                uint64
	idTableStart        uint64
	xattrTableStart     uint64
	inodeTableStart     uint64
	directoryTableStart uint64
	fragmentTableStart  uint64
	exportTableStart    uint64
	superblockFlags
}

func (s *superblock) equal(a *superblock) bool {
	// to compare, need to extract the rootInode
	inodeEql := *a.rootInode == *s.rootInode
	s1 := &superblock{}
	a1 := &superblock{}
	*s1 = *s
	*a1 = *a
	s1.rootInode = nil
	a1.rootInode = nil
	modTime := time.Now()
	s1.modTime = modTime
	a1.modTime = modTime
	sblockEql := *s1 == *a1
	return inodeEql && sblockEql
}

func (s *superblockFlags) bytes() []byte {
	var flags uint16
	if s.uncompressedInodes {
		flags |= 0x0001
	}
	if s.uncompressedData {
		flags |= 0x0002
	}
	if s.uncompressedFragments {
		flags |= 0x0008
	}
	if s.noFragments {
		flags |= 0x0010
	}
	if s.alwaysFragments {
		flags |= 0x0020
	}
	if s.dedup {
		flags |= 0x0040
	}
	if s.exportable {
		flags |= 0x0080
	}
	if s.uncompressedXattrs {
		flags |= 0x0100
	}
	if s.noXattrs {
		flags |= 0x0200
	}
	if s.compressorOptions {
		flags |= 0x0400
	}
	if s.uncompressedIDs {
		flags |= 0x0800
	}
	b := make([]byte, 2)
	binary.LittleEndian.PutUint16(b, flags)
	return b
}
func parseFlags(b []byte) (*superblockFlags, error) {
	targetLength := 2
	if len(b) != targetLength {
		return nil, fmt.Errorf("received %d bytes instead of expected %d", len(b), targetLength)
	}
	flags := binary.LittleEndian.Uint16(b)
	s := &superblockFlags{
		uncompressedInodes:    flags&0x0001 == 0x0001,
		uncompressedData:      flags&0x0002 == 0x0002,
		uncompressedFragments: flags&0x0008 == 0x0008,
		noFragments:           flags&0x0010 == 0x0010,
		alwaysFragments:       flags&0x0020 == 0x0020,
		dedup:                 flags&0x0040 == 0x0040,
		exportable:            flags&0x0080 == 0x0080,
		uncompressedXattrs:    flags&0x0100 == 0x0100,
		noXattrs:              flags&0x0200 == 0x0200,
		compressorOptions:     flags&0x0400 == 0x0400,
		uncompressedIDs:       flags&0x0800 == 0x0800,
	}
	return s, nil
}

func (s *superblock) toBytes() []byte {
	b := make([]byte, superblockSize)
	binary.LittleEndian.PutUint32(b[0:4], superblockMagic)
	binary.LittleEndian.PutUint32(b[4:8], s.inodes)
	binary.LittleEndian.PutUint32(b[8:12], uint32(s.modTime.Unix()))
	binary.LittleEndian.PutUint32(b[12:16], s.blocksize)
	binary.LittleEndian.PutUint32(b[16:20], s.fragmentCount)
	binary.LittleEndian.PutUint16(b[20:22], uint16(s.compression))
	binary.LittleEndian.PutUint16(b[22:24], uint16(math.Log2(float64(s.blocksize))))
	copy(b[24:26], s.superblockFlags.bytes())
	binary.LittleEndian.PutUint16(b[26:28], s.idCount)
	binary.LittleEndian.PutUint16(b[28:30], superblockMajorVersion)
	binary.LittleEndian.PutUint16(b[30:32], superblockMinorVersion)
	binary.LittleEndian.PutUint64(b[32:40], s.rootInode.toUint64())
	binary.LittleEndian.PutUint64(b[40:48], s.size)
	binary.LittleEndian.PutUint64(b[48:56], s.idTableStart)
	binary.LittleEndian.PutUint64(b[56:64], s.xattrTableStart)
	binary.LittleEndian.PutUint64(b[64:72], s.inodeTableStart)
	binary.LittleEndian.PutUint64(b[72:80], s.directoryTableStart)
	binary.LittleEndian.PutUint64(b[80:88], s.fragmentTableStart)
	binary.LittleEndian.PutUint64(b[88:96], s.exportTableStart)
	return b
}
func parseSuperblock(b []byte) (*superblock, error) {
	if len(b) != superblockSize {
		return nil, fmt.Errorf("superblock had %d bytes instead of expected %d", len(b), superblockSize)
	}
	magic := binary.LittleEndian.Uint32(b[0:4])
	if magic != superblockMagic {
		return nil, fmt.Errorf("superblock had magic of %d instead of expected %d", magic, superblockMagic)
	}
	majorVersion := binary.LittleEndian.Uint16(b[28:30])
	minorVersion := binary.LittleEndian.Uint16(b[30:32])
	if majorVersion != superblockMajorVersion || minorVersion != superblockMinorVersion {
		return nil, fmt.Errorf("superblock version mismatch, received %d.%d instead of expected %d.%d", majorVersion, minorVersion, superblockMajorVersion, superblockMinorVersion)
	}

	blocksize := binary.LittleEndian.Uint32(b[12:16])
	blocklog := binary.LittleEndian.Uint16(b[22:24])
	expectedLog := uint16(math.Log2(float64(blocksize)))
	if expectedLog != blocklog {
		return nil, fmt.Errorf("superblock block log mismatch, actual %d expected %d", blocklog, expectedLog)
	}
	flags, err := parseFlags(b[24:26])
	if err != nil {
		return nil, fmt.Errorf("error parsing flags bytes: %v", err)
	}
	s := &superblock{
		inodes:              binary.LittleEndian.Uint32(b[4:8]),
		modTime:             time.Unix(int64(binary.LittleEndian.Uint32(b[8:12])), 0),
		blocksize:           blocksize,
		fragmentCount:       binary.LittleEndian.Uint32(b[16:20]),
		compression:         compression(binary.LittleEndian.Uint16(b[20:22])),
		idCount:             binary.LittleEndian.Uint16(b[26:28]),
		versionMajor:        binary.LittleEndian.Uint16(b[28:30]),
		versionMinor:        binary.LittleEndian.Uint16(b[30:32]),
		rootInode:           parseRootInode(binary.LittleEndian.Uint64(b[32:40])),
		size:                binary.LittleEndian.Uint64(b[40:48]),
		idTableStart:        binary.LittleEndian.Uint64(b[48:56]),
		xattrTableStart:     binary.LittleEndian.Uint64(b[56:64]),
		inodeTableStart:     binary.LittleEndian.Uint64(b[64:72]),
		directoryTableStart: binary.LittleEndian.Uint64(b[72:80]),
		fragmentTableStart:  binary.LittleEndian.Uint64(b[80:88]),
		exportTableStart:    binary.LittleEndian.Uint64(b[88:96]),
		superblockFlags:     *flags,
	}
	return s, nil
}
