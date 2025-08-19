// Package filesystem provides interfaces and constants required for filesystem implementations.
// All interesting implementations are in subpackages, e.g. github.com/diskfs/go-diskfs/filesystem/fat32
package filesystem

import (
	"os"
)

// FileSystem is a reference to a single filesystem on a disk
type FileSystem interface {
	// Type return the type of filesystem
	Type() Type
	// Mkdir make a directory
	Mkdir(string) error
	// ReadDir read the contents of a directory
	ReadDir(string) ([]os.FileInfo, error)
	// OpenFile open a handle to read or write to a file
	OpenFile(string, int) (File, error)
	// Label get the label for the filesystem, or "" if none. Be careful to trim it, as it may contain
	// leading or following whitespace. The label is passed as-is and not cleaned up at all.
	Label() string
	// SetLabel changes the label on the writable filesystem. Different file system may hav different
	// length constraints.
	SetLabel(string) error
}

// Type represents the type of disk this is
type Type int

const (
	// TypeFat32 is a FAT32 compatible filesystem
	TypeFat32 Type = iota
	// TypeISO9660 is an iso filesystem
	TypeISO9660
	// TypeSquashfs is a squashfs filesystem
	TypeSquashfs
)
