package squashfs

import (
	"os"
	"time"
)

// finalizeFileInfo is a file info useful for finalization
// fulfills os.FileInfo
//
//	Name() string       // base name of the file
//	Size() int64        // length in bytes for regular files; system-dependent for others
//	Mode() FileMode     // file mode bits
//	ModTime() time.Time // modification time
//	IsDir() bool        // abbreviation for Mode().IsDir()
//	Sys() interface{}   // underlying data source (can return nil)
//
//nolint:structcheck // we are willing to leave unused elements here so that we can know their reference
type finalizeFileInfo struct {
	path              string
	target            string
	location          uint32
	recordSize        uint8
	depth             int
	name              string
	size              int64
	mode              os.FileMode
	modTime           time.Time
	isDir             bool
	isRoot            bool
	bytes             [][]byte
	parent            *finalizeFileInfo
	children          []*finalizeFileInfo
	content           []byte
	dataLocation      int64
	fileType          fileType
	inode             inode
	inodeLocation     blockPosition
	xattrs            map[string]string
	xAttrIndex        uint32
	links             uint32
	blocks            []*blockData
	startBlock        uint64
	fragment          *fragmentRef
	uid               uint32
	gid               uint32
	directory         *directory
	directoryLocation blockPosition
}

func (fi *finalizeFileInfo) Name() string {
	return fi.name
}
func (fi *finalizeFileInfo) Size() int64 {
	return fi.size
}
func (fi *finalizeFileInfo) Mode() os.FileMode {
	return fi.mode
}
func (fi *finalizeFileInfo) ModTime() time.Time {
	return fi.modTime
}
func (fi *finalizeFileInfo) IsDir() bool {
	return fi.isDir
}
func (fi *finalizeFileInfo) Sys() interface{} {
	return nil
}

// add depth to all children
func (fi *finalizeFileInfo) addProperties(depth int) {
	fi.depth = depth
	for _, e := range fi.children {
		e.parent = fi
		e.addProperties(depth + 1)
	}
}

type fragmentRef struct {
	block  uint32
	offset uint32
}
