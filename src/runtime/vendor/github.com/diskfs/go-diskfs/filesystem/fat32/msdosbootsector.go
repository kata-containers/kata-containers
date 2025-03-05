package fat32

import (
	"bytes"
	"encoding/binary"
	"fmt"
)

// MsDosBootSectorSignature is the required last 2 bytes of the MS-DOS boot sector
const msDosBootSectorSignature uint16 = 0x55aa

// MsDosBootSector is the structure representing an msdos boot structure
type msDosBootSector struct {
	jumpInstruction    [3]byte    // JumpInstruction is the instruction set to jump to for booting
	oemName            string     // OEMName is the 8-byte OEM Name
	biosParameterBlock *dos71EBPB // BIOSParameterBlock is the FAT32 Extended BIOS Parameter Block
	bootCode           []byte     // BootCode represents the actual boot code
}

func (m *msDosBootSector) equal(a *msDosBootSector) bool {
	if (m == nil && a != nil) || (a == nil && m != nil) {
		return false
	}
	if m == nil && a == nil {
		return true
	}
	return m.biosParameterBlock.equal(a.biosParameterBlock) &&
		m.oemName == a.oemName &&
		m.jumpInstruction == a.jumpInstruction &&
		bytes.Equal(m.bootCode, a.bootCode)
}

// MsDosBootSectorFromBytes create an MsDosBootSector from a byte slice
func msDosBootSectorFromBytes(b []byte) (*msDosBootSector, error) {
	if len(b) != int(SectorSize512) {
		return nil, fmt.Errorf("cannot parse MS-DOS Boot Sector from %d bytes, must be exactly %d", len(b), SectorSize512)
	}
	bs := msDosBootSector{}
	// extract the jump instruction
	copy(bs.jumpInstruction[:], b[0:3])
	// extract the OEM name
	bs.oemName = string(b[3:11])
	// extract the EBPB and its size
	bpb, bpbSize, err := dos71EBPBFromBytes(b[11:90])
	if err != nil {
		return nil, fmt.Errorf("could not read FAT32 BIOS Parameter Block from boot sector: %v", err)
	}
	bs.biosParameterBlock = bpb

	// we have the size of the EBPB, we can figure out the size of the boot code
	bootSectorStart := 11 + bpbSize
	bootSectorEnd := SectorSize512 - 2
	bs.bootCode = b[bootSectorStart:bootSectorEnd]

	// validate boot sector signature
	if bsSignature := binary.BigEndian.Uint16(b[bootSectorEnd:]); bsSignature != msDosBootSectorSignature {
		return nil, fmt.Errorf("invalid signature in last 2 bytes of boot sector: %v", bsSignature)
	}

	return &bs, nil
}

// ToBytes output a byte slice representing the boot sector
func (m *msDosBootSector) toBytes() ([]byte, error) {
	// exactly one sector
	b := make([]byte, SectorSize512)

	// copy the 3-byte jump instruction
	copy(b[0:3], m.jumpInstruction[:])
	// make sure OEMName is <= 8 bytes
	name := m.oemName
	if len(name) > 8 {
		return nil, fmt.Errorf("cannot use OEM Name > 8 bytes long: %s", m.oemName)
	}
	nameR := []rune(name)
	if len(nameR) != len(name) {
		return nil, fmt.Errorf("invalid OEM Name: non-ascii characters")
	}

	oemName := fmt.Sprintf("%-8s", m.oemName)
	copy(b[3:11], oemName)

	// bytes for the EBPB
	bpbBytes, err := m.biosParameterBlock.toBytes()
	if err != nil {
		return nil, fmt.Errorf("error getting FAT32 EBPB: %v", err)
	}
	copy(b[11:], bpbBytes)
	bpbLen := len(bpbBytes)

	// bytes for the boot sector
	if len(m.bootCode) > int(SectorSize512)-2-(11+bpbLen) {
		return nil, fmt.Errorf("boot code too long at %d bytes", len(m.bootCode))
	}
	copy(b[11+bpbLen:SectorSize512-2], m.bootCode)

	// bytes for the signature
	binary.BigEndian.PutUint16(b[SectorSize512-2:], msDosBootSectorSignature)

	return b, nil
}
