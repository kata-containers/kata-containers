package ext4

import (
	"fmt"
	"io"
)

// File represents a single file in an ext4 filesystem
type File struct {
	*directoryEntry
	*inode
	isReadWrite bool
	isAppend    bool
	offset      int64
	filesystem  *FileSystem
	extents     extents
}

// Read reads up to len(b) bytes from the File.
// It returns the number of bytes read and any error encountered.
// At end of file, Read returns 0, io.EOF
// reads from the last known offset in the file from last read or write
// use Seek() to set at a particular point
func (fl *File) Read(b []byte) (int, error) {
	var (
		fileSize  = int64(fl.size)
		blocksize = uint64(fl.filesystem.superblock.blockSize)
	)
	if fl.offset >= fileSize {
		return 0, io.EOF
	}

	// Calculate the number of bytes to read
	bytesToRead := int64(len(b))
	if fl.offset+bytesToRead > fileSize {
		bytesToRead = fileSize - fl.offset
	}

	// Create a buffer to hold the bytes to be read
	readBytes := int64(0)
	b = b[:bytesToRead]

	// the offset given for reading is relative to the file, so we need to calculate
	// where these are in the extents relative to the file
	readStartBlock := uint64(fl.offset) / blocksize
	for _, e := range fl.extents {
		// if the last block of the extent is before the first block we want to read, skip it
		if uint64(e.fileBlock)+uint64(e.count) < readStartBlock {
			continue
		}
		// extentSize is the number of bytes on the disk for the extent
		extentSize := int64(e.count) * int64(blocksize)
		// where do we start and end in the extent?
		startPositionInExtent := fl.offset - int64(e.fileBlock)*int64(blocksize)
		leftInExtent := extentSize - startPositionInExtent
		// how many bytes are left to read
		toReadInOffset := bytesToRead - readBytes
		if toReadInOffset > leftInExtent {
			toReadInOffset = leftInExtent
		}
		// read those bytes
		startPosOnDisk := e.startingBlock*blocksize + uint64(startPositionInExtent)
		b2 := make([]byte, toReadInOffset)
		read, err := fl.filesystem.backend.ReadAt(b2, int64(startPosOnDisk))
		if err != nil {
			return int(readBytes), fmt.Errorf("failed to read bytes: %v", err)
		}
		copy(b[readBytes:], b2[:read])
		readBytes += int64(read)
		fl.offset += int64(read)

		if readBytes >= bytesToRead {
			break
		}
	}
	var err error
	if fl.offset >= fileSize {
		err = io.EOF
	}

	return int(readBytes), err
}

// Write writes len(b) bytes to the File.
// It returns the number of bytes written and an error, if any.
// returns a non-nil error when n != len(b)
// writes to the last known offset in the file from last read or write
// use Seek() to set at a particular point
func (fl *File) Write(b []byte) (int, error) {
	var (
		fileSize           = int64(fl.size)
		originalFileSize   = int64(fl.size)
		blockCount         = fl.blocks
		originalBlockCount = fl.blocks
		blocksize          = uint64(fl.filesystem.superblock.blockSize)
	)
	if !fl.isReadWrite {
		return 0, fmt.Errorf("file is not open for writing")
	}

	// if adding these bytes goes past the filesize, update the inode filesize to the new size and write the inode
	// if adding these bytes goes past the total number of blocks, add more blocks, update the inode block count and write the inode
	// if the offset is greater than the filesize, update the inode filesize to the offset
	if fl.offset >= fileSize {
		fl.size = uint64(fl.offset)
	}

	// Calculate the number of bytes to write
	bytesToWrite := int64(len(b))

	offsetAfterWrite := fl.offset + bytesToWrite
	if offsetAfterWrite > int64(fl.size) {
		fl.size = uint64(fl.offset + bytesToWrite)
	}

	// calculate the number of blocks in the file post-write
	newBlockCount := fl.size / blocksize
	if fl.size%blocksize > 0 {
		newBlockCount++
	}
	blocksNeeded := newBlockCount - blockCount
	bytesNeeded := blocksNeeded * blocksize
	if newBlockCount > blockCount {
		newExtents, err := fl.filesystem.allocateExtents(bytesNeeded, &fl.extents)
		if err != nil {
			return 0, fmt.Errorf("could not allocate disk space for file %w", err)
		}
		extentTreeParsed, err := extendExtentTree(fl.inode.extents, newExtents, fl.filesystem, nil)
		if err != nil {
			return 0, fmt.Errorf("could not convert extents into tree: %w", err)
		}
		fl.inode.extents = extentTreeParsed
		fl.blocks = newBlockCount
	}

	if originalFileSize != int64(fl.size) || originalBlockCount != fl.blocks {
		err := fl.filesystem.writeInode(fl.inode)
		if err != nil {
			return 0, fmt.Errorf("could not write inode: %w", err)
		}
	}

	writtenBytes := int64(0)

	// the offset given for reading is relative to the file, so we need to calculate
	// where these are in the extents relative to the file
	writeStartBlock := uint64(fl.offset) / blocksize

	writableFile, err := fl.filesystem.backend.Writable()
	if err != nil {
		return -1, err
	}

	for _, e := range fl.extents {
		// if the last block of the extent is before the first block we want to write, skip it
		if uint64(e.fileBlock)+uint64(e.count) < writeStartBlock {
			continue
		}
		// extentSize is the number of bytes on the disk for the extent
		extentSize := int64(e.count) * int64(blocksize)
		// where do we start and end in the extent?
		startPositionInExtent := fl.offset - int64(e.fileBlock)*int64(blocksize)
		leftInExtent := extentSize - startPositionInExtent
		// how many bytes are left in the extent?
		toWriteInOffset := bytesToWrite - writtenBytes
		if toWriteInOffset > leftInExtent {
			toWriteInOffset = leftInExtent
		}
		// read those bytes
		startPosOnDisk := e.startingBlock*blocksize + uint64(startPositionInExtent)
		b2 := make([]byte, toWriteInOffset)
		copy(b2, b[writtenBytes:])
		written, err := writableFile.WriteAt(b2, int64(startPosOnDisk))
		if err != nil {
			return int(writtenBytes), fmt.Errorf("failed to read bytes: %v", err)
		}
		writtenBytes += int64(written)
		fl.offset += int64(written)

		if written >= len(b) {
			break
		}
	}

	if fl.offset >= fileSize {
		err = io.EOF
	}

	return int(writtenBytes), err
}

// Seek set the offset to a particular point in the file
func (fl *File) Seek(offset int64, whence int) (int64, error) {
	newOffset := int64(0)
	switch whence {
	case io.SeekStart:
		newOffset = offset
	case io.SeekEnd:
		newOffset = int64(fl.size) + offset
	case io.SeekCurrent:
		newOffset = fl.offset + offset
	}
	if newOffset < 0 {
		return fl.offset, fmt.Errorf("cannot set offset %d before start of file", offset)
	}
	fl.offset = newOffset
	return fl.offset, nil
}

// Close close a file that is being read
func (fl *File) Close() error {
	*fl = File{}
	return nil
}
