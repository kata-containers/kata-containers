package iso9660

import (
	"encoding/binary"
	"fmt"
	"io"
	"os"

	"github.com/diskfs/go-diskfs/partition/mbr"
	"github.com/diskfs/go-diskfs/util"
)

//nolint:deadcode,varcheck,unused // we need these references in the future
const (
	elToritoSector        = 0x11
	elToritoDefaultBlocks = 4
)

// Platform target booting system for a bootable iso
type Platform uint8

const (
	// BIOS classic PC-BIOS x86
	BIOS Platform = 0x0
	// PPC PowerPC
	PPC Platform = 0x1
	// Mac some Macintosh system,s
	Mac Platform = 0x2
	// EFI newer extensible firmware interface
	EFI Platform = 0xef
	// default name for a boot catalog
	elToritoDefaultCatalog   = "BOOT.CAT"
	elToritoDefaultCatalogRR = "boot.catalog"
)

// Emulation that should be used for booting, normally none
type Emulation uint8

const (
	// NoEmulation do not do any emulation, the normal mode
	NoEmulation Emulation = 0
	// Floppy12Emulation emulate a 1.2 M floppy
	Floppy12Emulation Emulation = 1
	// Floppy144Emulation emulate a 1.44 M floppy
	Floppy144Emulation Emulation = 2
	// Floppy288Emulation emulate a 2.88 M floppy
	Floppy288Emulation Emulation = 3
	// HardDiskEmulation emulate a hard disk
	HardDiskEmulation Emulation = 4
)

// ElTorito boot structure for a disk
type ElTorito struct {
	// BootCatalog path to save the boot catalog in the file structure. Defaults to "/BOOT.CAT" in iso9660 and "/boot.catalog" in Rock Ridge
	BootCatalog string
	// HideBootCatalog if the boot catalog should be hidden in the file system. Defaults to false
	HideBootCatalog bool
	// Entries list of ElToritoEntry boot entries
	Entries []*ElToritoEntry
	// Platform supported platform
	Platform Platform
}

// ElToritoEntry single entry in an el torito boot catalog
type ElToritoEntry struct {
	Platform     Platform
	Emulation    Emulation
	BootFile     string
	HideBootFile bool
	LoadSegment  uint16
	// BootTable whether to insert a boot table into the entry, equivalent to genisoimage
	// option `-boot-info-table`. Unlike genisoimage, does not modify the file in the
	// filesystem, but inserts it on the fly.
	BootTable bool
	// SystemType type of system the partition is, according to the MBR standard
	SystemType mbr.Type
	// LoadSize how many blocks of BootFile to load, equivalent to genisoimage option `-boot-load-size`
	LoadSize uint16
	size     uint32
	location uint32
}

// generateCatalog generate the el torito boot catalog file
func (et *ElTorito) generateCatalog() []byte {
	b := make([]byte, 0)
	b = append(b, et.validationEntry()...)
	for i, e := range et.Entries {
		// only subsequent entries have a header, not the first
		if i != 0 {
			b = append(b, e.headerBytes(i == len(et.Entries)-1, 1)...)
		}
		b = append(b, e.entryBytes()...)
	}
	return b
}

func (et *ElTorito) validationEntry() []byte {
	b := make([]byte, 0x20)
	b[0] = 1
	b[1] = byte(et.Platform)
	copy(b[4:0x1c], util.AppNameVersion)
	b[0x1e] = 0x55
	b[0x1f] = 0xaa
	// calculate checksum
	checksum := uint16(0x0)
	for i := 0; i < len(b); i += 2 {
		checksum += binary.LittleEndian.Uint16(b[i : i+2])
	}
	binary.LittleEndian.PutUint16(b[0x1c:0x1e], -checksum)
	return b
}

// toHeaderBytes provide header bytes
func (e *ElToritoEntry) headerBytes(last bool, entries uint16) []byte {
	b := make([]byte, 0x20)
	b[0] = 0x90
	if last {
		b[0] = 0x91
	}
	b[1] = byte(e.Platform)
	binary.LittleEndian.PutUint16(b[2:4], entries)
	// we do not use the section identifier for now
	return b
}

// toBytes convert ElToritoEntry to appropriate entry bytes
func (e *ElToritoEntry) entryBytes() []byte {
	blocks := e.LoadSize
	if blocks == 0 {
		blocks = uint16(e.size / 512)
		if e.size%512 > 1 {
			blocks++
		}
	}
	b := make([]byte, 0x20)
	b[0] = 0x88
	b[1] = byte(e.Emulation)
	binary.LittleEndian.PutUint16(b[2:4], e.LoadSegment)
	// b[4] is system type, taken from byte 5 in the partition table in the boot image
	b[4] = byte(e.SystemType)
	// b[5] is unused and must be 0
	// b[6:8] is the number of emulated (512-byte) sectors, i.e. the size of the file
	binary.LittleEndian.PutUint16(b[6:8], blocks)
	// b[8:0xc] is the location of the boot image on disk, in disk (2048) sectors
	binary.LittleEndian.PutUint32(b[8:12], e.location)
	// b[0xc] is selection criteria type. We do not yet support it, so leave as 0.
	// b[0xd:] is vendor unique selectiomn criteria. We do not yet support it, so leave as 0.
	return b
}

// generateBootTable generate the el torito boot table for this entry
func (e *ElToritoEntry) generateBootTable(pvdSector uint32, p string) ([]byte, error) {
	b := make([]byte, 56)
	binary.LittleEndian.PutUint32(b[0:4], pvdSector)
	binary.LittleEndian.PutUint32(b[4:8], e.location)
	binary.LittleEndian.PutUint32(b[8:12], e.size)
	// Checksum - simply add up all 32-bit words beginning at byte position 64
	f, err := os.Open(p)
	if err != nil {
		return nil, fmt.Errorf("failed to open boot file for checksum reading %s: %v", p, err)
	}
	defer f.Close()

	var (
		checksum uint32
	)

	for offset := 64; offset <= int(e.size); offset += 4 {
		buf := make([]byte, 4)
		n, err := f.ReadAt(buf, int64(offset))
		if err != nil && err != io.EOF {
			return nil, err
		}
		if n == 0 {
			break
		}
		checksum += binary.LittleEndian.Uint32(buf)
		if err == io.EOF {
			break
		}
	}

	binary.LittleEndian.PutUint32(b[12:16], checksum)
	return b, nil
}
