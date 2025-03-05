package fat32

import (
	"encoding/binary"
	"errors"
	"fmt"
)

// dos331BPB is the DOS 3.31 BIOS Parameter Block
type dos331BPB struct {
	dos20BPB        *dos20BPB // Dos20BPB holds the embedded DOS 2.0 BPB
	sectorsPerTrack uint16    // SectorsPerTrack is number of sectors per track. May be unused when LBA-only access is in place, but should store some value for safety.
	heads           uint16    // Heads is the number of heads. May be unused when LBA-only access is in place, but should store some value for safety. Maximum 255.
	hiddenSectors   uint32    // HiddenSectors is the number of hidden sectors preceding the partition that contains the FAT volume. Should be 0 on non-partitioned media.
	totalSectors    uint32    // TotalSectors is the total sectors if too many to fit into the DOS 2.0 BPB TotalSectors. In practice, if the DOS 2.0 TotalSectors is 0 and this is non-zero, use this one. For partitioned media, this and the DOS 2.0 BPB entry may be zero, and should retrieve information from each partition. For FAT32 systems, both also can be zero, even on non-partitioned, and use FileSystemType in DOS 7.1 EBPB as a 64-bit TotalSectors instead.
}

func (bpb *dos331BPB) equal(a *dos331BPB) bool {
	if (bpb == nil && a != nil) || (a == nil && bpb != nil) {
		return false
	}
	if bpb == nil && a == nil {
		return true
	}
	return *bpb.dos20BPB == *a.dos20BPB &&
		bpb.sectorsPerTrack == a.sectorsPerTrack &&
		bpb.heads == a.heads &&
		bpb.hiddenSectors == a.hiddenSectors &&
		bpb.totalSectors == a.totalSectors
}

// dos331BPBFromBytes reads the DOS 3.31 BIOS Parameter Block from a slice of exactly 25 bytes
func dos331BPBFromBytes(b []byte) (*dos331BPB, error) {
	if b == nil || len(b) != 25 {
		return nil, errors.New("cannot read DOS 3.31 BPB from invalid byte slice, must be precisely 25 bytes ")
	}
	bpb := dos331BPB{}
	dos20bpb, err := dos20BPBFromBytes(b[0:13])
	if err != nil {
		return nil, fmt.Errorf("error reading embedded DOS 2.0 BPB: %v", err)
	}
	bpb.dos20BPB = dos20bpb
	bpb.sectorsPerTrack = binary.LittleEndian.Uint16(b[13:15])
	bpb.heads = binary.LittleEndian.Uint16(b[15:17])
	bpb.hiddenSectors = binary.LittleEndian.Uint32(b[17:21])
	bpb.totalSectors = binary.LittleEndian.Uint32(b[21:25])
	return &bpb, nil
}

// ToBytes returns the bytes for a DOS 3.31 BIOS Parameter Block, ready to be written to disk
func (bpb *dos331BPB) toBytes() []byte {
	b := make([]byte, 25)
	dos20Bytes := bpb.dos20BPB.toBytes()
	copy(b[0:13], dos20Bytes)
	binary.LittleEndian.PutUint16(b[13:15], bpb.sectorsPerTrack)
	binary.LittleEndian.PutUint16(b[15:17], bpb.heads)
	binary.LittleEndian.PutUint32(b[17:21], bpb.hiddenSectors)
	binary.LittleEndian.PutUint32(b[21:25], bpb.totalSectors)
	return b
}
