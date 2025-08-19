package iso9660

import (
	"fmt"
	"io"
	"os"
)

// File represents a single file in an iso9660 filesystem
//
//	it is NOT used when working in a workspace, where we just use the underlying OS
type File struct {
	*directoryEntry
	isReadWrite bool
	isAppend    bool
	offset      int64
	closed      bool
}

// Read reads up to len(b) bytes from the File.
// It returns the number of bytes read and any error encountered.
// At end of file, Read returns 0, io.EOF
// reads from the last known offset in the file from last read or write
// use Seek() to set at a particular point
func (fl *File) Read(b []byte) (int, error) {
	if fl == nil || fl.closed {
		return 0, os.ErrClosed
	}
	// we have the DirectoryEntry, so we can get the starting location and size
	// since iso9660 files are contiguous, we only need the starting location and size
	//   to get the entire file
	fs := fl.filesystem
	size := int(fl.size) - int(fl.offset)
	location := int(fl.location)
	maxRead := size
	file := fs.file

	// if there is nothing left to read, just return EOF
	if size <= 0 {
		return 0, io.EOF
	}

	// we stop when we hit the lesser of
	//   1- len(b)
	//   2- file end
	if len(b) < maxRead {
		maxRead = len(b)
	}

	// just read the requested number of bytes and change our offset
	_, err := file.ReadAt(b[0:maxRead], int64(location)*fs.blocksize+fl.offset)
	if err != nil && err != io.EOF {
		return 0, err
	}

	fl.offset += int64(maxRead)
	var retErr error
	if fl.offset >= int64(fl.size) {
		retErr = io.EOF
	}
	return maxRead, retErr
}

// Write writes len(b) bytes to the File.
//
//	you cannot write to an iso, so this returns an error
func (fl *File) Write(p []byte) (int, error) {
	return 0, fmt.Errorf("cannot write to a read-only iso filesystem")
}

// Seek set the offset to a particular point in the file
func (fl *File) Seek(offset int64, whence int) (int64, error) {
	if fl == nil || fl.closed {
		return 0, os.ErrClosed
	}
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

func (fl *File) Location() uint32 {
	return fl.location
}

// Close close the file
func (fl *File) Close() error {
	fl.closed = true
	return nil
}
