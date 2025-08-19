package partition

import (
	"github.com/diskfs/go-diskfs/partition/part"
	"github.com/diskfs/go-diskfs/util"
)

// Table reference to a partitioning table on disk
type Table interface {
	Type() string
	Write(util.File, int64) error
	GetPartitions() []part.Partition
}
