package squashfs

import (
	"os"
	"time"
)

// FileStat is the extended data underlying a single file, similar to https://golang.org/pkg/syscall/#Stat_t
type FileStat struct {
	uid    uint32
	gid    uint32
	xattrs map[string]string
}

func (f *FileStat) equal(o *FileStat) bool {
	if f.uid != o.uid || f.gid != o.gid {
		return false
	}
	if len(f.xattrs) != len(o.xattrs) {
		return false
	}
	for k, v := range f.xattrs {
		ov, ok := o.xattrs[k]
		if !ok {
			return false
		}
		if ov != v {
			return false
		}
	}
	return true
}

// UID get uid of file
func (f *FileStat) UID() uint32 {
	return f.uid
}

// GID get gid of file
func (f *FileStat) GID() uint32 {
	return f.gid
}

// Xattrs get extended attributes of file
func (f *FileStat) Xattrs() map[string]string {
	return f.xattrs
}

// directoryEntry is a single directory entry
// it combines information from inode and the actual entry
// also fulfills os.FileInfo
//
//	Name() string       // base name of the file
//	Size() int64        // length in bytes for regular files; system-dependent for others
//	Mode() FileMode     // file mode bits
//	ModTime() time.Time // modification time
//	IsDir() bool        // abbreviation for Mode().IsDir()
//	Sys() interface{}   // underlying data source (can return nil)
type directoryEntry struct {
	isSubdirectory bool
	name           string
	size           int64
	modTime        time.Time
	mode           os.FileMode
	inode          inode
	sys            FileStat
}

func (d *directoryEntry) equal(o *directoryEntry) bool {
	if o == nil {
		return false
	}
	if !d.sys.equal(&o.sys) {
		return false
	}
	if d.inode == nil && o.inode == nil {
		return true
	}
	if (d.inode == nil && o.inode != nil) || (d.inode != nil && o.inode == nil) {
		return false
	}
	if !d.inode.equal(o.inode) {
		return false
	}
	return d.isSubdirectory == o.isSubdirectory && d.name == o.name && d.size == o.size && d.modTime == o.modTime && d.mode == o.mode
}

// Name string       // base name of the file
func (d *directoryEntry) Name() string {
	return d.name
}

// Size int64        // length in bytes for regular files; system-dependent for others
func (d *directoryEntry) Size() int64 {
	return d.size
}

// IsDir bool        // abbreviation for Mode().IsDir()
func (d *directoryEntry) IsDir() bool {
	return d.isSubdirectory
}

// ModTime time.Time // modification time
func (d *directoryEntry) ModTime() time.Time {
	return d.modTime
}

// Mode FileMode     // file mode bits
func (d *directoryEntry) Mode() os.FileMode {
	return d.mode
}

// Sys interface{}   // underlying data source (can return nil)
func (d *directoryEntry) Sys() interface{} {
	return d.sys
}
