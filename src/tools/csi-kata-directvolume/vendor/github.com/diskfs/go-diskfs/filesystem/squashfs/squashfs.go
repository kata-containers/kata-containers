package squashfs

import (
	"encoding/binary"
	"fmt"
	"io"
	"math"
	"os"
	"path"

	"github.com/diskfs/go-diskfs/filesystem"
	"github.com/diskfs/go-diskfs/util"
)

const (
	defaultBlockSize  = 128 * KB
	metadataBlockSize = 8 * KB
	minBlocksize      = 4 * KB
	maxBlocksize      = 1 * MB
)

// FileSystem implements the FileSystem interface
type FileSystem struct {
	workspace  string
	superblock *superblock
	size       int64
	start      int64
	file       util.File
	blocksize  int64
	compressor Compressor
	fragments  []*fragmentEntry
	uidsGids   []uint32
	xattrs     *xAttrTable
	rootDir    inode
}

// Equal compare if two filesystems are equal
func (fs *FileSystem) Equal(a *FileSystem) bool {
	localMatch := fs.file == a.file && fs.size == a.size
	superblockMatch := fs.superblock.equal(a.superblock)
	return localMatch && superblockMatch
}

// Label return the filesystem label
func (fs *FileSystem) Label() string {
	return ""
}

func (fs *FileSystem) SetLabel(string) error {
	return fmt.Errorf("SquashFS filesystem is read-only")
}

// Workspace get the workspace path
func (fs *FileSystem) Workspace() string {
	return fs.workspace
}

// Create creates a squashfs filesystem in a given directory
//
// requires the util.File where to create the filesystem, size is the size of the filesystem in bytes,
// start is how far in bytes from the beginning of the util.File to create the filesystem,
// and blocksize is is the logical blocksize to use for creating the filesystem
//
// note that you are *not* required to create the filesystem on the entire disk. You could have a disk of size
// 20GB, and create a small filesystem of size 50MB that begins 2GB into the disk.
// This is extremely useful for creating filesystems on disk partitions.
//
// Note, however, that it is much easier to do this using the higher-level APIs at github.com/diskfs/go-diskfs
// which allow you to work directly with partitions, rather than having to calculate (and hopefully not make any errors)
// where a partition starts and ends.
//
// If the provided blocksize is 0, it will use the default of 128 KB.
func Create(f util.File, size, start, blocksize int64) (*FileSystem, error) {
	if blocksize == 0 {
		blocksize = defaultBlockSize
	}
	// make sure it is an allowed blocksize
	if err := validateBlocksize(blocksize); err != nil {
		return nil, err
	}

	// create a temporary working area where we can create the filesystem.
	//  It is only on `Finalize()` that we write it out to the actual disk file
	tmpdir, err := os.MkdirTemp("", "diskfs_squashfs")
	if err != nil {
		return nil, fmt.Errorf("could not create working directory: %v", err)
	}

	// create root directory
	// there is nothing in there
	return &FileSystem{
		workspace: tmpdir,
		start:     start,
		size:      size,
		file:      f,
		blocksize: blocksize,
	}, nil
}

// Read reads a filesystem from a given disk.
//
// requires the util.File where to read the filesystem, size is the size of the filesystem in bytes,
// start is how far in bytes from the beginning of the util.File the filesystem is expected to begin,
// and blocksize is is the logical blocksize to use for creating the filesystem
//
// note that you are *not* required to read a filesystem on the entire disk. You could have a disk of size
// 20GB, and a small filesystem of size 50MB that begins 2GB into the disk.
// This is extremely useful for working with filesystems on disk partitions.
//
// Note, however, that it is much easier to do this using the higher-level APIs at github.com/diskfs/go-diskfs
// which allow you to work directly with partitions, rather than having to calculate (and hopefully not make any errors)
// where a partition starts and ends.
//
// If the provided blocksize is 0, it will use the default of 2K bytes
func Read(file util.File, size, start, blocksize int64) (*FileSystem, error) {
	var (
		read int
		err  error
	)

	if blocksize == 0 {
		blocksize = defaultBlockSize
	}
	// make sure it is an allowed blocksize
	if err := validateBlocksize(blocksize); err != nil {
		return nil, err
	}

	// load the information from the disk

	// read the superblock
	b := make([]byte, superblockSize)
	read, err = file.ReadAt(b, start)
	if err != nil {
		return nil, fmt.Errorf("unable to read bytes for superblock: %v", err)
	}
	if int64(read) != superblockSize {
		return nil, fmt.Errorf("read %d bytes instead of expected %d for superblock", read, superblockSize)
	}

	// parse superblock
	s, err := parseSuperblock(b)
	if err != nil {
		return nil, fmt.Errorf("error parsing superblock: %v", err)
	}

	// create the compressor function we will use
	compress, err := newCompressor(s.compression)
	if err != nil {
		return nil, fmt.Errorf("unable to create compressor")
	}

	// load fragments
	fragments, err := readFragmentTable(s, file, compress)
	if err != nil {
		return nil, fmt.Errorf("error reading fragments: %v", err)
	}

	// read xattrs
	var (
		xattrs *xAttrTable
	)
	if !s.noXattrs {
		// xattr is right to the end of the disk
		xattrs, err = readXattrsTable(s, file, compress)
		if err != nil {
			return nil, fmt.Errorf("error reading xattr table: %v", err)
		}
	}

	// read uidsgids
	uidsgids, err := readUidsGids(s, file, compress)
	if err != nil {
		return nil, fmt.Errorf("error reading uids/gids: %v", err)
	}

	fs := &FileSystem{
		workspace:  "", // no workspace when we do nothing with it
		start:      start,
		size:       size,
		file:       file,
		superblock: s,
		blocksize:  blocksize,
		xattrs:     xattrs,
		compressor: compress,
		fragments:  fragments,
		uidsGids:   uidsgids,
	}
	// for efficiency, read in the root inode right now
	rootInode, err := fs.getInode(s.rootInode.block, s.rootInode.offset, inodeBasicDirectory)
	if err != nil {
		return nil, fmt.Errorf("unable to read root inode")
	}
	fs.rootDir = rootInode
	return fs, nil
}

// Type returns the type code for the filesystem. Always returns filesystem.TypeFat32
func (fs *FileSystem) Type() filesystem.Type {
	return filesystem.TypeSquashfs
}

// Mkdir make a directory at the given path. It is equivalent to `mkdir -p`, i.e. idempotent, in that:
//
// * It will make the entire tree path if it does not exist
// * It will not return an error if the path already exists
//
// if readonly and not in workspace, will return an error
func (fs *FileSystem) Mkdir(p string) error {
	if fs.workspace == "" {
		return fmt.Errorf("cannot write to read-only filesystem")
	}
	err := os.MkdirAll(path.Join(fs.workspace, p), 0o755)
	if err != nil {
		return fmt.Errorf("could not create directory %s: %v", p, err)
	}
	// we are not interesting in returning the entries
	return err
}

// ReadDir return the contents of a given directory in a given filesystem.
//
// Returns a slice of os.FileInfo with all of the entries in the directory.
//
// Will return an error if the directory does not exist or is a regular file and not a directory
func (fs *FileSystem) ReadDir(p string) ([]os.FileInfo, error) {
	var fi []os.FileInfo
	// non-workspace: read from squashfs
	// workspace: read from regular filesystem
	if fs.workspace != "" {
		fullPath := path.Join(fs.workspace, p)
		// read the entries
		dirEntries, err := os.ReadDir(fullPath)
		if err != nil {
			return nil, fmt.Errorf("could not read directory %s: %v", p, err)
		}
		for _, e := range dirEntries {
			info, err := e.Info()
			if err != nil {
				return nil, fmt.Errorf("could not read directory %s: %v", p, err)
			}

			fi = append(fi, info)
		}
	} else {
		dirEntries, err := fs.readDirectory(p)
		if err != nil {
			return nil, fmt.Errorf("error reading directory %s: %v", p, err)
		}
		fi = make([]os.FileInfo, 0, len(dirEntries))
		for _, entry := range dirEntries {
			fi = append(fi, entry)
		}
	}
	return fi, nil
}

// OpenFile returns an io.ReadWriter from which you can read the contents of a file
// or write contents to the file
//
// accepts normal os.OpenFile flags
//
// returns an error if the file does not exist
func (fs *FileSystem) OpenFile(p string, flag int) (filesystem.File, error) {
	var f filesystem.File
	var err error

	// get the path and filename
	dir := path.Dir(p)
	filename := path.Base(p)

	// if the dir == filename, then it is just /
	if dir == filename {
		return nil, fmt.Errorf("cannot open directory %s as file", p)
	}

	// cannot open to write or append or create if we do not have a workspace
	writeMode := flag&os.O_WRONLY != 0 || flag&os.O_RDWR != 0 || flag&os.O_APPEND != 0 || flag&os.O_CREATE != 0 || flag&os.O_TRUNC != 0 || flag&os.O_EXCL != 0
	if fs.workspace == "" {
		if writeMode {
			return nil, fmt.Errorf("cannot write to read-only filesystem")
		}

		// get the directory entries
		var entries []*directoryEntry
		entries, err = fs.readDirectory(dir)
		if err != nil {
			return nil, fmt.Errorf("could not read directory entries for %s", dir)
		}
		// we now know that the directory exists, see if the file exists
		var targetEntry *directoryEntry
		for _, e := range entries {
			eName := e.Name()
			// cannot do anything with directories
			if eName == filename && e.IsDir() {
				return nil, fmt.Errorf("cannot open directory %s as file", p)
			}
			if eName == filename {
				// if we got this far, we have found the file
				targetEntry = e
				break
			}
		}

		// see if the file exists
		// if the file does not exist, and is not opened for os.O_CREATE, return an error
		if targetEntry == nil {
			return nil, fmt.Errorf("target file %s does not exist", p)
		}
		// get the inode data for this file
		// now open the file
		// get the inode for the file
		var eFile *extendedFile
		in := targetEntry.inode
		iType := in.inodeType()
		body := in.getBody()
		//nolint:exhaustive // all other cases fall under default
		switch iType {
		case inodeBasicFile:
			extFile := body.(*basicFile).toExtended()
			eFile = &extFile
		case inodeExtendedFile:
			eFile, _ = body.(*extendedFile)
		default:
			return nil, fmt.Errorf("inode is of type %d, neither basic nor extended directory", iType)
		}

		f = &File{
			extendedFile: eFile,
			isReadWrite:  false,
			isAppend:     false,
			offset:       0,
			filesystem:   fs,
		}
	} else {
		f, err = os.OpenFile(path.Join(fs.workspace, p), flag, 0o644)
		if err != nil {
			return nil, fmt.Errorf("target file %s does not exist: %v", p, err)
		}
	}

	return f, nil
}

// readDirectory - read directory entry on squashfs only (not workspace)
func (fs *FileSystem) readDirectory(p string) ([]*directoryEntry, error) {
	// use the root inode to find the location of the root direectory in the table
	entries, err := fs.getDirectoryEntries(p, fs.rootDir)
	if err != nil {
		return nil, fmt.Errorf("could not read directory at path %s: %v", p, err)
	}
	return entries, nil
}

func (fs *FileSystem) getDirectoryEntries(p string, in inode) ([]*directoryEntry, error) {
	var (
		block  uint32
		offset uint16
		size   int
	)

	// break path down into parts and levels
	parts := splitPath(p)

	iType := in.inodeType()
	body := in.getBody()
	//nolint:exhaustive // we only are looking for directory types here
	switch iType {
	case inodeBasicDirectory:
		dir, _ := body.(*basicDirectory)
		block = dir.startBlock
		offset = dir.offset
		size = int(dir.fileSize)
	case inodeExtendedDirectory:
		dir, _ := body.(*extendedDirectory)
		block = dir.startBlock
		offset = dir.offset
		size = int(dir.fileSize)
	default:
		return nil, fmt.Errorf("inode is of type %d, neither basic nor extended directory", iType)
	}
	// read the directory data from the directory table
	dir, err := fs.getDirectory(block, offset, size)
	if err != nil {
		return nil, fmt.Errorf("unable to read directory from table: %v", err)
	}
	entriesRaw := dir.entries
	var entries []*directoryEntry
	// if this is the directory we are looking for, return the entries
	if len(parts) == 0 {
		entries, err = fs.hydrateDirectoryEntries(entriesRaw)
		if err != nil {
			return nil, fmt.Errorf("could not populate directory entries for %s with properties: %v", p, err)
		}
		return entries, nil
	}

	// it is not, so dig down one level
	// find the entry among the children that has the desired name
	for _, entry := range entriesRaw {
		// only care if not self or parent entry
		checkFilename := entry.name
		if checkFilename == parts[0] {
			// read the inode for this entry
			inode, err := fs.getInode(entry.startBlock, entry.offset, entry.inodeType)
			if err != nil {
				return nil, fmt.Errorf("error finding inode for %s: %v", p, err)
			}

			childPath := ""
			if len(parts) > 1 {
				childPath = path.Join(parts[1:]...)
			}
			entries, err = fs.getDirectoryEntries(childPath, inode)
			if err != nil {
				return nil, fmt.Errorf("could not get entries: %v", err)
			}
			return entries, nil
		}
	}
	// if we made it here, we were not looking for this directory, but did not find it among our children
	return nil, fmt.Errorf("could not find path %s", p)
}

func (fs *FileSystem) hydrateDirectoryEntries(entries []*directoryEntryRaw) ([]*directoryEntry, error) {
	fullEntries := make([]*directoryEntry, 0)
	for _, e := range entries {
		// read the inode for this entry
		in, err := fs.getInode(e.startBlock, e.offset, e.inodeType)
		if err != nil {
			return nil, fmt.Errorf("error finding inode for %s: %v", e.name, err)
		}
		body, header := in.getBody(), in.getHeader()
		xattrIndex, has := body.xattrIndex()
		xattrs := map[string]string{}
		if has && xattrIndex != noXattrInodeFlag {
			xattrs, err = fs.xattrs.find(int(xattrIndex))
			if err != nil {
				return nil, fmt.Errorf("error reading xattrs for %s: %v", e.name, err)
			}
		}
		fullEntries = append(fullEntries, &directoryEntry{
			isSubdirectory: e.isSubdirectory,
			name:           e.name,
			size:           body.size(),
			modTime:        header.modTime,
			mode:           header.mode,
			inode:          in,
			sys: FileStat{
				uid:    fs.uidsGids[header.uidIdx],
				gid:    fs.uidsGids[header.gidIdx],
				xattrs: xattrs,
			},
		})
	}
	return fullEntries, nil
}

// getInode read a single inode, given the block offset, and the offset in the
// block when uncompressed. This may require two reads, one to get the header and discover the type,
// and then another to read the rest. Some inodes even have a variable length, which complicates it
// further.
func (fs *FileSystem) getInode(blockOffset uint32, byteOffset uint16, iType inodeType) (inode, error) {
	// get the block
	// start by getting the minimum for the proposed type. It very well might be wrong.
	size := inodeTypeToSize(iType)
	uncompressed, err := readMetadata(fs.file, fs.compressor, int64(fs.superblock.inodeTableStart), blockOffset, byteOffset, size)
	if err != nil {
		return nil, fmt.Errorf("error reading block at position %d: %v", blockOffset, err)
	}
	// parse the header to see the type matches
	header, err := parseInodeHeader(uncompressed)
	if err != nil {
		return nil, fmt.Errorf("error parsing inode header: %v", err)
	}
	if header.inodeType != iType {
		iType = header.inodeType
	}
	// now read the body, which may have a variable size
	body, extra, err := parseInodeBody(uncompressed[inodeHeaderSize:], int(fs.blocksize), iType)
	if err != nil {
		return nil, fmt.Errorf("error parsing inode body: %v", err)
	}
	// if it returns extra > 0, then it needs that many more bytes to be read, and to be reparsed
	if extra > 0 {
		size += extra
		uncompressed, err = readMetadata(fs.file, fs.compressor, int64(fs.superblock.inodeTableStart), blockOffset, byteOffset, size)
		if err != nil {
			return nil, fmt.Errorf("error reading block at position %d: %v", blockOffset, err)
		}
		// no need to revalidate the body type, or check for extra
		body, _, err = parseInodeBody(uncompressed[inodeHeaderSize:], int(fs.blocksize), iType)
		if err != nil {
			return nil, fmt.Errorf("error parsing inode body: %v", err)
		}
	}
	return &inodeImpl{
		header: header,
		body:   body,
	}, nil
}

// getDirectory read a single directory, given the block offset, and the offset in the
// block when uncompressed.
func (fs *FileSystem) getDirectory(blockOffset uint32, byteOffset uint16, size int) (*directory, error) {
	// get the block
	uncompressed, err := readMetadata(fs.file, fs.compressor, int64(fs.superblock.directoryTableStart), blockOffset, byteOffset, size)
	if err != nil {
		return nil, fmt.Errorf("error reading block at position %d: %v", blockOffset, err)
	}
	// for parseDirectory, we only want to use precisely the right number of bytes
	if len(uncompressed) > size {
		uncompressed = uncompressed[:size]
	}
	// get the inode from the offset into the uncompressed block
	return parseDirectory(uncompressed)
}

func (fs *FileSystem) readBlock(location int64, compressed bool, size uint32) ([]byte, error) {
	b := make([]byte, size)
	read, err := fs.file.ReadAt(b, location)
	if err != nil && err != io.EOF {
		return nil, fmt.Errorf("error reading block %d: %v", location, err)
	}
	if read != int(size) {
		return nil, fmt.Errorf("read %d bytes instead of expected %d", read, size)
	}
	if compressed {
		b, err = fs.compressor.decompress(b)
		if err != nil {
			return nil, fmt.Errorf("decompress error: %v", err)
		}
	}
	return b, nil
}

func (fs *FileSystem) readFragment(index, offset uint32, fragmentSize int64) ([]byte, error) {
	// get info from the fragment table
	// figure out which block of the fragment table we need

	// first find where the compressed fragment table entry for the given index is
	if len(fs.fragments)-1 < int(index) {
		return nil, fmt.Errorf("cannot find fragment block with index %d", index)
	}
	fragmentInfo := fs.fragments[index]
	// figure out the size of the compressed block and if it is compressed
	b := make([]byte, fragmentInfo.size)
	read, err := fs.file.ReadAt(b, int64(fragmentInfo.start))
	if err != nil && err != io.EOF {
		return nil, fmt.Errorf("unable to read fragment block %d: %v", index, err)
	}
	if read != len(b) {
		return nil, fmt.Errorf("read %d instead of expected %d bytes for fragment block %d", read, len(b), index)
	}

	data := b
	if fragmentInfo.compressed {
		if fs.compressor == nil {
			return nil, fmt.Errorf("fragment compressed but do not have valid compressor")
		}
		data, err = fs.compressor.decompress(b)
		if err != nil {
			return nil, fmt.Errorf("decompress error: %v", err)
		}
	}
	// now get the data from the offset
	return data[offset : int64(offset)+fragmentSize], nil
}

func validateBlocksize(blocksize int64) error {
	blocksizeFloat := float64(blocksize)
	l2 := math.Log2(blocksizeFloat)
	switch {
	case blocksize < minBlocksize:
		return fmt.Errorf("blocksize %d too small, must be at least %d", blocksize, minBlocksize)
	case blocksize > maxBlocksize:
		return fmt.Errorf("blocksize %d too large, must be no more than %d", blocksize, maxBlocksize)
	case math.Trunc(l2) != l2:
		return fmt.Errorf("blocksize %d is not a power of 2", blocksize)
	}
	return nil
}

func readFragmentTable(s *superblock, file util.File, c Compressor) ([]*fragmentEntry, error) {
	// get the first level index, which is just the pointers to the fragment table metadata blocks
	blockCount := s.fragmentCount / 512
	if s.fragmentCount%512 > 0 {
		blockCount++
	}
	// now read the index - we have as many offsets, each of uint64, as we have blockCount
	b := make([]byte, 8*blockCount)
	read, err := file.ReadAt(b, int64(s.fragmentTableStart))
	if err != nil {
		return nil, fmt.Errorf("error reading fragment table index: %v", err)
	}
	if read != len(b) {
		return nil, fmt.Errorf("read %d bytes instead of expected %d bytes of fragment table index", read, len(b))
	}
	var offsets []int64
	for i := 0; i < len(b); i += 8 {
		offsets = append(offsets, int64(binary.LittleEndian.Uint64(b[i:i+8])))
	}
	// offsets now contains all of the fragment block offsets
	// load in the actual fragment entries
	// read each block and uncompress it
	var fragmentTable []*fragmentEntry
	for i, offset := range offsets {
		uncompressed, _, err := readMetaBlock(file, c, offset)
		if err != nil {
			return nil, fmt.Errorf("error reading meta block %d at position %d: %v", i, offset, err)
		}
		// uncompressed should be a multiple of 16 bytes
		for j := 0; j < len(uncompressed); j += 16 {
			entry, err := parseFragmentEntry(uncompressed[j:])
			if err != nil {
				return nil, fmt.Errorf("error parsing fragment table entry in block %d position %d: %v", i, j, err)
			}
			fragmentTable = append(fragmentTable, entry)
		}
	}
	return fragmentTable, nil
}

/*
How the xattr table is laid out
It has three components in the following order
1- xattr metadata
2- xattr id table
3- xattr index

To read the xattr table:
1- Get the start of the index from the superblock
2- read the index header, which contains: metadata start; id count
3- Calculate how many bytes of index data there are: (id count)*(index size)
4- Calculate how many meta blocks of index data there are, as each block is 8K uncompressed
5- Read the indexes immediately following the header. They are uncompressed, 8 bytes each (uint64); one index per id metablock
6- Read the id metablocks based on the indexes and uncompress if needed
7- Read all of the xattr metadata. It starts at the location indicated by the header, and ends at the id table
*/
func readXattrsTable(s *superblock, file util.File, c Compressor) (*xAttrTable, error) {
	// first read the header
	b := make([]byte, xAttrHeaderSize)
	read, err := file.ReadAt(b, int64(s.xattrTableStart))
	if err != nil && err != io.EOF {
		return nil, fmt.Errorf("unable to read bytes for xattrs metadata ID header: %v", err)
	}
	if read != len(b) {
		return nil, fmt.Errorf("read %d bytes instead of expected %d for xattrs metadata ID header", read, len(b))
	}
	// find out how many xattr IDs we have and where the metadata starts. The table always starts
	//   with this information
	xAttrStart := binary.LittleEndian.Uint64(b[0:8])
	xAttrCount := binary.LittleEndian.Uint32(b[8:12])
	// the last 4 bytes are an unused uint32

	// if we have none?
	if xAttrCount == 0 {
		return nil, nil
	}

	// how many bytes total do we need?
	idBytes := xAttrCount * xAttrIDEntrySize
	// how many metadata blocks?
	idBlocks := ((idBytes - 1) / uint32(metadataBlockSize)) + 1
	b = make([]byte, idBlocks*8)
	read, err = file.ReadAt(b, int64(s.xattrTableStart)+int64(xAttrHeaderSize))
	if err != nil && err != io.EOF {
		return nil, fmt.Errorf("unable to read bytes for xattrs metadata ID table: %v", err)
	}
	if read != len(b) {
		return nil, fmt.Errorf("read %d bytes instead of expected %d for xattrs metadata ID table", read, len(b))
	}

	var (
		uncompressed []byte
		size         uint16
	)

	bIndex := make([]byte, 0)
	// convert those into indexes
	for i := 0; i+8-1 < len(b); i += 8 {
		locn := binary.LittleEndian.Uint64(b[i : i+8])
		uncompressed, _, err = readMetaBlock(file, c, int64(locn))
		if err != nil {
			return nil, fmt.Errorf("error reading xattr index meta block %d at position %d: %v", i, locn, err)
		}
		bIndex = append(bIndex, uncompressed...)
	}

	// now load the actual xAttrs data
	xAttrEnd := binary.LittleEndian.Uint64(b[:8])
	xAttrData := make([]byte, 0)
	for i := xAttrStart; i < xAttrEnd; {
		uncompressed, size, err = readMetaBlock(file, c, int64(i))
		if err != nil {
			return nil, fmt.Errorf("error reading xattr data meta block at position %d: %v", i, err)
		}
		xAttrData = append(xAttrData, uncompressed...)
		i += uint64(size)
	}

	// now have all of the indexes and metadata loaded
	// need to pass it the offset of the beginning of the id table from the beginning of the disk
	return parseXattrsTable(xAttrData, bIndex, s.idTableStart, c)
}

//nolint:unparam // this does not use offset or compressor yet, but only because we have not yet added support
func parseXattrsTable(bUIDXattr, bIndex []byte, offset uint64, c Compressor) (*xAttrTable, error) {
	// create the ID list
	var (
		xAttrIDList []*xAttrIndex
	)

	entrySize := int(xAttrIDEntrySize)
	for i := 0; i+entrySize <= len(bIndex); i += entrySize {
		entry, err := parseXAttrIndex(bIndex[i:])
		if err != nil {
			return nil, fmt.Errorf("error parsing xAttr ID table entry in position %d: %v", i, err)
		}
		xAttrIDList = append(xAttrIDList, entry)
	}

	return &xAttrTable{
		list: xAttrIDList,
		data: bUIDXattr,
	}, nil
}

/*
How the uids/gids table is laid out
It has two components in the following order
1- list of uids/gids in order, each uint32. These are in metadata blocks of uncompressed 8K size
2- list of indexes to metadata blocks

To read the uids/gids table:
1- Get the start of the index from the superblock
2- Calculate how many bytes of ids there are: (id count)*(id size), where (id size) = 4 bytes (uint32)
3- Calculate how many meta blocks of id data there are, as each block is 8K uncompressed
4- Read the indexes. They are uncompressed, 8 bytes each (uint64); one index per id metablock
5- Read the id metablocks based on the indexes and uncompress if needed
*/
func readUidsGids(s *superblock, file util.File, c Compressor) ([]uint32, error) {
	// find out how many xattr IDs we have and where the metadata starts. The table always starts
	//   with this information
	idStart := s.idTableStart
	idCount := s.idCount

	// if we have none?
	if idCount == 0 {
		return nil, nil
	}

	// how many bytes total do we need?
	idBytes := idCount * idEntrySize
	// how many metadata blocks?
	idBlocks := ((idBytes - 1) / uint16(metadataBlockSize)) + 1
	b := make([]byte, idBlocks*8)
	read, err := file.ReadAt(b, int64(idStart))
	if err != nil && err != io.EOF {
		return nil, fmt.Errorf("unable to read index bytes for uidgid ID table: %v", err)
	}
	if read != len(b) {
		return nil, fmt.Errorf("read %d bytes instead of expected %d for uidgid ID table", read, len(b))
	}

	var (
		uncompressed []byte
	)

	data := make([]byte, 0)
	// convert those into indexes
	for i := 0; i+8-1 < len(b); i += 8 {
		locn := binary.LittleEndian.Uint64(b[i : i+8])
		uncompressed, _, err = readMetaBlock(file, c, int64(locn))
		if err != nil {
			return nil, fmt.Errorf("error reading uidgid index meta block %d at position %d: %v", i, locn, err)
		}
		data = append(data, uncompressed...)
	}

	// now have all of the data loaded
	return parseIDTable(data), nil
}
