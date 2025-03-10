package squashfs

import (
	"encoding/binary"
	"fmt"
	"os"
	"time"
)

type inodeType uint16

const (
	inodeBasicDirectory    inodeType = 1
	inodeBasicFile         inodeType = 2
	inodeBasicSymlink      inodeType = 3
	inodeBasicBlock        inodeType = 4
	inodeBasicChar         inodeType = 5
	inodeBasicFifo         inodeType = 6
	inodeBasicSocket       inodeType = 7
	inodeExtendedDirectory inodeType = 8
	inodeExtendedFile      inodeType = 9
	inodeExtendedSymlink   inodeType = 10
	inodeExtendedBlock     inodeType = 11
	inodeExtendedChar      inodeType = 12
	inodeExtendedFifo      inodeType = 13
	inodeExtendedSocket    inodeType = 14
)

const (
	inodeHeaderSize              = 16
	inodeDirectoryIndexEntrySize = 3*4 + 1
)

type inodeHeader struct {
	inodeType inodeType
	uidIdx    uint16
	gidIdx    uint16
	modTime   time.Time
	index     uint32
	mode      os.FileMode
	// permissions
}
type inodeBody interface {
	toBytes() []byte
	size() int64
	xattrIndex() (uint32, bool)
	equal(inodeBody) bool
}
type inode interface {
	toBytes() []byte
	equal(inode) bool
	size() int64
	inodeType() inodeType
	index() uint32
	getHeader() *inodeHeader
	getBody() inodeBody
}
type inodeImpl struct {
	header *inodeHeader
	body   inodeBody
}

func (i *inodeImpl) equal(o inode) bool {
	other, ok := o.(*inodeImpl)
	if !ok {
		return false
	}
	if (i.header == nil && other.header != nil) || (i.header != nil && other.header == nil) || (i.header != nil && other.header != nil && *i.header != *other.header) {
		return false
	}
	if (i.body == nil && other.body != nil) || (i.body != nil && other.body == nil) || (i.body != nil && other.body != nil && !i.body.equal(other.body)) {
		return false
	}
	return true
}
func (i *inodeImpl) toBytes() []byte {
	h := i.header.toBytes()
	b := i.body.toBytes()
	return append(h, b...)
}
func (i *inodeImpl) inodeType() inodeType {
	return i.header.inodeType
}
func (i *inodeImpl) index() uint32 {
	return i.header.index
}

// Size return the size of the item reflected by this inode, if it supports it
func (i *inodeImpl) size() int64 {
	return i.body.size()
}

func (i *inodeImpl) getHeader() *inodeHeader {
	return i.header
}
func (i *inodeImpl) getBody() inodeBody {
	return i.body
}

func (i *inodeHeader) toBytes() []byte {
	b := make([]byte, inodeHeaderSize)
	binary.LittleEndian.PutUint16(b[0:2], uint16(i.inodeType))
	binary.LittleEndian.PutUint16(b[2:4], uint16(i.mode))
	binary.LittleEndian.PutUint16(b[4:6], i.uidIdx)
	binary.LittleEndian.PutUint16(b[6:8], i.gidIdx)
	binary.LittleEndian.PutUint32(b[8:12], uint32(i.modTime.Unix()))
	binary.LittleEndian.PutUint32(b[12:16], i.index)
	return b
}
func parseInodeHeader(b []byte) (*inodeHeader, error) {
	target := inodeHeaderSize
	if len(b) < target {
		return nil, fmt.Errorf("received only %d bytes instead of minimum %d", len(b), target)
	}
	i := &inodeHeader{
		inodeType: inodeType(binary.LittleEndian.Uint16(b[0:2])),
		mode:      os.FileMode(binary.LittleEndian.Uint16(b[2:4])),
		uidIdx:    binary.LittleEndian.Uint16(b[4:6]),
		gidIdx:    binary.LittleEndian.Uint16(b[6:8]),
		modTime:   time.Unix(int64(binary.LittleEndian.Uint32(b[8:12])), 0),
		index:     binary.LittleEndian.Uint32(b[12:16]),
	}
	return i, nil
}

// blockdata used by files and directories
type blockData struct {
	size       uint32
	compressed bool
}

func (b *blockData) toUint32() uint32 {
	u := b.size
	if !b.compressed {
		u |= (1 << 24)
	}
	return u
}
func parseBlockData(u uint32) *blockData {
	var mask uint32 = 1 << 24
	return &blockData{
		compressed: u&mask != mask,
		size:       u & 0x00ffffff,
	}
}
func parseFileBlockSizes(b []byte, blockListSize int) []*blockData {
	blocks := make([]*blockData, 0, blockListSize)
	for j := 0; j < blockListSize && j < len(b); j++ {
		blocks = append(blocks, parseBlockData(binary.LittleEndian.Uint32(b[4*j:4*j+4])))
	}
	return blocks
}

// inodeTypeToSize return the minimum size of the inode body including the header
func inodeTypeToSize(i inodeType) int {
	size := inodeTypeToBodySize(i)
	if size != 0 {
		size += inodeHeaderSize
	}
	return size
}

// inodeTypeToBodySize return the minimum size of the inode body not including the header
func inodeTypeToBodySize(i inodeType) int {
	var size int
	switch i {
	case inodeBasicDirectory:
		size = 16
	case inodeExtendedDirectory:
		size = 24
	case inodeBasicFile:
		size = 16
	case inodeExtendedFile:
		size = 40
	case inodeBasicChar:
		size = 8
	case inodeExtendedChar:
		size = 12
	case inodeBasicFifo:
		size = 4
	case inodeExtendedFifo:
		size = 8
	case inodeBasicSymlink:
		size = 9
	case inodeExtendedSymlink:
		size = 13
	case inodeBasicBlock:
		size = 8
	case inodeExtendedBlock:
		size = 12
	case inodeBasicSocket:
		size = 4
	case inodeExtendedSocket:
		size = 8
	default:
		return 0
	}
	return size
}

/*
  All of our 14 inode types
*/
// basicDirectory
type basicDirectory struct {
	startBlock       uint32
	links            uint32
	fileSize         uint16
	offset           uint16
	parentInodeIndex uint32
}

func (i basicDirectory) toBytes() []byte {
	b := make([]byte, 16)
	binary.LittleEndian.PutUint32(b[0:4], i.startBlock)
	binary.LittleEndian.PutUint32(b[4:8], i.links)
	binary.LittleEndian.PutUint16(b[8:10], i.fileSize)
	binary.LittleEndian.PutUint16(b[10:12], i.offset)
	binary.LittleEndian.PutUint32(b[12:16], i.parentInodeIndex)
	return b
}
func (i basicDirectory) size() int64 {
	return int64(i.fileSize)
}
func (i basicDirectory) xattrIndex() (uint32, bool) {
	return 0, false
}
func (i basicDirectory) equal(o inodeBody) bool {
	oi, ok := o.(basicDirectory)
	if !ok {
		return false
	}
	if i != oi {
		return false
	}
	return true
}
func parseBasicDirectory(b []byte) (*basicDirectory, error) {
	target := 16
	if len(b) < target {
		return nil, fmt.Errorf("received %d bytes, fewer than minimum %d", len(b), target)
	}
	d := &basicDirectory{
		startBlock:       binary.LittleEndian.Uint32(b[0:4]),
		links:            binary.LittleEndian.Uint32(b[4:8]),
		fileSize:         binary.LittleEndian.Uint16(b[8:10]),
		offset:           binary.LittleEndian.Uint16(b[10:12]),
		parentInodeIndex: binary.LittleEndian.Uint32(b[12:16]),
	}
	return d, nil
}

// directoryIndex
type directoryIndex struct {
	index uint32
	block uint32
	size  uint32
	name  byte
}

// extendedDirectory
type extendedDirectory struct {
	links            uint32
	fileSize         uint32
	startBlock       uint32
	parentInodeIndex uint32
	indexCount       uint16
	offset           uint16
	xAttrIndex       uint32
	indexes          []*directoryIndex
}

func (i extendedDirectory) toBytes() []byte {
	b := make([]byte, 24)
	binary.LittleEndian.PutUint32(b[0:4], i.links)
	binary.LittleEndian.PutUint32(b[4:8], i.fileSize)
	binary.LittleEndian.PutUint32(b[8:12], i.startBlock)
	binary.LittleEndian.PutUint32(b[12:16], i.parentInodeIndex)
	binary.LittleEndian.PutUint16(b[16:18], i.indexCount)
	binary.LittleEndian.PutUint16(b[18:20], i.offset)
	binary.LittleEndian.PutUint32(b[20:24], i.xAttrIndex)
	return b
}
func (i extendedDirectory) size() int64 {
	return int64(i.fileSize)
}
func (i extendedDirectory) xattrIndex() (uint32, bool) {
	return i.xAttrIndex, i.xAttrIndex != noXattrInodeFlag
}
func (i extendedDirectory) equal(o inodeBody) bool {
	oi, ok := o.(extendedDirectory)
	if !ok {
		return false
	}
	if len(i.indexes) != len(oi.indexes) {
		return false
	}
	for c, elm := range i.indexes {
		if *elm != *oi.indexes[c] {
			return false
		}
	}
	return i.links == oi.links && i.fileSize == oi.fileSize && i.startBlock == oi.startBlock &&
		i.parentInodeIndex == oi.parentInodeIndex && i.indexCount == oi.indexCount &&
		i.offset == oi.offset && i.xAttrIndex == oi.xAttrIndex
}

func parseExtendedDirectory(b []byte) (*extendedDirectory, int, error) {
	var (
		target = 24
		extra  int
	)
	if len(b) < target {
		return nil, 0, fmt.Errorf("received %d bytes, fewer than minimum %d", len(b), target)
	}
	d := &extendedDirectory{
		links:            binary.LittleEndian.Uint32(b[0:4]),
		fileSize:         binary.LittleEndian.Uint32(b[4:8]),
		startBlock:       binary.LittleEndian.Uint32(b[8:12]),
		parentInodeIndex: binary.LittleEndian.Uint32(b[12:16]),
		indexCount:       binary.LittleEndian.Uint16(b[16:18]),
		offset:           binary.LittleEndian.Uint16(b[18:20]),
		xAttrIndex:       binary.LittleEndian.Uint32(b[20:24]),
	}
	// see how many other bytes we need to read for directory indexes
	//
	// each entry in indexes is a struct squashfs_dir_index, which is:
	// struct squashfs_dir_index {
	//      unsigned int            index;
	//      unsigned int            start_block;
	//      unsigned int            size;
	//      unsigned char           name[0];
	// };
	// so each is 4 int + 1 char
	extra = int(d.indexCount) * inodeDirectoryIndexEntrySize
	// do we have enough data left to read those?
	if len(b[target:]) >= extra {
		indexes, err := parseDirectoryIndexes(b[target:target+extra], int(d.indexCount))
		if err != nil {
			return d, 0, err
		}
		d.indexes = indexes
		extra = 0
	}

	return d, extra, nil
}

// parseDirectoryIndexes parse count directoryIndex from the given byte data
func parseDirectoryIndexes(b []byte, count int) ([]*directoryIndex, error) {
	var (
		indexes      []*directoryIndex
		expectedSize = count * inodeDirectoryIndexEntrySize
	)
	if len(b) < expectedSize {
		return nil, fmt.Errorf("expected at least %d bytes, received only %d", expectedSize, len(b))
	}
	for i := 0; i < len(b); i += inodeDirectoryIndexEntrySize {
		indexes = append(indexes, &directoryIndex{
			index: binary.LittleEndian.Uint32(b[0:4]),
			block: binary.LittleEndian.Uint32(b[4:8]),
			size:  binary.LittleEndian.Uint32(b[8:12]),
			name:  b[12],
		})
	}
	return indexes, nil
}

// basicFile
type basicFile struct {
	startBlock         uint32 // block count from the start of the data section where data for this file is stored
	fragmentBlockIndex uint32
	fragmentOffset     uint32
	fileSize           uint32
	blockSizes         []*blockData
}

func (i basicFile) equal(o inodeBody) bool {
	oi, ok := o.(basicFile)
	if !ok {
		return false
	}
	if len(i.blockSizes) != len(oi.blockSizes) {
		return false
	}
	for i, b := range i.blockSizes {
		if (b == nil && oi.blockSizes[i] == nil) || (b != nil && oi.blockSizes[i] == nil) {
			return false
		}
		if *b != *oi.blockSizes[i] {
			return false
		}
	}
	return i.startBlock == oi.startBlock && i.fragmentOffset == oi.fragmentOffset && i.fragmentBlockIndex == oi.fragmentBlockIndex && i.fileSize == oi.fileSize
}

func (i basicFile) toBytes() []byte {
	b := make([]byte, 16+4*len(i.blockSizes))
	binary.LittleEndian.PutUint32(b[0:4], i.startBlock)
	binary.LittleEndian.PutUint32(b[4:8], i.fragmentBlockIndex)
	binary.LittleEndian.PutUint32(b[8:12], i.fragmentOffset)
	binary.LittleEndian.PutUint32(b[12:16], i.fileSize)
	for j, e := range i.blockSizes {
		binary.LittleEndian.PutUint32(b[16+j*4:16+j*4+4], e.toUint32())
	}
	return b
}
func (i basicFile) size() int64 {
	return int64(i.fileSize)
}
func (i basicFile) xattrIndex() (uint32, bool) {
	return 0, false
}
func (i basicFile) toExtended() extendedFile {
	return extendedFile{
		startBlock:         uint64(i.startBlock),
		fileSize:           uint64(i.fileSize),
		sparse:             0,
		links:              0,
		fragmentBlockIndex: i.fragmentBlockIndex,
		fragmentOffset:     i.fragmentOffset,
		xAttrIndex:         0,
		blockSizes:         i.blockSizes,
	}
}
func parseBasicFile(b []byte, blocksize int) (*basicFile, int, error) {
	var (
		target = 16
		extra  int
	)
	if len(b) < target {
		return nil, 0, fmt.Errorf("received %d bytes, fewer than minimum %d", len(b), target)
	}
	fileSize := binary.LittleEndian.Uint32(b[12:16])
	d := &basicFile{
		startBlock:         binary.LittleEndian.Uint32(b[0:4]),
		fragmentBlockIndex: binary.LittleEndian.Uint32(b[4:8]),
		fragmentOffset:     binary.LittleEndian.Uint32(b[8:12]),
		fileSize:           fileSize,
	}
	// see how many other bytes we need to read
	blockListSize := int(d.fileSize / uint32(blocksize))
	if d.fileSize%uint32(blocksize) > 0 && d.fragmentBlockIndex == 0xffffffff {
		blockListSize++
	}
	// do we have enough data left to read those?
	extra = blockListSize * 4
	if len(b[16:]) >= extra {
		d.blockSizes = parseFileBlockSizes(b[16:], blockListSize)
		extra = 0
	}

	return d, extra, nil
}

// extendedFile
type extendedFile struct {
	startBlock         uint64
	fileSize           uint64
	sparse             uint64
	links              uint32
	fragmentBlockIndex uint32
	fragmentOffset     uint32
	xAttrIndex         uint32
	blockSizes         []*blockData
}

func (i extendedFile) equal(o inodeBody) bool {
	oi, ok := o.(extendedFile)
	if !ok {
		return false
	}
	if len(i.blockSizes) != len(oi.blockSizes) {
		return false
	}
	for i, b := range i.blockSizes {
		if (b == nil && oi.blockSizes[i] == nil) || (b != nil && oi.blockSizes[i] == nil) {
			return false
		}
		if *b != *oi.blockSizes[i] {
			return false
		}
	}
	return i.startBlock == oi.startBlock &&
		i.fragmentOffset == oi.fragmentOffset &&
		i.fragmentBlockIndex == oi.fragmentBlockIndex &&
		i.fileSize == oi.fileSize &&
		i.sparse == oi.sparse &&
		i.links == oi.links &&
		i.xAttrIndex == oi.xAttrIndex
}

func (i extendedFile) toBytes() []byte {
	b := make([]byte, 40+4*len(i.blockSizes))
	binary.LittleEndian.PutUint64(b[0:8], i.startBlock)
	binary.LittleEndian.PutUint64(b[8:16], i.fileSize)
	binary.LittleEndian.PutUint64(b[16:24], i.sparse)
	binary.LittleEndian.PutUint32(b[24:28], i.links)
	binary.LittleEndian.PutUint32(b[28:32], i.fragmentBlockIndex)
	binary.LittleEndian.PutUint32(b[32:36], i.fragmentOffset)
	binary.LittleEndian.PutUint32(b[36:40], i.xAttrIndex)
	for j, e := range i.blockSizes {
		binary.LittleEndian.PutUint32(b[40+j*4:40+j*4+4], e.toUint32())
	}
	return b
}
func (i extendedFile) size() int64 {
	return int64(i.fileSize)
}
func (i extendedFile) xattrIndex() (uint32, bool) {
	return i.xAttrIndex, i.xAttrIndex != noXattrInodeFlag
}

func parseExtendedFile(b []byte, blocksize int) (*extendedFile, int, error) {
	var (
		target = 40
		extra  int
	)
	if len(b) < target {
		return nil, 0, fmt.Errorf("received %d bytes instead of expected minimal %d", len(b), target)
	}
	fileSize := binary.LittleEndian.Uint64(b[8:16])
	d := &extendedFile{
		startBlock:         binary.LittleEndian.Uint64(b[0:8]),
		fileSize:           fileSize,
		sparse:             binary.LittleEndian.Uint64(b[16:24]),
		links:              binary.LittleEndian.Uint32(b[24:28]),
		fragmentBlockIndex: binary.LittleEndian.Uint32(b[28:32]),
		fragmentOffset:     binary.LittleEndian.Uint32(b[32:36]),
		xAttrIndex:         binary.LittleEndian.Uint32(b[36:40]),
	}
	// see how many other bytes we need to read
	blockListSize := int(d.fileSize / uint64(blocksize))
	if d.fileSize%uint64(blocksize) > 0 && d.fragmentBlockIndex == 0xffffffff {
		blockListSize++
	}
	// do we have enough data left to read those?
	extra = blockListSize * 4
	if len(b[40:]) >= extra {
		d.blockSizes = parseFileBlockSizes(b[40:], blockListSize)
		extra = 0
	}
	return d, extra, nil
}

// basicSymlink
type basicSymlink struct {
	links  uint32
	target string
}

func (i basicSymlink) toBytes() []byte {
	b := make([]byte, 8)
	binary.LittleEndian.PutUint32(b[0:4], i.links)
	binary.LittleEndian.PutUint32(b[4:8], uint32(len(i.target)))
	b = append(b, []byte(i.target)...)
	return b
}
func (i basicSymlink) size() int64 {
	return 0
}
func (i basicSymlink) xattrIndex() (uint32, bool) {
	return 0, false
}

func (i basicSymlink) equal(o inodeBody) bool {
	oi, ok := o.(basicSymlink)
	if !ok {
		return false
	}
	if i != oi {
		return false
	}
	return true
}

func parseBasicSymlink(b []byte) (*basicSymlink, int, error) {
	var (
		target = 8
		extra  int
	)
	if len(b) < target {
		return nil, 0, fmt.Errorf("received %d bytes instead of expected minimal %d", len(b), target)
	}
	s := &basicSymlink{
		links: binary.LittleEndian.Uint32(b[0:4]),
	}
	extra = int(binary.LittleEndian.Uint32(b[4:8]))
	if len(b[target:]) >= extra {
		s.target = string(b[8 : 8+extra])
		extra = 0
	}
	return s, extra, nil
}

// extendedSymlink
type extendedSymlink struct {
	links      uint32
	target     string
	xAttrIndex uint32
}

func (i extendedSymlink) toBytes() []byte {
	b := make([]byte, 8)
	binary.LittleEndian.PutUint32(b[0:4], i.links)
	binary.LittleEndian.PutUint32(b[4:8], uint32(len(i.target)))
	b = append(b, []byte(i.target)...)
	b2 := make([]byte, 4)
	binary.LittleEndian.PutUint32(b2[0:4], i.xAttrIndex)
	b = append(b, b2...)
	return b
}
func (i extendedSymlink) size() int64 {
	return 0
}
func (i extendedSymlink) xattrIndex() (uint32, bool) {
	return i.xAttrIndex, i.xAttrIndex != noXattrInodeFlag
}

func (i extendedSymlink) equal(o inodeBody) bool {
	oi, ok := o.(extendedSymlink)
	if !ok {
		return false
	}
	if i != oi {
		return false
	}
	return true
}

func parseExtendedSymlink(b []byte) (*extendedSymlink, int, error) {
	var (
		target = 8
		extra  int
	)
	if len(b) < target {
		return nil, 0, fmt.Errorf("received %d bytes instead of expected minimal %d", len(b), target)
	}
	s := &extendedSymlink{
		links: binary.LittleEndian.Uint32(b[0:4]),
	}
	targetSize := int(binary.LittleEndian.Uint32(b[4:8]))
	// account for the synlink target, plus 4 bytes for the xattr index after it
	extra = targetSize + 4
	if len(b) >= extra+target {
		s.target = string(b[target : target+targetSize])
		s.xAttrIndex = binary.LittleEndian.Uint32(b[target+targetSize : target+targetSize+4])
		extra = 0
	}
	return s, extra, nil
}

type basicDevice struct {
	links uint32
	major uint32
	minor uint32
}

func (i basicDevice) toBytes() []byte {
	b := make([]byte, 8)
	var devNum = (i.major << 8) | (i.minor & 0xff) | ((i.minor & 0xfff00) << 12)

	binary.LittleEndian.PutUint32(b[0:4], i.links)
	binary.LittleEndian.PutUint32(b[4:8], devNum)
	return b
}
func (i basicDevice) size() int64 {
	return 0
}
func (i basicDevice) xattrIndex() (uint32, bool) {
	return 0, false
}

func (i basicDevice) equal(o inodeBody) bool {
	oi, ok := o.(basicDevice)
	if !ok {
		return false
	}
	if i != oi {
		return false
	}
	return true
}
func parseBasicDevice(b []byte) (*basicDevice, error) {
	target := 8
	if len(b) < target {
		return nil, fmt.Errorf("received %d bytes instead of expected %d", len(b), target)
	}
	devNum := binary.LittleEndian.Uint32(b[4:8])
	s := &basicDevice{
		links: binary.LittleEndian.Uint32(b[0:4]),
		major: (devNum & 0xfff00) >> 8,
		minor: (devNum & 0xff) | ((devNum >> 12) & 0xfff00),
	}
	return s, nil
}

// basicBlock
type basicBlock struct {
	basicDevice
}
type basicChar struct {
	basicDevice
}

type extendedDevice struct {
	links      uint32
	major      uint32
	minor      uint32
	xAttrIndex uint32
}

func (i extendedDevice) toBytes() []byte {
	// easiest to use the basic one
	basic := &basicDevice{
		links: i.links,
		major: i.major,
		minor: i.minor,
	}
	b := basic.toBytes()
	b2 := make([]byte, 4)
	binary.LittleEndian.PutUint32(b2[0:4], i.xAttrIndex)
	b = append(b, b2...)
	return b
}
func (i extendedDevice) size() int64 {
	return 0
}
func (i extendedDevice) xattrIndex() (uint32, bool) {
	return i.xAttrIndex, i.xAttrIndex != noXattrInodeFlag
}

func (i extendedDevice) equal(o inodeBody) bool {
	oi, ok := o.(extendedDevice)
	if !ok {
		return false
	}
	if i != oi {
		return false
	}
	return true
}

func parseExtendedDevice(b []byte) (*extendedDevice, error) {
	target := 12
	if len(b) < target {
		return nil, fmt.Errorf("received %d bytes instead of expected minimal %d", len(b), target)
	}
	basic, err := parseBasicDevice(b[:8])
	if err != nil {
		return nil, fmt.Errorf("error parsing block device: %v", err)
	}
	return &extendedDevice{
		links:      basic.links,
		major:      basic.major,
		minor:      basic.minor,
		xAttrIndex: binary.LittleEndian.Uint32(b[8:12]),
	}, nil
}

// extendedBlock
type extendedBlock struct {
	extendedDevice
}
type extendedChar struct {
	extendedDevice
}

type basicIPC struct {
	links uint32
}

func (i basicIPC) toBytes() []byte {
	b := make([]byte, 4)

	binary.LittleEndian.PutUint32(b[0:4], i.links)
	return b
}
func (i basicIPC) size() int64 {
	return 0
}
func (i basicIPC) xattrIndex() (uint32, bool) {
	return 0, false
}

func (i basicIPC) equal(o inodeBody) bool {
	oi, ok := o.(basicIPC)
	if !ok {
		return false
	}
	if i != oi {
		return false
	}
	return true
}

func parseBasicIPC(b []byte) (*basicIPC, error) {
	target := 4
	if len(b) < target {
		return nil, fmt.Errorf("received %d bytes instead of expected %d", len(b), target)
	}
	s := &basicIPC{
		links: binary.LittleEndian.Uint32(b[0:4]),
	}
	return s, nil
}

type basicFifo struct {
	basicIPC
}
type basicSocket struct {
	basicIPC
}

type extendedIPC struct {
	links      uint32
	xAttrIndex uint32
}

func (i extendedIPC) toBytes() []byte {
	b := make([]byte, 8)

	binary.LittleEndian.PutUint32(b[0:4], i.links)
	binary.LittleEndian.PutUint32(b[4:8], i.xAttrIndex)
	return b
}
func (i extendedIPC) size() int64 {
	return 0
}
func (i extendedIPC) xattrIndex() (uint32, bool) {
	return i.xAttrIndex, i.xAttrIndex != noXattrInodeFlag
}

func (i extendedIPC) equal(o inodeBody) bool {
	oi, ok := o.(extendedIPC)
	if !ok {
		return false
	}
	if i != oi {
		return false
	}
	return true
}

func parseExtendedIPC(b []byte) (*extendedIPC, error) {
	target := 8
	if len(b) < target {
		return nil, fmt.Errorf("received %d bytes instead of expected %d", len(b), target)
	}
	s := &extendedIPC{
		links:      binary.LittleEndian.Uint32(b[0:4]),
		xAttrIndex: binary.LittleEndian.Uint32(b[4:8]),
	}
	return s, nil
}

type extendedFifo struct {
	extendedIPC
}
type extendedSocket struct {
	extendedIPC
}

// idTable is an indexed table of IDs
//
//nolint:deadcode // we need these references in the future
type idTable []uint32

// parseInodeBody parse the body of an inode. This only parses the non-variable size part,
// e.g. not the list of directory indexes in an extended directory inode, or
// the list of blocksizes at the end of a basic file inode or extended file inode. For those,
// parseInodeBody will return how many more bytes it needs to read. It is up to the caller
// to provide those bytes in another call.
func parseInodeBody(b []byte, blocksize int, iType inodeType) (inodeBody, int, error) {
	// now try to read the rest
	var (
		body  inodeBody
		err   error
		extra int
	)
	switch iType {
	case inodeBasicDirectory:
		body, err = parseBasicDirectory(b)
	case inodeExtendedDirectory:
		body, extra, err = parseExtendedDirectory(b)
	case inodeBasicFile:
		body, extra, err = parseBasicFile(b, blocksize)
	case inodeExtendedFile:
		body, extra, err = parseExtendedFile(b, blocksize)
	case inodeBasicChar, inodeBasicBlock:
		body, err = parseBasicDevice(b)
	case inodeExtendedChar, inodeExtendedBlock:
		body, err = parseExtendedDevice(b)
	case inodeBasicSymlink:
		body, extra, err = parseBasicSymlink(b)
	case inodeExtendedSymlink:
		body, extra, err = parseExtendedSymlink(b)
	case inodeBasicFifo, inodeBasicSocket:
		body, err = parseBasicIPC(b)
	case inodeExtendedFifo, inodeExtendedSocket:
		body, err = parseExtendedIPC(b)
	default:
		err = fmt.Errorf("unknown inode type: %v", iType)
	}
	return body, extra, err
}
