package fat32

import (
	"encoding/binary"
	"fmt"
)

// FSInfoSectorSignature is the signature for every FAT32 FSInformationSector
type fsInfoSectorSignature uint32

const (
	// FSInfoSectorSignatureStart is the 4 bytes that signify the beginning of a FAT32 FS Information Sector
	fsInfoSectorSignatureStart fsInfoSectorSignature = 0x52526141
	// FSInfoSectorSignatureMid is the 4 bytes that signify the middle bytes 484-487 of a FAT32 FS Information Sector
	fsInfoSectorSignatureMid fsInfoSectorSignature = 0x72724161
	// FSInfoSectorSignatureEnd is the 4 bytes that signify the end of a FAT32 FS Information Sector
	fsInfoSectorSignatureEnd fsInfoSectorSignature = 0x000055AA
)

const (
	// unknownFreeDataClusterCount is the fixed flag for unknown number of free data clusters
	//nolint:varcheck,deadcode // keep for future reference
	unknownFreeDataClusterCount uint32 = 0xffffffff
	// unknownlastAllocatedCluster is the fixed flag for unknown most recently allocated cluster
	//nolint:varcheck,deadcode // keep for future reference
	unknownlastAllocatedCluster uint32 = 0xffffffff
)

// FSInformationSector is a structure holding the FAT32 filesystem information sector
type FSInformationSector struct {
	freeDataClustersCount uint32
	lastAllocatedCluster  uint32
}

// FSInformationSectorFromBytes create an FSInformationSector struct from bytes
func fsInformationSectorFromBytes(b []byte) (*FSInformationSector, error) {
	bLen := len(b)
	if bLen != int(SectorSize512) {
		return nil, fmt.Errorf("cannot read FAT32 FS Information Sector from %d bytes instead of expected %d", bLen, SectorSize512)
	}

	fsis := FSInformationSector{}

	// validate the signatures
	signatureStart := binary.BigEndian.Uint32(b[0:4])
	signatureMid := binary.BigEndian.Uint32(b[484:488])
	signatureEnd := binary.BigEndian.Uint32(b[508:512])

	if signatureStart != uint32(fsInfoSectorSignatureStart) {
		return nil, fmt.Errorf("invalid signature at beginning of FAT 32 Filesystem Information Sector: %x", signatureStart)
	}
	if signatureMid != uint32(fsInfoSectorSignatureMid) {
		return nil, fmt.Errorf("invalid signature at middle of FAT 32 Filesystem Information Sector: %x", signatureMid)
	}
	if signatureEnd != uint32(fsInfoSectorSignatureEnd) {
		return nil, fmt.Errorf("invalid signature at end of FAT 32 Filesystem Information Sector: %x", signatureEnd)
	}

	// validated, so just read the data
	fsis.freeDataClustersCount = binary.LittleEndian.Uint32(b[488:492])
	fsis.lastAllocatedCluster = binary.LittleEndian.Uint32(b[492:496])

	return &fsis, nil
}

// ToBytes returns a FAT32 Filesystem Information Sector ready to be written to disk
func (fsis *FSInformationSector) toBytes() []byte {
	b := make([]byte, SectorSize512)

	// signatures
	binary.BigEndian.PutUint32(b[0:4], uint32(fsInfoSectorSignatureStart))
	binary.BigEndian.PutUint32(b[484:488], uint32(fsInfoSectorSignatureMid))
	binary.BigEndian.PutUint32(b[508:512], uint32(fsInfoSectorSignatureEnd))

	// reserved 0x00
	// these are set to 0 by default, so not much to do

	// actual data
	binary.LittleEndian.PutUint32(b[488:492], fsis.freeDataClustersCount)
	binary.LittleEndian.PutUint32(b[492:496], fsis.lastAllocatedCluster)

	return b
}
