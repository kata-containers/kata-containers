package fat32

import (
	"encoding/binary"
	"errors"
	"fmt"
)

// Dos20BPB is a DOS 2.0 BIOS Parameter Block structure
type dos20BPB struct {
	bytesPerSector       SectorSize // BytesPerSector is bytes in each sector - always should be 512
	sectorsPerCluster    uint8      // SectorsPerCluster is number of sectors per cluster
	reservedSectors      uint16     // ReservedSectors is number of reserved sectors
	fatCount             uint8      // FatCount is total number of FAT tables in the filesystem
	rootDirectoryEntries uint16     // RootDirectoryEntries is maximum number of FAT12 or FAT16 root directory entries; must be 0 for FAT32
	totalSectors         uint16     // TotalSectors is total number of sectors in the filesystem
	mediaType            uint8      // MediaType is the type of media, mostly unused
	sectorsPerFat        uint16     // SectorsPerFat is number of sectors per each table
}

// Dos20BPBFromBytes reads the DOS 2.0 BIOS Parameter Block from a slice of exactly 13 bytes
func dos20BPBFromBytes(b []byte) (*dos20BPB, error) {
	if b == nil || len(b) != 13 {
		return nil, errors.New("cannot read DOS 2.0 BPB from invalid byte slice, must be precisely 13 bytes ")
	}
	bpb := dos20BPB{}
	// make sure we have a valid sector size
	sectorSize := binary.LittleEndian.Uint16(b[0:2])
	if sectorSize != uint16(SectorSize512) {
		return nil, fmt.Errorf("invalid sector size %d provided in DOS 2.0 BPB. Must be %d", sectorSize, SectorSize512)
	}
	bpb.bytesPerSector = SectorSize512
	bpb.sectorsPerCluster = b[2]
	bpb.reservedSectors = binary.LittleEndian.Uint16(b[3:5])
	bpb.fatCount = b[5]
	bpb.rootDirectoryEntries = binary.LittleEndian.Uint16(b[6:8])
	bpb.totalSectors = binary.LittleEndian.Uint16(b[8:10])
	bpb.mediaType = b[10]
	bpb.sectorsPerFat = binary.LittleEndian.Uint16(b[11:13])
	return &bpb, nil
}

// ToBytes returns the bytes for a DOS 2.0 BIOS Parameter Block, ready to be written to disk
func (bpb *dos20BPB) toBytes() []byte {
	b := make([]byte, 13)
	binary.LittleEndian.PutUint16(b[0:2], uint16(bpb.bytesPerSector))
	b[2] = bpb.sectorsPerCluster
	binary.LittleEndian.PutUint16(b[3:5], bpb.reservedSectors)
	b[5] = bpb.fatCount
	binary.LittleEndian.PutUint16(b[6:8], bpb.rootDirectoryEntries)
	binary.LittleEndian.PutUint16(b[8:10], bpb.totalSectors)
	b[10] = bpb.mediaType
	binary.LittleEndian.PutUint16(b[11:13], bpb.sectorsPerFat)
	return b
}
