// Package filesystem provides interfaces and constants required for filesystem implementations.
// All interesting implementations are in subpackages, e.g. github.com/diskfs/go-diskfs/filesystem/fat32
package filesystem

import (
	"errors"
	"os"
)

var (
	ErrNotSupported       = errors.New("method not supported by this filesystem")
	ErrNotImplemented     = errors.New("method not implemented (patches are welcome)")
	ErrReadonlyFilesystem = errors.New("read-only filesystem")
)

// FileSystem is a reference to a single filesystem on a disk
type FileSystem interface {
	// Type return the type of filesystem
	Type() Type
	// Mkdir make a directory
	Mkdir(pathname string) error
	// creates a filesystem node (file, device special file, or named pipe) named pathname,
	// with attributes specified by mode and dev
	Mknod(pathname string, mode uint32, dev int) error
	// creates a new link (also known as a hard link) to an existing file.
	Link(oldpath, newpath string) error
	// creates a symbolic link named linkpath which contains the string target.
	Symlink(oldpath, newpath string) error
	// Chmod changes the mode of the named file to mode. If the file is a symbolic link,
	// it changes the mode of the link's target.
	Chmod(name string, mode os.FileMode) error
	// Chown changes the numeric uid and gid of the named file. If the file is a symbolic link,
	// it changes the uid and gid of the link's target. A uid or gid of -1 means to not change that value
	Chown(name string, uid, gid int) error
	// ReadDir read the contents of a directory
	ReadDir(pathname string) ([]os.FileInfo, error)
	// OpenFile open a handle to read or write to a file
	OpenFile(pathname string, flag int) (File, error)
	// Rename renames (moves) oldpath to newpath. If newpath already exists and is not a directory, Rename replaces it.
	Rename(oldpath, newpath string) error
	// removes the named file or (empty) directory.
	Remove(pathname string) error
	// Label get the label for the filesystem, or "" if none. Be careful to trim it, as it may contain
	// leading or following whitespace. The label is passed as-is and not cleaned up at all.
	Label() string
	// SetLabel changes the label on the writable filesystem. Different file system may hav different
	// length constraints.
	SetLabel(label string) error
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
	// TypeExt4 is an ext4 compatible filesystem
	TypeExt4
)
