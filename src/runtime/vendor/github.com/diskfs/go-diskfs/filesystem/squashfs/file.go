package squashfs

import (
	"fmt"
	"io"
	"os"

	"github.com/diskfs/go-diskfs/filesystem"
)

// File represents a single file in a squashfs filesystem
//
//	it is NOT used when working in a workspace, where we just use the underlying OS
//	note that the inode for a file can be the basicFile or extendedFile. We just use extendedFile to
//	include all of the data
type File struct {
	*extendedFile
	isReadWrite   bool
	isAppend      bool
	offset        int64
	filesystem    *FileSystem
	blockLocation int64  // the position of the last block decompressed
	block         []byte // the actual last block decompressed
}

// Read reads up to len(b) bytes from the File.
// It returns the number of bytes read and any error encountered.
// At end of file, Read returns 0, io.EOF
// reads from the last known offset in the file from last read or write
// use Seek() to set at a particular point
func (fl *File) Read(b []byte) (int, error) {
	if fl == nil || fl.filesystem == nil {
		return 0, os.ErrClosed
	}
	// squashfs files are *mostly* contiguous, we only need the starting location and size for whole blocks
	// if there are fragments, we need the location of those as well

	// logic:
	// 1- find the uncompressed blocksize from the superblock
	// 2- calculate the relative blocks needed for this file
	//  e.g. if uncompressed blocksize is 100 bytes, and we want from byte 240 for 200 bytes,
	//       then we need blocks 2,3,4 of this file
	// 3- find the compressed blockSizes for this file from inode.blockSizes
	//       this tells us how many bytes to read for each block from the disk
	// 4- find the starting location for the first block for this file from inode.startBlock
	//      e.g. if starting block is at position 10245, then we want blocks 27,28,29 from the disk
	// 5- read in and uncompress the necessary blocks
	fs := fl.filesystem
	size := fl.size() - fl.offset
	location := int64(fl.startBlock)
	maxRead := len(b)

	// if there is nothing left to read, just return EOF
	if size <= 0 {
		return 0, io.EOF
	}

	// we stop when we hit the lesser of
	//   1- len(b)
	//   2- file end
	if size < int64(maxRead) {
		maxRead = int(size)
	}

	// just read the requested number of bytes and change our offset
	// figure out which block number has the bytes we are looking for
	startBlock := int(fl.offset / fs.blocksize)
	endBlock := int((fl.offset + int64(maxRead) - 1) / fs.blocksize)

	// do we end in fragment territory?
	fragments := false
	if endBlock >= len(fl.blockSizes) {
		fragments = true
		endBlock--
	}

	read := 0
	offsetEnd := fl.offset + int64(maxRead)
	pos := int64(0)

	// send input to b, clipping as appropriate
	outputBlock := func(input []byte) {
		inputSize := int64(len(input))
		start := fl.offset - pos
		end := offsetEnd - pos
		if start >= 0 && start < inputSize {
			if end > inputSize {
				end = inputSize
			}
			n := copy(b[read:], input[start:end])
			read += n
			fl.offset += int64(n)
		}
	}

	// we need to cycle through all of the blocks to find where the desired one starts
	for i, block := range fl.blockSizes {
		if i > endBlock || read >= maxRead {
			break
		}
		// if we are in the range of desired ones, read it in
		if i >= startBlock {
			if int64(block.size) > fs.blocksize {
				return read, fmt.Errorf("unexpected block.size=%d > fs.blocksize=%d", block.size, fs.blocksize)
			}
			var input []byte
			if fl.blockLocation == location && fl.block != nil {
				// Read last block from cache
				input = fl.block
			} else {
				var err error
				input, err = fs.readBlock(location, block.compressed, block.size)
				if err != nil {
					return read, fmt.Errorf("error reading data block %d from squashfs: %v", i, err)
				}
				// Cache the last block
				fl.blockLocation = location
				fl.block = input
			}
			outputBlock(input)
		}
		location += int64(block.size)
		pos += fs.blocksize
	}

	// did we have a fragment to read?
	if read < maxRead && fragments {
		if fl.fragmentBlockIndex == 0xffffffff {
			return read, fmt.Errorf("expecting fragment to read %d bytes but no fragment found", maxRead-read)
		}
		input, err := fs.readFragment(fl.fragmentBlockIndex, fl.fragmentOffset, fl.size()%fs.blocksize)
		if err != nil {
			return read, fmt.Errorf("error reading fragment block %d from squashfs: %v", fl.fragmentBlockIndex, err)
		}
		pos = int64(len(fl.blockSizes)) * fs.blocksize
		outputBlock(input)
	}
	var retErr error
	if fl.offset >= fl.size() {
		retErr = io.EOF
	} else if read == 0 {
		retErr = fmt.Errorf("internal error: read no bytes")
	}
	return read, retErr
}

// Write writes len(b) bytes to the File.
//
//	you cannot write to a finished squashfs, so this returns an error
//
//nolint:unused,revive // but it is important to implement the interface
func (fl *File) Write(p []byte) (int, error) {
	return 0, filesystem.ErrReadonlyFilesystem
}

// Seek set the offset to a particular point in the file
func (fl *File) Seek(offset int64, whence int) (int64, error) {
	if fl == nil || fl.filesystem == nil {
		return 0, os.ErrClosed
	}
	newOffset := int64(0)
	switch whence {
	case io.SeekStart:
		newOffset = offset
	case io.SeekEnd:
		newOffset = fl.size() - offset
	case io.SeekCurrent:
		newOffset = fl.offset + offset
	}
	if newOffset < 0 {
		return fl.offset, fmt.Errorf("cannot set offset %d before start of file", offset)
	}
	fl.offset = newOffset
	return fl.offset, nil
}

// Close close the file
func (fl *File) Close() error {
	fl.filesystem = nil
	return nil
}
