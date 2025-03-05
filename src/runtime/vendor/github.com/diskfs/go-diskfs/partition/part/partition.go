package part

import (
	"io"

	"github.com/diskfs/go-diskfs/backend"
)

// Partition reference to an individual partition on disk
type Partition interface {
	GetSize() int64
	GetStart() int64
	ReadContents(backend.File, io.Writer) (int64, error)
	WriteContents(backend.WritableFile, io.Reader) (uint64, error)
	UUID() string
}
