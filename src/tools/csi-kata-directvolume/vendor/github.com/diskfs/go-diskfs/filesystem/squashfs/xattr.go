package squashfs

import (
	"encoding/binary"
	"fmt"
)

const (
	xAttrIDEntrySize      uint32 = 16
	xAttrHeaderSize       uint32 = 16
	noXattrInodeFlag      uint32 = 0xffffffff
	noXattrSuperblockFlag uint64 = 0xffffffffffffffff
)

type xAttrIndex struct {
	pos   uint64
	count uint32
	size  uint32
}

func parseXAttrIndex(b []byte) (*xAttrIndex, error) {
	if len(b) < int(xAttrIDEntrySize) {
		return nil, fmt.Errorf("cannot parse xAttr Index of size %d less than minimum %d", len(b), xAttrIDEntrySize)
	}
	return &xAttrIndex{
		pos:   binary.LittleEndian.Uint64(b[0:8]),
		count: binary.LittleEndian.Uint32(b[8:12]),
		size:  binary.LittleEndian.Uint32(b[12:16]),
	}, nil
}

type xAttrTable struct {
	list []*xAttrIndex
	data []byte
}

func (x *xAttrTable) find(pos int) (map[string]string, error) {
	if pos >= len(x.list) {
		return nil, fmt.Errorf("position %d is greater than list size %d", pos, len(x.list))
	}
	entry := x.list[pos]
	b := x.data[entry.pos:]
	count := entry.count
	ptr := 0
	xattrs := map[string]string{}
	for i := 0; i < int(count); i++ {
		// must be 4 bytes for header
		if len(b[pos:]) < 4 {
			return nil, fmt.Errorf("insufficient bytes %d to read the xattr at position %d", len(b[ptr:]), ptr)
		}
		// get the type and size
		//   xType := binary.LittleEndian.Uint16(b[ptr : ptr+2])
		xSize := int(binary.LittleEndian.Uint16(b[ptr+2 : ptr+4]))
		nameStart := ptr + 4
		valHeaderStart := nameStart + xSize
		valStart := valHeaderStart + 4
		// make sure we have enough bytes
		if len(b[nameStart:]) < xSize {
			return nil, fmt.Errorf("xattr header has size %d, but only %d bytes available to read at position %d", xSize, len(b[pos+4:]), ptr)
		}
		if xSize < 1 {
			return nil, fmt.Errorf("no name given for xattr at position %d", ptr)
		}
		key := string(b[nameStart : nameStart+xSize])
		// read the size of the value
		if len(b[valHeaderStart:]) < 4 {
			return nil, fmt.Errorf("insufficient bytes %d to read the xattr value at position %d", len(b[valHeaderStart:]), ptr)
		}
		valSize := int(binary.LittleEndian.Uint32(b[valHeaderStart:valStart]))
		if len(b[valStart:]) < valSize {
			return nil, fmt.Errorf("xattr value has size %d, but only %d bytes available to read at position %d", valSize, len(b[valStart:]), ptr)
		}
		val := string(b[valStart : valStart+valSize])
		xattrs[key] = val

		// increment the position pointer
		ptr += valStart + valSize
	}
	return xattrs, nil
}
