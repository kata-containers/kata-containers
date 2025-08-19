package squashfs

import (
	"encoding/binary"
	"fmt"
)

//nolint:deadcode,varcheck,unused // we need these references in the future
const (
	fragmentEntriesPerBlock = 512
	fragmentEntrySize       = 16
)

type fragmentEntry struct {
	start      uint64
	size       uint32
	compressed bool
}

//nolint:structcheck,deadcode,unused // we need these references in the future
type fragmentTable struct {
	entries []*fragmentEntry
}

func (f *fragmentEntry) toBytes() []byte {
	b := make([]byte, 16)
	binary.LittleEndian.PutUint64(b[0:8], f.start)
	size := f.size
	if !f.compressed {
		size |= (1 << 24)
	}
	binary.LittleEndian.PutUint32(b[8:12], size)
	return b
}
func parseFragmentEntry(b []byte) (*fragmentEntry, error) {
	target := 16
	if len(b) < target {
		return nil, fmt.Errorf("mismatched fragment entry size, received %d bytes, less than minimum %d", len(b), target)
	}
	start := binary.LittleEndian.Uint64(b[0:8])
	size := binary.LittleEndian.Uint32(b[8:12])
	unCompFlag := uint32(1 << 24)
	compressed := true
	if size&unCompFlag == unCompFlag {
		compressed = false
	}
	size &= 0xffffff
	return &fragmentEntry{
		size:       size,
		compressed: compressed,
		start:      start,
	}, nil
}
