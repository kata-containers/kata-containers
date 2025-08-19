package iso9660

import (
	"encoding/binary"
)

// pathTable represents an on-iso path table
type pathTable struct {
	records []*pathTableEntry
}

type pathTableEntry struct {
	nameSize      uint8
	size          uint16
	extAttrLength uint8
	location      uint32
	parentIndex   uint16
	dirname       string
}

func (pt *pathTable) equal(b *pathTable) bool {
	switch {
	case (pt == nil && b != nil) || (pt != nil && b == nil):
		return false
	case len(pt.records) != len(b.records):
		return false
	default:
		for i, e := range pt.records {
			if *e != *b.records[i] {
				return false
			}
		}
	}
	return true
}

func (pt *pathTable) names() []string {
	ret := make([]string, len(pt.records))
	for i, v := range pt.records {
		ret[i] = v.dirname
	}
	return ret
}

func (pt *pathTable) toLBytes() []byte {
	b := make([]byte, 0)
	for _, e := range pt.records {
		name := []byte(e.dirname)
		nameSize := len(name)
		size := 8 + uint16(nameSize)
		if nameSize%2 != 0 {
			size++
		}

		b2 := make([]byte, size)
		b2[0] = uint8(nameSize)
		b2[1] = e.extAttrLength
		binary.LittleEndian.PutUint32(b2[2:6], e.location)
		binary.LittleEndian.PutUint16(b2[6:8], e.parentIndex)
		copy(b2[8:8+nameSize], name)
		if nameSize%2 != 0 {
			b2[8+nameSize] = 0
		}
		b = append(b, b2...)
	}
	return b
}
func (pt *pathTable) toMBytes() []byte {
	b := make([]byte, 0)
	for _, e := range pt.records {
		name := []byte(e.dirname)
		nameSize := len(name)
		size := 8 + uint16(nameSize)
		if nameSize%2 != 0 {
			size++
		}

		b2 := make([]byte, size)
		b2[0] = uint8(nameSize)
		b2[1] = e.extAttrLength
		binary.BigEndian.PutUint32(b2[2:6], e.location)
		binary.BigEndian.PutUint16(b2[6:8], e.parentIndex)
		copy(b2[8:8+nameSize], name)
		if nameSize%2 != 0 {
			b2[8+nameSize] = 0
		}
		b = append(b, b2...)
	}
	return b
}

// getLocation gets the location of the extent that contains this path
// we can get the size because the first record always points to the current directory
func (pt *pathTable) getLocation(p string) uint32 {
	// break path down into parts and levels
	parts := splitPath(p)
	// level represents the level of the parent
	var level uint16 = 1
	var location uint32
	if len(parts) == 0 {
		location = pt.records[0].location
	} else {
		current := parts[0]
		// loop through the path table until we find our entry
		// we always can go forward because of the known depth ordering of path table
		for i, entry := range pt.records {
			// did we find a match for our current level?
			if entry.parentIndex == level && entry.dirname == current {
				level = uint16(i)
				if len(parts) > 1 {
					parts = parts[1:]
				} else {
					// this is the final one, we found it, keep it
					location = entry.location
					break
				}
			}
		}
	}
	return location
}

// parsePathTable load pathtable bytes into structures
func parsePathTable(b []byte) *pathTable {
	totalSize := len(b)
	entries := make([]*pathTableEntry, 0, 20)
	for i := 0; i < totalSize; {
		var nameSize = b[i]
		// is it zeroes? If so, we are at the end
		if nameSize == 0 {
			break
		}
		size := 8 + uint16(nameSize)
		if nameSize%2 != 0 {
			size++
		}
		var extAttrSize = b[i+1]
		location := binary.LittleEndian.Uint32(b[i+2 : i+6])
		parent := binary.LittleEndian.Uint16(b[i+6 : i+8])
		name := string(b[i+8 : i+8+int(nameSize)])
		entry := &pathTableEntry{
			nameSize:      nameSize,
			size:          size,
			extAttrLength: extAttrSize,
			location:      location,
			parentIndex:   parent,
			dirname:       name,
		}
		entries = append(entries, entry)
		i += int(size)
	}
	return &pathTable{
		records: entries,
	}
}
