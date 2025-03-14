package backend

import (
	"errors"
	"io"
	"io/fs"
	"os"
)

var (
	ErrIncorrectOpenMode = errors.New("disk file or device not open for write")
	ErrNotSuitable       = errors.New("backing file is not suitable")
)

type File interface {
	fs.File
	io.ReaderAt
	io.Seeker
	io.Closer
}

type WritableFile interface {
	File
	io.WriterAt
}

type Storage interface {
	File
	// OS-specific file for ioctl calls via fd
	Sys() (*os.File, error)
	// file for read-write operations
	Writable() (WritableFile, error)
}
