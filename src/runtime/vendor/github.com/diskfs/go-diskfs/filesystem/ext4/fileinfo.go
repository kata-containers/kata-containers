package ext4

import (
	"os"
	"time"
)

// FileInfo represents the information for an individual file
// it fulfills os.FileInfo interface
type FileInfo struct {
	modTime time.Time
	mode    os.FileMode
	name    string
	size    int64
	isDir   bool
}

// IsDir abbreviation for Mode().IsDir()
func (fi *FileInfo) IsDir() bool {
	return fi.isDir
}

// ModTime modification time
func (fi *FileInfo) ModTime() time.Time {
	return fi.modTime
}

// Mode returns file mode
func (fi *FileInfo) Mode() os.FileMode {
	return fi.mode
}

// Name base name of the file
//
//	will return the long name of the file. If none exists, returns the shortname and extension
func (fi *FileInfo) Name() string {
	return fi.name
}

// Size length in bytes for regular files
func (fi *FileInfo) Size() int64 {
	return fi.size
}

// Sys underlying data source - not supported yet and so will return nil
func (fi *FileInfo) Sys() interface{} {
	return nil
}
