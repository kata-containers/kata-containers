package part

import (
	"io"

	"github.com/diskfs/go-diskfs/util"
)

// Partition reference to an individual partition on disk
type Partition interface {
	GetSize() int64
	GetStart() int64
	ReadContents(util.File, io.Writer) (int64, error)
	WriteContents(util.File, io.Reader) (uint64, error)
}
