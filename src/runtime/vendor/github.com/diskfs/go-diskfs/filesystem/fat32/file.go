package fat32

import (
	"fmt"
	"io"
	"os"

	"github.com/diskfs/go-diskfs/filesystem"
)

// File represents a single file in a FAT32 filesystem
type File struct {
	*directoryEntry
	isReadWrite bool
	isAppend    bool
	offset      int64
	parent      *Directory
	filesystem  *FileSystem
}

// Get the full cluster chain of the File.
// Getting this file system internal info can be beneficial for some low-level operations, such as:
// - Performing secure erase.
// - Detecting file fragmentation.
// - Passing Disk locations to a different tool that can work with it.
func (fl *File) GetClusterChain() ([]uint32, error) {
	if fl == nil || fl.filesystem == nil {
		return nil, os.ErrClosed
	}

	fs := fl.filesystem
	clusters, err := fs.getClusterList(fl.clusterLocation)
	if err != nil {
		return nil, fmt.Errorf("unable to get list of clusters for file: %v", err)
	}

	return clusters, nil
}

type DiskRange struct {
	Offset uint64
	Length uint64
}

// Get the disk ranges occupied by the File.
// Returns an array of disk ranges, where each entry is a contiguous area on disk.
// This information is similar to that returned by GetClusterChain, just in a different format,
// directly returning disk ranges instead of FAT clusters.
func (fl *File) GetDiskRanges() ([]DiskRange, error) {
	clusters, err := fl.GetClusterChain()
	if err != nil {
		return nil, err
	}

	fs := fl.filesystem
	bytesPerCluster := uint64(fs.bytesPerCluster)
	dataStart := uint64(fs.dataStart)

	var ranges []DiskRange
	var lastCluster uint32

	for _, cluster := range clusters {
		if lastCluster != 0 && cluster == lastCluster+1 {
			// Extend the current range
			ranges[len(ranges)-1].Length += bytesPerCluster
		} else {
			// Add a new range
			offset := dataStart + uint64(cluster-2)*bytesPerCluster
			ranges = append(ranges, DiskRange{
				Offset: offset,
				Length: bytesPerCluster,
			})
		}
		lastCluster = cluster
	}

	return ranges, nil
}

// Read reads up to len(b) bytes from the File.
// It returns the number of bytes read and any error encountered.
// At end of file, Read returns 0, io.EOF
// reads from the last known offset in the file from last read or write
// and increments the offset by the number of bytes read.
// Use Seek() to set at a particular point
func (fl *File) Read(b []byte) (int, error) {
	if fl == nil || fl.filesystem == nil {
		return 0, os.ErrClosed
	}
	// we have the DirectoryEntry, so we can get the starting cluster location
	// we then get a list of the clusters, and read the data from all of those clusters
	// write the content for the file
	totalRead := 0
	fs := fl.filesystem
	bytesPerCluster := fs.bytesPerCluster
	start := int(fs.dataStart)
	size := int(fl.fileSize) - int(fl.offset)
	maxRead := size
	file := fs.backend
	clusters, err := fs.getClusterList(fl.clusterLocation)
	if err != nil {
		return totalRead, fmt.Errorf("unable to get list of clusters for file: %v", err)
	}
	clusterIndex := 0

	// if there is nothing left to read, just return EOF
	if size <= 0 {
		return totalRead, io.EOF
	}

	// we stop when we hit the lesser of
	//   1- len(b)
	//   2- file end
	if len(b) < maxRead {
		maxRead = len(b)
	}

	// figure out which cluster we start with
	if fl.offset > 0 {
		clusterIndex = int(fl.offset / int64(bytesPerCluster))
		lastCluster := clusters[clusterIndex]
		// read any partials, if needed
		remainder := fl.offset % int64(bytesPerCluster)
		if remainder != 0 {
			offset := int64(start) + int64(lastCluster-2)*int64(bytesPerCluster) + remainder
			toRead := int64(bytesPerCluster) - remainder
			if toRead > int64(len(b)) {
				toRead = int64(len(b))
			}
			_, _ = file.ReadAt(b[0:toRead], offset+fs.start)
			totalRead += int(toRead)
			clusterIndex++
		}
	}

	for i := clusterIndex; i < len(clusters); i++ {
		left := maxRead - totalRead
		toRead := bytesPerCluster
		if toRead > left {
			toRead = left
		}
		offset := int64(start) + int64(clusters[i]-2)*int64(bytesPerCluster)
		_, _ = file.ReadAt(b[totalRead:totalRead+toRead], offset+fs.start)
		totalRead += toRead
		if totalRead >= maxRead {
			break
		}
	}

	fl.offset += int64(totalRead)
	var retErr error
	if fl.offset >= int64(fl.fileSize) {
		retErr = io.EOF
	}
	return totalRead, retErr
}

// Write writes len(b) bytes to the File.
// It returns the number of bytes written and an error, if any.
// returns a non-nil error when n != len(b)
// writes to the last known offset in the file from last read or write
// and increments the offset by the number of bytes read.
// Use Seek() to set at a particular point
func (fl *File) Write(p []byte) (int, error) {
	if fl == nil || fl.filesystem == nil {
		return 0, os.ErrClosed
	}

	totalWritten := 0
	writableFile, err := fl.filesystem.backend.Writable()
	if err != nil {
		return totalWritten, err
	}

	fs := fl.filesystem
	// if the file was not opened RDWR, nothing we can do
	if !fl.isReadWrite {
		return totalWritten, filesystem.ErrReadonlyFilesystem
	}
	// what is the new file size?
	writeSize := len(p)
	oldSize := int64(fl.fileSize)
	newSize := fl.offset + int64(writeSize)
	if newSize < oldSize {
		newSize = oldSize
	}
	// 1- ensure we have space and clusters
	clusters, err := fs.allocateSpace(uint64(newSize), fl.clusterLocation)
	if err != nil {
		return 0x00, fmt.Errorf("unable to allocate clusters for file: %v", err)
	}

	// update the directory entry size for the file
	if oldSize != newSize {
		fl.fileSize = uint32(newSize)
	}
	// write the content for the file
	bytesPerCluster := fl.filesystem.bytesPerCluster
	start := int(fl.filesystem.dataStart)
	clusterIndex := 0

	// figure out which cluster we start with
	if fl.offset > 0 {
		clusterIndex = int(fl.offset) / bytesPerCluster
		lastCluster := clusters[clusterIndex]
		// write any partials, if needed
		remainder := fl.offset % int64(bytesPerCluster)
		if remainder != 0 {
			offset := int64(start) + int64(lastCluster-2)*int64(bytesPerCluster) + remainder
			toWrite := int64(bytesPerCluster) - remainder
			// max we can write
			if toWrite > int64(len(p)) {
				toWrite = int64(len(p))
			}
			_, err := writableFile.WriteAt(p[0:toWrite], offset+fs.start)
			if err != nil {
				return totalWritten, fmt.Errorf("unable to write to file: %v", err)
			}
			totalWritten += int(toWrite)
			clusterIndex++
		}
	}

	for i := clusterIndex; i < len(clusters); i++ {
		left := len(p) - totalWritten
		toWrite := bytesPerCluster
		if toWrite > left {
			toWrite = left
		}
		offset := int64(start) + int64(clusters[i]-2)*int64(bytesPerCluster)
		_, err := writableFile.WriteAt(p[totalWritten:totalWritten+toWrite], offset+fs.start)
		if err != nil {
			return totalWritten, fmt.Errorf("unable to write to file: %v", err)
		}
		totalWritten += toWrite
	}

	fl.offset += int64(totalWritten)

	// update the parent that we have changed the file size
	err = fs.writeDirectoryEntries(fl.parent)
	if err != nil {
		return 0, fmt.Errorf("error writing directory entries to disk: %v", err)
	}

	return totalWritten, nil
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
		newOffset = int64(fl.fileSize) + offset
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
