package squashfs

import (
	"encoding/binary"
	"fmt"
	"io"
	"math"
	"os"
	"path"

	"github.com/diskfs/go-diskfs/backend"
	"github.com/diskfs/go-diskfs/filesystem"
)

const (
	defaultBlockSize  = 128 * KB
	metadataBlockSize = 8 * KB
	minBlocksize      = 4 * KB
	maxBlocksize      = 1 * MB
	defaultCacheSize  = 128 * MB
)

// FileSystem implements the FileSystem interface
type FileSystem struct {
	workspace  string
	superblock *superblock
	size       int64
	start      int64
	backend    backend.Storage
	blocksize  int64
	compressor Compressor
	fragments  []*fragmentEntry
	uidsGids   []uint32
	xattrs     *xAttrTable
	rootDir    inode
	cache      *lru
}

// Equal compare if two filesystems are equal
func (fs *FileSystem) Equal(a *FileSystem) bool {
	localMatch := fs.backend == a.backend && fs.size == a.size
	superblockMatch := fs.superblock.equal(a.superblock)
	return localMatch && superblockMatch
}

// Label return the filesystem label
func (fs *FileSystem) Label() string {
	return ""
}

func (fs *FileSystem) SetLabel(string) error {
	return filesystem.ErrReadonlyFilesystem
}

// Workspace get the workspace path
func (fs *FileSystem) Workspace() string {
	return fs.workspace
}

// Create creates a squashfs filesystem in a given directory
//
// requires the backend.Storage where to create the filesystem, size is the size of the filesystem in bytes,
// start is how far in bytes from the beginning of the backend.Storage to create the filesystem,
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
func Create(b backend.Storage, size, start, blocksize int64) (*FileSystem, error) {
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
		backend:   b,
		blocksize: blocksize,
	}, nil
}

// Read reads a filesystem from a given disk.
//
// requires the backend.Storage where to read the filesystem, size is the size of the filesystem in bytes,
// start is how far in bytes from the beginning of the backend.Storage the filesystem is expected to begin,
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
// If the provided blocksize is 0, it will use the default of 2K bytes.
//
// This will use a cache for the decompressed blocks of 128 MB by
// default. (You can set this with the SetCacheSize method and read
// its size with the GetCacheSize method). A block cache is essential
// for performance when reading. This implements a cache for the
// fragments (tail ends of files) and the metadata (directory
// listings) which otherwise would be read, decompressed and discarded
// many times.
//
// Unpacking a 3 GB squashfs made from the tensorflow docker image like this:
//
//	docker export $(docker create tensorflow/tensorflow:latest-gpu-jupyter) -o tensorflow.tar.gz
//	mkdir -p tensorflow && tar xf tensorflow.tar.gz -C tensorflow
//	[ -f tensorflow.sqfs ] && rm tensorflow.sqfs
//	mksquashfs tensorflow tensorflow.sqfs  -comp zstd -Xcompression-level 3 -b 1M -no-xattrs -all-root
//
// Gives these timings with and without cache:
//
// - no caching:   206s
// - 256 MB cache:  16.7s
// - 128 MB cache:  17.5s (the default)
// - 64 MB cache:   23.4s
// - 32 MB cache:   54.s
//
// The cached versions compare favourably to the C program unsquashfs
// which takes 12.0s to unpack the same archive.
//
// These tests were done using rclone and the archive backend which
// uses this library like this:
//
//	rclone -P --transfers 16 --checkers 16 copy :archive:/path/to/tensorflow.sqfs /tmp/tensorflow
func Read(b backend.Storage, size, start, blocksize int64) (*FileSystem, error) {
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
	superblockBytes := make([]byte, superblockSize)
	read, err = b.ReadAt(superblockBytes, start)
	if err != nil {
		return nil, fmt.Errorf("unable to read bytes for superblock: %v", err)
	}
	if int64(read) != superblockSize {
		return nil, fmt.Errorf("read %d bytes instead of expected %d for superblock", read, superblockSize)
	}

	// parse superblock
	s, err := parseSuperblock(superblockBytes)
	if err != nil {
		return nil, fmt.Errorf("error parsing superblock: %v", err)
	}

	// create the compressor function we will use
	compress, err := newCompressor(s.compression)
	if err != nil {
		return nil, fmt.Errorf("unable to create compressor: %v", err)
	}

	// load fragments
	fragments, err := readFragmentTable(s, b, compress)
	if err != nil {
		return nil, fmt.Errorf("error reading fragments: %v", err)
	}

	// read xattrs
	var (
		xattrs *xAttrTable
	)
	if !s.noXattrs && s.xattrTableStart != 0xffff_ffff_ffff_ffff {
		// xattr is right to the end of the disk
		xattrs, err = readXattrsTable(s, b, compress)
		if err != nil {
			return nil, fmt.Errorf("error reading xattr table: %v", err)
		}
	}

	// read uidsgids
	uidsgids, err := readUidsGids(s, b, compress)
	if err != nil {
		return nil, fmt.Errorf("error reading uids/gids: %v", err)
	}

	fs := &FileSystem{
		workspace:  "", // no workspace when we do nothing with it
		start:      start,
		size:       size,
		backend:    b,
		superblock: s,
		blocksize:  int64(s.blocksize), // use the blocksize in the superblock
		xattrs:     xattrs,
		compressor: compress,
		fragments:  fragments,
		uidsGids:   uidsgids,
		cache:      newLRU(int(defaultCacheSize) / int(s.blocksize)),
	}
	// for efficiency, read in the root inode right now
	rootInode, err := fs.getInode(s.rootInode.block, s.rootInode.offset, inodeBasicDirectory)
	if err != nil {
		return nil, fmt.Errorf("unable to read root inode")
	}
	fs.rootDir = rootInode
	return fs, nil
}

// interface guard
var _ filesystem.FileSystem = (*FileSystem)(nil)

// Delete the temporary directory created during the SquashFS image creation
func (fs *FileSystem) Close() error {
	if fs.workspace != "" {
		return os.RemoveAll(fs.workspace)
	}
	return nil
}

// Type returns the type code for the filesystem. Always returns filesystem.TypeFat32
func (fs *FileSystem) Type() filesystem.Type {
	return filesystem.TypeSquashfs
}

// SetCacheSize set the maximum memory used by the block cache to cacheSize bytes.
//
// The default is 128 MB.
//
// If this is <= 0 then the cache will be disabled.
func (fs *FileSystem) SetCacheSize(cacheSize int) {
	if fs.cache == nil {
		return
	}
	blocks := cacheSize / int(fs.blocksize)
	if blocks <= 0 {
		blocks = 0
	}
	fs.cache.setMaxBlocks(blocks)
}

// GetCacheSize get the maximum memory used by the block cache in bytes.
func (fs *FileSystem) GetCacheSize() int {
	if fs.cache == nil {
		return 0
	}
	return fs.cache.maxBlocks * int(fs.blocksize)
}

// Mkdir make a directory at the given path. It is equivalent to `mkdir -p`, i.e. idempotent, in that:
//
// * It will make the entire tree path if it does not exist
// * It will not return an error if the path already exists
//
// if readonly and not in workspace, will return an error
func (fs *FileSystem) Mkdir(p string) error {
	if fs.workspace == "" {
		return filesystem.ErrReadonlyFilesystem
	}
	err := os.MkdirAll(path.Join(fs.workspace, p), 0o755)
	if err != nil {
		return fmt.Errorf("could not create directory %s: %v", p, err)
	}
	// we are not interesting in returning the entries
	return err
}

// creates a filesystem node (file, device special file, or named pipe) named pathname,
// with attributes specified by mode and dev
//
//nolint:revive // parameters will be used eventually
func (fs *FileSystem) Mknod(pathname string, mode uint32, dev int) error {
	// https://dr-emann.github.io/squashfs/squashfs.html#_device_special_files
	// https://dr-emann.github.io/squashfs/squashfs.html#_ipc_inodes_fifo_or_socket
	return filesystem.ErrNotImplemented
}

// creates a new link (also known as a hard link) to an existing file.
//
//nolint:revive // parameters will be used eventually
func (fs *FileSystem) Link(oldpath, newpath string) error {
	// https://dr-emann.github.io/squashfs/squashfs.html#_symbolic_links
	return filesystem.ErrNotImplemented
}

// creates a symbolic link named linkpath which contains the string target.
//
//nolint:revive // parameters will be used eventually
func (fs *FileSystem) Symlink(oldpath, newpath string) error {
	// https://dr-emann.github.io/squashfs/squashfs.html#_symbolic_links
	return filesystem.ErrNotImplemented
}

// Chmod changes the mode of the named file to mode. If the file is a symbolic link,
// it changes the mode of the link's target.
//
//nolint:revive // parameters will be used eventually
func (fs *FileSystem) Chmod(name string, mode os.FileMode) error {
	// https://dr-emann.github.io/squashfs/squashfs.html#_common_inode_header
	return filesystem.ErrNotImplemented
}

// Chown changes the numeric uid and gid of the named file. If the file is a symbolic link,
// it changes the uid and gid of the link's target. A uid or gid of -1 means to not change that value
//
//nolint:revive // parameters will be used eventually
func (fs *FileSystem) Chown(name string, uid, gid int) error {
	// https://dr-emann.github.io/squashfs/squashfs.html#_id_table
	return filesystem.ErrNotImplemented
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
			return nil, filesystem.ErrReadonlyFilesystem
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
		f, err = targetEntry.Open()
		if err != nil {
			return nil, err
		}
	} else {
		f, err = os.OpenFile(path.Join(fs.workspace, p), flag, 0o644)
		if err != nil {
			return nil, fmt.Errorf("target file %s does not exist: %v", p, err)
		}
	}

	return f, nil
}

// Rename renames (moves) oldpath to newpath. If newpath already exists and is not a directory, Rename replaces it.
func (fs *FileSystem) Rename(oldpath, newpath string) error {
	if fs.workspace == "" {
		return filesystem.ErrReadonlyFilesystem
	}
	return os.Rename(path.Join(fs.workspace, oldpath), path.Join(fs.workspace, newpath))
}

func (fs *FileSystem) Remove(p string) error {
	if fs.workspace == "" {
		return filesystem.ErrReadonlyFilesystem
	}
	return os.Remove(path.Join(fs.workspace, p))
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
		if has {
			xattrs, err = fs.xattrs.find(int(xattrIndex))
			if err != nil {
				return nil, fmt.Errorf("error reading xattrs for %s: %v", e.name, err)
			}
		}
		fullEntries = append(fullEntries, &directoryEntry{
			fs:             fs,
			isSubdirectory: e.isSubdirectory,
			name:           e.name,
			size:           body.size(),
			modTime:        header.modTime,
			mode:           header.mode,
			inode:          in,
			uid:            fs.uidsGids[header.uidIdx],
			gid:            fs.uidsGids[header.gidIdx],
			xattrs:         xattrs,
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
	uncompressed, err := fs.readMetadata(fs.backend, fs.compressor, int64(fs.superblock.inodeTableStart), blockOffset, byteOffset, size)
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
		size = inodeTypeToSize(iType)
		// Read more data if necessary (quite rare)
		if size > len(uncompressed) {
			uncompressed, err = fs.readMetadata(fs.backend, fs.compressor, int64(fs.superblock.inodeTableStart), blockOffset, byteOffset, size)
			if err != nil {
				return nil, fmt.Errorf("error reading block at position %d: %v", blockOffset, err)
			}
		}
	}
	// now read the body, which may have a variable size
	body, extra, err := parseInodeBody(uncompressed[inodeHeaderSize:], int(fs.blocksize), iType)
	if err != nil {
		return nil, fmt.Errorf("error parsing inode body: %v", err)
	}
	// if it returns extra > 0, then it needs that many more bytes to be read, and to be reparsed
	if extra > 0 {
		size += extra
		uncompressed, err = fs.readMetadata(fs.backend, fs.compressor, int64(fs.superblock.inodeTableStart), blockOffset, byteOffset, size)
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
	uncompressed, err := fs.readMetadata(fs.backend, fs.compressor, int64(fs.superblock.directoryTableStart), blockOffset, byteOffset, size)
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
	// Zero size is a sparse block of blocksize
	if size == 0 {
		return make([]byte, fs.superblock.blocksize), nil
	}
	b := make([]byte, size)
	read, err := fs.backend.ReadAt(b, location)
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
	pos := int64(fragmentInfo.start)
	data, _, err := fs.cache.get(pos, func() (data []byte, size uint16, err error) {
		// figure out the size of the compressed block and if it is compressed
		b := make([]byte, fragmentInfo.size)
		read, err := fs.backend.ReadAt(b, pos)
		if err != nil && err != io.EOF {
			return nil, 0, fmt.Errorf("unable to read fragment block %d: %v", index, err)
		}
		if read != len(b) {
			return nil, 0, fmt.Errorf("read %d instead of expected %d bytes for fragment block %d", read, len(b), index)
		}

		data = b
		if fragmentInfo.compressed {
			if fs.compressor == nil {
				return nil, 0, fmt.Errorf("fragment compressed but do not have valid compressor")
			}
			data, err = fs.compressor.decompress(b)
			if err != nil {
				return nil, 0, fmt.Errorf("decompress error: %v", err)
			}
		}
		return data, 0, nil
	})
	if err != nil {
		return nil, err
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

func readFragmentTable(s *superblock, file backend.File, c Compressor) ([]*fragmentEntry, error) {
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
	var fs = &FileSystem{}
	for i, offset := range offsets {
		uncompressed, _, err := fs.readMetaBlock(file, c, offset)
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
func readXattrsTable(s *superblock, file backend.File, c Compressor) (*xAttrTable, error) {
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
		fs           = &FileSystem{}
	)

	bIndex := make([]byte, 0)
	// convert those into indexes
	for i := 0; i+8-1 < len(b); i += 8 {
		locn := binary.LittleEndian.Uint64(b[i : i+8])
		uncompressed, _, err = fs.readMetaBlock(file, c, int64(locn))
		if err != nil {
			return nil, fmt.Errorf("error reading xattr index meta block %d at position %d: %v", i, locn, err)
		}
		bIndex = append(bIndex, uncompressed...)
	}

	// now load the actual xAttrs data
	xAttrEnd := binary.LittleEndian.Uint64(b[:8])
	xAttrData := make([]byte, 0)
	offsetMap := map[uint32]uint32{0: 0}
	for i := xAttrStart; i < xAttrEnd; {
		uncompressed, size, err = fs.readMetaBlock(file, c, int64(i))
		if err != nil {
			return nil, fmt.Errorf("error reading xattr data meta block at position %d: %v", i, err)
		}
		xAttrData = append(xAttrData, uncompressed...)
		i += uint64(size)
		offsetMap[uint32(i-xAttrStart)] = uint32(len(xAttrData))
	}

	// now have all of the indexes and metadata loaded
	// need to pass it the offset of the beginning of the id table from the beginning of the disk
	return parseXattrsTable(xAttrData, bIndex, offsetMap, c)
}

//nolint:unparam,unused,revive // this does not use compressor yet, but only because we have not yet added support
func parseXattrsTable(bUIDXattr, bIndex []byte, offsetMap map[uint32]uint32, c Compressor) (*xAttrTable, error) {
	// create the ID list
	var (
		xAttrIDList []*xAttrIndex
	)

	entrySize := int(xAttrIDEntrySize)
	for i := 0; i+entrySize <= len(bIndex); i += entrySize {
		entry, err := parseXAttrIndex(bIndex[i:], offsetMap)
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
func readUidsGids(s *superblock, file backend.File, c Compressor) ([]uint32, error) {
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
		fs           = &FileSystem{}
	)

	data := make([]byte, 0)
	// convert those into indexes
	for i := 0; i+8-1 < len(b); i += 8 {
		locn := binary.LittleEndian.Uint64(b[i : i+8])
		uncompressed, _, err = fs.readMetaBlock(file, c, int64(locn))
		if err != nil {
			return nil, fmt.Errorf("error reading uidgid index meta block %d at position %d: %v", i, locn, err)
		}
		data = append(data, uncompressed...)
	}

	// now have all of the data loaded
	return parseIDTable(data), nil
}
