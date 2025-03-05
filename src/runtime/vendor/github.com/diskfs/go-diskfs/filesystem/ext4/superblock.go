package ext4

import (
	"encoding/binary"
	"fmt"
	"math"
	"reflect"
	"sort"
	"time"

	"github.com/diskfs/go-diskfs/filesystem/ext4/crc"
	"github.com/diskfs/go-diskfs/util"
	"github.com/google/uuid"
)

type filesystemState uint16
type errorBehaviour uint16
type osFlag uint32
type feature uint32
type hashAlgorithm byte
type flag uint32
type encryptionAlgorithm byte

func (f feature) included(a uint32) bool {
	return a&uint32(f) == uint32(f)
}

//nolint:unused // we know this is unused, but it will be needed in future
func (f flag) equal(a flag) bool {
	return f == a
}
func (f flag) included(a uint32) bool {
	return a&uint32(f) == uint32(f)
}

const (
	// superblockSignature is the signature for every superblock
	superblockSignature uint16 = 0xef53
	// optional states for the filesystem
	fsStateCleanlyUnmounted filesystemState = 0x0001
	fsStateErrors           filesystemState = 0x0002
	fsStateOrphansRecovered filesystemState = 0x0004
	// how to handle erorrs
	errorsContinue        errorBehaviour = 1
	errorsRemountReadOnly errorBehaviour = 2
	errorsPanic           errorBehaviour = 3
	// checksum type
	checkSumTypeCRC32c byte = 1
	// oses
	osLinux   osFlag = 0
	osHurd    osFlag = 1
	osMasix   osFlag = 2
	osFreeBSD osFlag = 3
	osLites   osFlag = 4
	// compatible, incompatible, and compatibleReadOnly feature flags
	compatFeatureDirectoryPreAllocate               feature = 0x1
	compatFeatureImagicInodes                       feature = 0x2
	compatFeatureHasJournal                         feature = 0x4
	compatFeatureExtendedAttributes                 feature = 0x8
	compatFeatureReservedGDTBlocksForExpansion      feature = 0x10
	compatFeatureDirectoryIndices                   feature = 0x20
	compatFeatureLazyBlockGroup                     feature = 0x40
	compatFeatureExcludeInode                       feature = 0x80
	compatFeatureExcludeBitmap                      feature = 0x100
	compatFeatureSparseSuperBlockV2                 feature = 0x200
	compatFeatureFastCommit                         feature = 0x400
	compatFeatureStableInodes                       feature = 0x800
	compatFeatureOrphanFile                         feature = 0x1000
	incompatFeatureCompression                      feature = 0x1
	incompatFeatureDirectoryEntriesRecordFileType   feature = 0x2
	incompatFeatureRecoveryNeeded                   feature = 0x4
	incompatFeatureSeparateJournalDevice            feature = 0x8
	incompatFeatureMetaBlockGroups                  feature = 0x10
	incompatFeatureExtents                          feature = 0x40
	incompatFeature64Bit                            feature = 0x80
	incompatFeatureMultipleMountProtection          feature = 0x100
	incompatFeatureFlexBlockGroups                  feature = 0x200
	incompatFeatureExtendedAttributeInodes          feature = 0x400
	incompatFeatureDataInDirectoryEntries           feature = 0x1000
	incompatFeatureMetadataChecksumSeedInSuperblock feature = 0x2000
	incompatFeatureLargeDirectory                   feature = 0x4000
	incompatFeatureDataInInode                      feature = 0x8000
	incompatFeatureEncryptInodes                    feature = 0x10000
	roCompatFeatureSparseSuperblock                 feature = 0x1
	roCompatFeatureLargeFile                        feature = 0x2
	roCompatFeatureBtreeDirectory                   feature = 0x4
	roCompatFeatureHugeFile                         feature = 0x8
	roCompatFeatureGDTChecksum                      feature = 0x10
	roCompatFeatureLargeSubdirectoryCount           feature = 0x20
	roCompatFeatureLargeInodes                      feature = 0x40
	roCompatFeatureSnapshot                         feature = 0x80
	roCompatFeatureQuota                            feature = 0x100
	roCompatFeatureBigalloc                         feature = 0x200
	roCompatFeatureMetadataChecksums                feature = 0x400
	roCompatFeatureReplicas                         feature = 0x800
	roCompatFeatureReadOnly                         feature = 0x1000
	roCompatFeatureProjectQuotas                    feature = 0x2000
	// hash algorithms for htree directory entries
	hashLegacy          hashAlgorithm = 0x0
	hashHalfMD4         hashAlgorithm = 0x1
	hashTea             hashAlgorithm = 0x2
	hashLegacyUnsigned  hashAlgorithm = 0x3
	hashHalfMD4Unsigned hashAlgorithm = 0x4
	hashTeaUnsigned     hashAlgorithm = 0x5
	// miscellaneous flags
	flagSignedDirectoryHash   flag = 0x0001
	flagUnsignedDirectoryHash flag = 0x0002
	flagTestDevCode           flag = 0x0004
	// encryption algorithms
	//nolint:unused // we know these are unused, but they will be needed in the future
	encryptionAlgorithmInvalid   encryptionAlgorithm = 0
	encryptionAlgorithm256AESXTS encryptionAlgorithm = 1
	encryptionAlgorithm256AESGCM encryptionAlgorithm = 2
	encryptionAlgorithm256AESCBC encryptionAlgorithm = 3
)

// journalBackup is a backup in the superblock of the journal's inode i_block[] array and size
type journalBackup struct {
	iBlocks [15]uint32
	iSize   uint64
}

// Superblock is a structure holding the ext4 superblock
type superblock struct {
	inodeCount                   uint32
	blockCount                   uint64
	reservedBlocks               uint64
	freeBlocks                   uint64
	freeInodes                   uint32
	firstDataBlock               uint32
	blockSize                    uint32
	clusterSize                  uint64
	blocksPerGroup               uint32
	clustersPerGroup             uint32
	inodesPerGroup               uint32
	mountTime                    time.Time
	writeTime                    time.Time
	mountCount                   uint16
	mountsToFsck                 uint16
	filesystemState              filesystemState
	errorBehaviour               errorBehaviour
	minorRevision                uint16
	lastCheck                    time.Time
	checkInterval                uint32
	creatorOS                    osFlag
	revisionLevel                uint32
	reservedBlocksDefaultUID     uint16
	reservedBlocksDefaultGID     uint16
	firstNonReservedInode        uint32
	inodeSize                    uint16
	blockGroup                   uint16
	features                     featureFlags
	uuid                         *uuid.UUID
	volumeLabel                  string
	lastMountedDirectory         string
	algorithmUsageBitmap         uint32
	preallocationBlocks          byte
	preallocationDirectoryBlocks byte
	reservedGDTBlocks            uint16
	journalSuperblockUUID        *uuid.UUID
	journalInode                 uint32
	journalDeviceNumber          uint32
	orphanedInodesStart          uint32
	hashTreeSeed                 []uint32
	hashVersion                  hashAlgorithm
	groupDescriptorSize          uint16
	defaultMountOptions          mountOptions
	firstMetablockGroup          uint32
	mkfsTime                     time.Time
	journalBackup                *journalBackup
	// 64-bit mode features
	inodeMinBytes                uint16
	inodeReserveBytes            uint16
	miscFlags                    miscFlags
	raidStride                   uint16
	multiMountPreventionInterval uint16
	multiMountProtectionBlock    uint64
	raidStripeWidth              uint32
	logGroupsPerFlex             uint64
	checksumType                 byte
	totalKBWritten               uint64
	snapshotInodeNumber          uint32
	snapshotID                   uint32
	snapshotReservedBlocks       uint64
	snapshotStartInode           uint32
	errorCount                   uint32
	errorFirstTime               time.Time
	errorFirstInode              uint32
	errorFirstBlock              uint64
	errorFirstFunction           string
	errorFirstLine               uint32
	errorLastTime                time.Time
	errorLastInode               uint32
	errorLastLine                uint32
	errorLastBlock               uint64
	errorLastFunction            string
	errorFirstCode               byte
	errorLastCode                byte
	mountOptions                 string
	userQuotaInode               uint32
	groupQuotaInode              uint32
	overheadBlocks               uint32
	backupSuperblockBlockGroups  [2]uint32
	encryptionAlgorithms         [4]encryptionAlgorithm
	encryptionSalt               [16]byte
	lostFoundInode               uint32
	projectQuotaInode            uint32
	checksumSeed                 uint32
	// encoding
	filenameCharsetEncoding      uint16
	filenameCharsetEncodingFlags uint16
	// inode for tracking orphaned inodes
	orphanedInodeInodeNumber uint32
}

func (sb *superblock) equal(o *superblock) bool {
	if (sb == nil && o != nil) || (o == nil && sb != nil) {
		return false
	}
	if sb == nil && o == nil {
		return true
	}
	return reflect.DeepEqual(sb, o)
}

// FSInformationSectorFromBytes create an FSInformationSector struct from bytes
func superblockFromBytes(b []byte) (*superblock, error) {
	bLen := len(b)
	if bLen != int(SuperblockSize) {
		return nil, fmt.Errorf("cannot read superblock from %d bytes instead of expected %d", bLen, SuperblockSize)
	}

	// check the magic signature
	actualSignature := binary.LittleEndian.Uint16(b[0x38:0x3a])
	if actualSignature != superblockSignature {
		return nil, fmt.Errorf("erroneous signature at location 0x38 was %x instead of expected %x", actualSignature, superblockSignature)
	}

	sb := superblock{}

	// first read feature flags of various types
	compatFlags := binary.LittleEndian.Uint32(b[0x5c:0x60])
	incompatFlags := binary.LittleEndian.Uint32(b[0x60:0x64])
	roCompatFlags := binary.LittleEndian.Uint32(b[0x64:0x68])
	// track which ones are set
	sb.features = parseFeatureFlags(compatFlags, incompatFlags, roCompatFlags)

	sb.inodeCount = binary.LittleEndian.Uint32(b[0:4])

	// block count, reserved block count and free blocks depends on whether the fs is 64-bit or not
	blockCount := make([]byte, 8)
	reservedBlocks := make([]byte, 8)
	freeBlocks := make([]byte, 8)

	copy(blockCount[0:4], b[0x4:0x8])
	copy(reservedBlocks[0:4], b[0x8:0xc])
	copy(freeBlocks[0:4], b[0xc:0x10])

	if sb.features.fs64Bit {
		copy(blockCount[4:8], b[0x150:0x154])
		copy(reservedBlocks[4:8], b[0x154:0x158])
		copy(freeBlocks[4:8], b[0x158:0x15c])
	}
	sb.blockCount = binary.LittleEndian.Uint64(blockCount)
	sb.reservedBlocks = binary.LittleEndian.Uint64(reservedBlocks)
	sb.freeBlocks = binary.LittleEndian.Uint64(freeBlocks)

	sb.freeInodes = binary.LittleEndian.Uint32(b[0x10:0x14])
	sb.firstDataBlock = binary.LittleEndian.Uint32(b[0x14:0x18])
	sb.blockSize = uint32(math.Exp2(float64(10 + binary.LittleEndian.Uint32(b[0x18:0x1c]))))
	sb.clusterSize = uint64(math.Exp2(float64(binary.LittleEndian.Uint32(b[0x1c:0x20]))))
	sb.blocksPerGroup = binary.LittleEndian.Uint32(b[0x20:0x24])
	if sb.features.bigalloc {
		sb.clustersPerGroup = binary.LittleEndian.Uint32(b[0x24:0x28])
	}
	sb.inodesPerGroup = binary.LittleEndian.Uint32(b[0x28:0x2c])
	// these higher bits are listed as reserved in https://ext4.wiki.kernel.org/index.php/Ext4_Disk_Layout
	// but looking at the source to mke2fs, we see that some are used, see
	// https://git.kernel.org/pub/scm/fs/ext2/e2fsprogs.git/tree/lib/ext2fs/ext2_fs.h#n653
	//
	// mount time has low 32 bits at 0x2c and high 8 bits at 0x274
	// write time has low 32 bits at 0x30 and high 8 bits at 0x275
	// mkfs time has low 32 bits at 0x108 and high 8 bits at 0x276
	// lastcheck time has low 32 bits at 0x40 and high 8 bits at 0x277
	// firsterror time has low 32 bits at 0x198 and high 8 bits at 0x278
	// lasterror time has low 32 bits at 0x1cc and high 8 bits at 0x279
	// firsterror code is 8 bits at 0x27a
	// lasterror code is 8 bits at 0x27b
	sb.mountTime = bytesToTime(b[0x2c:0x30], b[0x274:0x275])
	sb.writeTime = bytesToTime(b[0x30:0x34], b[0x275:0x276])
	sb.mkfsTime = bytesToTime(b[0x108:0x10c], b[0x276:0x277])
	sb.lastCheck = bytesToTime(b[0x40:0x44], b[0x277:0x278])
	sb.errorFirstTime = bytesToTime(b[0x198:0x19c], b[0x278:0x279])
	sb.errorLastTime = bytesToTime(b[0x1cc:0x1d0], b[0x279:0x280])

	sb.errorFirstCode = b[0x27a]
	sb.errorLastCode = b[0x27b]

	sb.mountCount = binary.LittleEndian.Uint16(b[0x34:0x36])
	sb.mountsToFsck = binary.LittleEndian.Uint16(b[0x36:0x38])

	sb.filesystemState = filesystemState(binary.LittleEndian.Uint16(b[0x3a:0x3c]))
	sb.errorBehaviour = errorBehaviour(binary.LittleEndian.Uint16(b[0x3c:0x3e]))

	sb.minorRevision = binary.LittleEndian.Uint16(b[0x3e:0x40])
	sb.checkInterval = binary.LittleEndian.Uint32(b[0x44:0x48])

	sb.creatorOS = osFlag(binary.LittleEndian.Uint32(b[0x48:0x4c]))
	sb.revisionLevel = binary.LittleEndian.Uint32(b[0x4c:0x50])
	sb.reservedBlocksDefaultUID = binary.LittleEndian.Uint16(b[0x50:0x52])
	sb.reservedBlocksDefaultGID = binary.LittleEndian.Uint16(b[0x52:0x54])

	sb.firstNonReservedInode = binary.LittleEndian.Uint32(b[0x54:0x58])
	sb.inodeSize = binary.LittleEndian.Uint16(b[0x58:0x5a])
	sb.blockGroup = binary.LittleEndian.Uint16(b[0x5a:0x5c])

	voluuid, err := uuid.FromBytes(b[0x68:0x78])
	if err != nil {
		return nil, fmt.Errorf("unable to read volume UUID: %v", err)
	}
	sb.uuid = &voluuid
	sb.volumeLabel = minString(b[0x78:0x88])
	sb.lastMountedDirectory = minString(b[0x88:0xc8])
	sb.algorithmUsageBitmap = binary.LittleEndian.Uint32(b[0xc8:0xcc])

	sb.preallocationBlocks = b[0xcc]
	sb.preallocationDirectoryBlocks = b[0xcd]
	sb.reservedGDTBlocks = binary.LittleEndian.Uint16(b[0xce:0xd0])

	journaluuid, err := uuid.FromBytes(b[0xd0:0xe0])
	if err != nil {
		return nil, fmt.Errorf("unable to read journal UUID: %v", err)
	}
	sb.journalSuperblockUUID = &journaluuid
	sb.journalInode = binary.LittleEndian.Uint32(b[0xe0:0xe4])
	sb.journalDeviceNumber = binary.LittleEndian.Uint32(b[0xe4:0xe8])
	sb.orphanedInodesStart = binary.LittleEndian.Uint32(b[0xe8:0xec])

	htreeSeed := make([]uint32, 0, 4)
	htreeSeed = append(htreeSeed,
		binary.LittleEndian.Uint32(b[0xec:0xf0]),
		binary.LittleEndian.Uint32(b[0xf0:0xf4]),
		binary.LittleEndian.Uint32(b[0xf4:0xf8]),
		binary.LittleEndian.Uint32(b[0xf8:0xfc]),
	)
	sb.hashTreeSeed = htreeSeed

	sb.hashVersion = hashAlgorithm(b[0xfc])

	sb.groupDescriptorSize = binary.LittleEndian.Uint16(b[0xfe:0x100])

	sb.defaultMountOptions = parseMountOptions(binary.LittleEndian.Uint32(b[0x100:0x104]))
	sb.firstMetablockGroup = binary.LittleEndian.Uint32(b[0x104:0x108])

	journalBackupType := b[0xfd]
	if journalBackupType == 0 || journalBackupType == 1 {
		journalBackupArray := [15]uint32{}
		startJournalBackup := 0x10c
		for i := 0; i < 15; i++ {
			start := startJournalBackup + 4*i
			end := startJournalBackup + 4*i + 4
			journalBackupArray[i] = binary.LittleEndian.Uint32(b[start:end])
		}
		iSizeBytes := make([]byte, 8)

		copy(iSizeBytes[0:4], b[startJournalBackup+4*16:startJournalBackup+4*17])
		copy(iSizeBytes[4:8], b[startJournalBackup+4*15:startJournalBackup+4*16])

		sb.journalBackup = &journalBackup{
			iSize:   binary.LittleEndian.Uint64(iSizeBytes),
			iBlocks: journalBackupArray,
		}
	}

	sb.inodeMinBytes = binary.LittleEndian.Uint16(b[0x15c:0x15e])
	sb.inodeReserveBytes = binary.LittleEndian.Uint16(b[0x15e:0x160])
	sb.miscFlags = parseMiscFlags(binary.LittleEndian.Uint32(b[0x160:0x164]))

	sb.raidStride = binary.LittleEndian.Uint16(b[0x164:0x166])
	sb.raidStripeWidth = binary.LittleEndian.Uint32(b[0x170:0x174])

	sb.multiMountPreventionInterval = binary.LittleEndian.Uint16(b[0x166:0x168])
	sb.multiMountProtectionBlock = binary.LittleEndian.Uint64(b[0x168:0x170])

	sb.logGroupsPerFlex = uint64(math.Exp2(float64(b[0x174])))

	sb.checksumType = b[0x175] // only valid one is 1
	if sb.checksumType != checkSumTypeCRC32c {
		return nil, fmt.Errorf("cannot read superblock: invalid checksum type %d, only valid is %d", sb.checksumType, checkSumTypeCRC32c)
	}

	// b[0x176:0x178] are reserved padding

	sb.totalKBWritten = binary.LittleEndian.Uint64(b[0x178:0x180])

	sb.snapshotInodeNumber = binary.LittleEndian.Uint32(b[0x180:0x184])
	sb.snapshotID = binary.LittleEndian.Uint32(b[0x184:0x188])
	sb.snapshotReservedBlocks = binary.LittleEndian.Uint64(b[0x188:0x190])
	sb.snapshotStartInode = binary.LittleEndian.Uint32(b[0x190:0x194])

	// errors
	sb.errorCount = binary.LittleEndian.Uint32(b[0x194:0x198])
	sb.errorFirstInode = binary.LittleEndian.Uint32(b[0x19c:0x1a0])
	sb.errorFirstBlock = binary.LittleEndian.Uint64(b[0x1a0:0x1a8])
	sb.errorFirstFunction = minString(b[0x1a8:0x1c8])
	sb.errorFirstLine = binary.LittleEndian.Uint32(b[0x1c8:0x1cc])
	sb.errorLastInode = binary.LittleEndian.Uint32(b[0x1d0:0x1d4])
	sb.errorLastLine = binary.LittleEndian.Uint32(b[0x1d4:0x1d8])
	sb.errorLastBlock = binary.LittleEndian.Uint64(b[0x1d8:0x1e0])
	sb.errorLastFunction = minString(b[0x1e0:0x200])

	sb.mountOptions = minString(b[0x200:0x240])
	sb.userQuotaInode = binary.LittleEndian.Uint32(b[0x240:0x244])
	sb.groupQuotaInode = binary.LittleEndian.Uint32(b[0x244:0x248])
	// overheadBlocks *always* is 0
	sb.overheadBlocks = binary.LittleEndian.Uint32(b[0x248:0x24c])
	sb.backupSuperblockBlockGroups = [2]uint32{
		binary.LittleEndian.Uint32(b[0x24c:0x250]),
		binary.LittleEndian.Uint32(b[0x250:0x254]),
	}
	for i := 0; i < 4; i++ {
		sb.encryptionAlgorithms[i] = encryptionAlgorithm(b[0x254+i])
	}
	for i := 0; i < 16; i++ {
		sb.encryptionSalt[i] = b[0x258+i]
	}
	sb.lostFoundInode = binary.LittleEndian.Uint32(b[0x268:0x26c])
	sb.projectQuotaInode = binary.LittleEndian.Uint32(b[0x26c:0x270])

	sb.checksumSeed = binary.LittleEndian.Uint32(b[0x270:0x274])
	// what if the seed is missing? It can be.
	if sb.features.metadataChecksums && sb.checksumSeed == 0 {
		sb.checksumSeed = crc.CRC32c(0xffffffff, sb.uuid[:])
	}

	sb.filenameCharsetEncoding = binary.LittleEndian.Uint16(b[0x27c:0x27e])
	sb.filenameCharsetEncodingFlags = binary.LittleEndian.Uint16(b[0x27e:0x280])
	sb.orphanedInodeInodeNumber = binary.LittleEndian.Uint32(b[0x280:0x284])

	// b[0x288:0x3fc] are reserved for zero padding

	// checksum
	checksum := binary.LittleEndian.Uint32(b[0x3fc:0x400])

	// calculate the checksum and validate - we use crc32c
	if sb.features.metadataChecksums {
		actualChecksum := crc.CRC32c(0xffffffff, b[0:0x3fc])
		if actualChecksum != checksum {
			return nil, fmt.Errorf("invalid superblock checksum, actual was %x, on disk was %x, inverted on disk was %x", actualChecksum, checksum, 0xffffffff-checksum)
		}
	}

	return &sb, nil
}

// toBytes returns a superblock ready to be written to disk
func (sb *superblock) toBytes() ([]byte, error) {
	b := make([]byte, SuperblockSize)

	binary.LittleEndian.PutUint16(b[0x38:0x3a], superblockSignature)
	compatFlags, incompatFlags, roCompatFlags := sb.features.toInts()
	binary.LittleEndian.PutUint32(b[0x5c:0x60], compatFlags)
	binary.LittleEndian.PutUint32(b[0x60:0x64], incompatFlags)
	binary.LittleEndian.PutUint32(b[0x64:0x68], roCompatFlags)

	binary.LittleEndian.PutUint32(b[0:4], sb.inodeCount)

	// block count, reserved block count and free blocks depends on whether the fs is 64-bit or not
	blockCount := make([]byte, 8)
	reservedBlocks := make([]byte, 8)
	freeBlocks := make([]byte, 8)

	binary.LittleEndian.PutUint64(blockCount, sb.blockCount)
	binary.LittleEndian.PutUint64(reservedBlocks, sb.reservedBlocks)
	binary.LittleEndian.PutUint64(freeBlocks, sb.freeBlocks)

	copy(b[0x4:0x8], blockCount[0:4])
	copy(b[0x8:0xc], reservedBlocks[0:4])
	copy(b[0xc:0x10], freeBlocks[0:4])

	if sb.features.fs64Bit {
		copy(b[0x150:0x154], blockCount[4:8])
		copy(b[0x154:0x158], reservedBlocks[4:8])
		copy(b[0x158:0x15c], freeBlocks[4:8])
	}

	binary.LittleEndian.PutUint32(b[0x10:0x14], sb.freeInodes)
	binary.LittleEndian.PutUint32(b[0x14:0x18], sb.firstDataBlock)
	binary.LittleEndian.PutUint32(b[0x18:0x1c], uint32(math.Log2(float64(sb.blockSize))-10))
	binary.LittleEndian.PutUint32(b[0x1c:0x20], uint32(math.Log2(float64(sb.clusterSize))))

	binary.LittleEndian.PutUint32(b[0x20:0x24], sb.blocksPerGroup)
	if sb.features.bigalloc {
		binary.LittleEndian.PutUint32(b[0x24:0x28], sb.clustersPerGroup)
	} else {
		binary.LittleEndian.PutUint32(b[0x24:0x28], sb.blocksPerGroup)
	}
	binary.LittleEndian.PutUint32(b[0x28:0x2c], sb.inodesPerGroup)
	mountTime := timeToBytes(sb.mountTime)
	writeTime := timeToBytes(sb.writeTime)
	mkfsTime := timeToBytes(sb.mkfsTime)
	lastCheck := timeToBytes(sb.lastCheck)
	errorFirstTime := timeToBytes(sb.errorFirstTime)
	errorLastTime := timeToBytes(sb.errorLastTime)

	// mount time low bits, high bit
	copy(b[0x2c:0x30], mountTime[0:4])
	b[0x274] = mountTime[4]
	// write time low bits, high bit
	copy(b[0x30:0x34], writeTime[0:4])
	b[0x275] = writeTime[4]
	// mkfs time low bits, high bit
	copy(b[0x108:0x10c], mkfsTime[0:4])
	b[0x276] = mkfsTime[4]
	// last check time low bits, high bit
	copy(b[0x40:0x44], lastCheck[0:4])
	b[0x277] = lastCheck[4]
	// first error time low bits, high bit
	copy(b[0x198:0x19c], errorFirstTime[0:4])
	b[0x278] = errorFirstTime[4]
	// last error time low bits, high bit
	copy(b[0x1cc:0x1d0], errorLastTime[0:4])
	b[0x279] = errorLastTime[4]

	// error codes
	b[0x27a] = sb.errorFirstCode
	b[0x27b] = sb.errorLastCode

	binary.LittleEndian.PutUint16(b[0x34:0x36], sb.mountCount)
	binary.LittleEndian.PutUint16(b[0x36:0x38], sb.mountsToFsck)

	binary.LittleEndian.PutUint16(b[0x3a:0x3c], uint16(sb.filesystemState))
	binary.LittleEndian.PutUint16(b[0x3c:0x3e], uint16(sb.errorBehaviour))

	binary.LittleEndian.PutUint16(b[0x3e:0x40], sb.minorRevision)
	binary.LittleEndian.PutUint32(b[0x40:0x44], uint32(sb.lastCheck.Unix()))
	binary.LittleEndian.PutUint32(b[0x44:0x48], sb.checkInterval)

	binary.LittleEndian.PutUint32(b[0x48:0x4c], uint32(sb.creatorOS))
	binary.LittleEndian.PutUint32(b[0x4c:0x50], sb.revisionLevel)
	binary.LittleEndian.PutUint16(b[0x50:0x52], sb.reservedBlocksDefaultUID)
	binary.LittleEndian.PutUint16(b[0x52:0x54], sb.reservedBlocksDefaultGID)

	binary.LittleEndian.PutUint32(b[0x54:0x58], sb.firstNonReservedInode)
	binary.LittleEndian.PutUint16(b[0x58:0x5a], sb.inodeSize)
	binary.LittleEndian.PutUint16(b[0x5a:0x5c], sb.blockGroup)

	if sb.uuid != nil {
		copy(b[0x68:0x78], sb.uuid[:])
	}

	ab, err := stringToASCIIBytes(sb.volumeLabel, 16)
	if err != nil {
		return nil, fmt.Errorf("error converting volume label to bytes: %v", err)
	}
	copy(b[0x78:0x88], ab[0:16])
	ab, err = stringToASCIIBytes(sb.lastMountedDirectory, 64)
	if err != nil {
		return nil, fmt.Errorf("error last mounted directory to bytes: %v", err)
	}
	copy(b[0x88:0xc8], ab[0:64])

	binary.LittleEndian.PutUint32(b[0xc8:0xcc], sb.algorithmUsageBitmap)

	b[0xcc] = sb.preallocationBlocks
	b[0xcd] = sb.preallocationDirectoryBlocks
	binary.LittleEndian.PutUint16(b[0xce:0xd0], sb.reservedGDTBlocks)

	if sb.journalSuperblockUUID != nil {
		copy(b[0xd0:0xe0], sb.journalSuperblockUUID[:])
	}

	binary.LittleEndian.PutUint32(b[0xe0:0xe4], sb.journalInode)
	binary.LittleEndian.PutUint32(b[0xe4:0xe8], sb.journalDeviceNumber)
	binary.LittleEndian.PutUint32(b[0xe8:0xec], sb.orphanedInodesStart)

	// to be safe
	if len(sb.hashTreeSeed) < 4 {
		sb.hashTreeSeed = append(sb.hashTreeSeed, 0, 0, 0, 0)
	}
	binary.LittleEndian.PutUint32(b[0xec:0xf0], sb.hashTreeSeed[0])
	binary.LittleEndian.PutUint32(b[0xf0:0xf4], sb.hashTreeSeed[1])
	binary.LittleEndian.PutUint32(b[0xf4:0xf8], sb.hashTreeSeed[2])
	binary.LittleEndian.PutUint32(b[0xf8:0xfc], sb.hashTreeSeed[3])

	b[0xfc] = byte(sb.hashVersion)

	binary.LittleEndian.PutUint16(b[0xfe:0x100], sb.groupDescriptorSize)

	binary.LittleEndian.PutUint32(b[0x100:0x104], sb.defaultMountOptions.toInt())
	binary.LittleEndian.PutUint32(b[0x104:0x108], sb.firstMetablockGroup)

	if sb.journalBackup != nil {
		b[0xfd] = 1
		startJournalBackup := 0x10c
		for i := 0; i < 15; i++ {
			start := startJournalBackup + 4*i
			end := startJournalBackup + 4*i + 4
			binary.LittleEndian.PutUint32(b[start:end], sb.journalBackup.iBlocks[i])
		}

		iSizeBytes := make([]byte, 8)
		binary.LittleEndian.PutUint64(iSizeBytes, sb.journalBackup.iSize)
		copy(b[startJournalBackup+4*16:startJournalBackup+4*17], iSizeBytes[0:4])
		copy(b[startJournalBackup+4*15:startJournalBackup+4*16], iSizeBytes[4:8])
	}

	binary.LittleEndian.PutUint16(b[0x15c:0x15e], sb.inodeMinBytes)
	binary.LittleEndian.PutUint16(b[0x15e:0x160], sb.inodeReserveBytes)
	binary.LittleEndian.PutUint32(b[0x160:0x164], sb.miscFlags.toInt())

	binary.LittleEndian.PutUint16(b[0x164:0x166], sb.raidStride)
	binary.LittleEndian.PutUint32(b[0x170:0x174], sb.raidStripeWidth)

	binary.LittleEndian.PutUint16(b[0x166:0x168], sb.multiMountPreventionInterval)
	binary.LittleEndian.PutUint64(b[0x168:0x170], sb.multiMountProtectionBlock)

	b[0x174] = uint8(math.Log2(float64(sb.logGroupsPerFlex)))

	b[0x175] = sb.checksumType // only valid one is 1

	// b[0x176:0x178] are reserved padding

	binary.LittleEndian.PutUint64(b[0x178:0x180], sb.totalKBWritten)

	binary.LittleEndian.PutUint32(b[0x180:0x184], sb.snapshotInodeNumber)
	binary.LittleEndian.PutUint32(b[0x184:0x188], sb.snapshotID)
	binary.LittleEndian.PutUint64(b[0x188:0x190], sb.snapshotReservedBlocks)
	binary.LittleEndian.PutUint32(b[0x190:0x194], sb.snapshotStartInode)

	// errors
	binary.LittleEndian.PutUint32(b[0x194:0x198], sb.errorCount)
	binary.LittleEndian.PutUint32(b[0x19c:0x1a0], sb.errorFirstInode)
	binary.LittleEndian.PutUint64(b[0x1a0:0x1a8], sb.errorFirstBlock)
	errorFirstFunctionBytes, err := stringToASCIIBytes(sb.errorFirstFunction, 32)
	if err != nil {
		return nil, fmt.Errorf("error converting errorFirstFunction to bytes: %v", err)
	}
	copy(b[0x1a8:0x1c8], errorFirstFunctionBytes)
	binary.LittleEndian.PutUint32(b[0x1c8:0x1cc], sb.errorFirstLine)
	binary.LittleEndian.PutUint32(b[0x1d0:0x1d4], sb.errorLastInode)
	binary.LittleEndian.PutUint32(b[0x1d4:0x1d8], sb.errorLastLine)
	binary.LittleEndian.PutUint64(b[0x1d8:0x1e0], sb.errorLastBlock)
	errorLastFunctionBytes, err := stringToASCIIBytes(sb.errorLastFunction, 32)
	if err != nil {
		return nil, fmt.Errorf("error converting errorLastFunction to bytes: %v", err)
	}
	copy(b[0x1e0:0x200], errorLastFunctionBytes)

	mountOptionsBytes, err := stringToASCIIBytes(sb.mountOptions, 64)
	if err != nil {
		return nil, fmt.Errorf("error converting mountOptions to bytes: %v", err)
	}
	copy(b[0x200:0x240], mountOptionsBytes)
	binary.LittleEndian.PutUint32(b[0x240:0x244], sb.userQuotaInode)
	binary.LittleEndian.PutUint32(b[0x244:0x248], sb.groupQuotaInode)
	// overheadBlocks *always* is 0
	binary.LittleEndian.PutUint32(b[0x248:0x24c], sb.overheadBlocks)
	binary.LittleEndian.PutUint32(b[0x24c:0x250], sb.backupSuperblockBlockGroups[0])
	binary.LittleEndian.PutUint32(b[0x250:0x254], sb.backupSuperblockBlockGroups[1])
	// safety check of encryption algorithms

	for i := 0; i < 4; i++ {
		b[0x254+i] = byte(sb.encryptionAlgorithms[i])
	}
	for i := 0; i < 16; i++ {
		b[0x258+i] = sb.encryptionSalt[i]
	}
	binary.LittleEndian.PutUint32(b[0x268:0x26c], sb.lostFoundInode)
	binary.LittleEndian.PutUint32(b[0x26c:0x270], sb.projectQuotaInode)

	binary.LittleEndian.PutUint32(b[0x270:0x274], sb.checksumSeed)

	binary.LittleEndian.PutUint16(b[0x27c:0x27e], sb.filenameCharsetEncoding)
	binary.LittleEndian.PutUint16(b[0x27e:0x280], sb.filenameCharsetEncodingFlags)
	binary.LittleEndian.PutUint32(b[0x280:0x284], sb.orphanedInodeInodeNumber)

	// b[0x288:0x3fc] are reserved for zero padding

	// calculate the checksum and validate - we use crc32c
	if sb.features.metadataChecksums {
		actualChecksum := crc.CRC32c(0xffffffff, b[0:0x3fc])
		binary.LittleEndian.PutUint32(b[0x3fc:0x400], actualChecksum)
	}

	return b, nil
}

func (sb *superblock) gdtChecksumType() gdtChecksumType {
	var gdtChecksumTypeInFS gdtChecksumType
	switch {
	case sb.features.metadataChecksums:
		gdtChecksumTypeInFS = gdtChecksumMetadata
	case sb.features.gdtChecksum:
		gdtChecksumTypeInFS = gdtChecksumGdt
	default:
		gdtChecksumTypeInFS = gdtChecksumNone
	}
	return gdtChecksumTypeInFS
}

func (sb *superblock) blockGroupCount() uint64 {
	whole := sb.blockCount / uint64(sb.blocksPerGroup)
	part := sb.blockCount % uint64(sb.blocksPerGroup)
	if part > 0 {
		whole++
	}
	return whole
}

// calculateBackupSuperblocks calculate which block groups should have backup superblocks.
func calculateBackupSuperblockGroups(bgs int64) []int64 {
	// calculate which block groups should have backup superblocks
	// these are if the block group number is a power of 3, 5, or 7
	var backupGroups []int64
	for i := float64(0); ; i++ {
		bg := int64(math.Pow(3, i))
		if bg >= bgs {
			break
		}
		backupGroups = append(backupGroups, bg)
	}
	for i := float64(0); ; i++ {
		bg := int64(math.Pow(5, i))
		if bg >= bgs {
			break
		}
		backupGroups = append(backupGroups, bg)
	}
	for i := float64(0); ; i++ {
		bg := int64(math.Pow(7, i))
		if bg >= bgs {
			break
		}
		backupGroups = append(backupGroups, bg)
	}
	// sort the backup groups
	uniqBackupGroups := util.Uniqify[int64](backupGroups)
	sort.Slice(uniqBackupGroups, func(i, j int) bool {
		return uniqBackupGroups[i] < uniqBackupGroups[j]
	})
	return uniqBackupGroups
}

func bytesToTime(b ...[]byte) time.Time {
	// ensure it is at least 8 bytes
	var (
		in    [8]byte
		count int
	)
	for _, v := range b {
		toCopy := len(v)
		if toCopy+count > len(in) {
			toCopy = len(in) - count
		}
		copied := copy(in[count:], v[:toCopy])
		count += copied
	}
	return time.Unix(int64(binary.LittleEndian.Uint64(in[:])), 0).UTC()
}

// timeToBytes convert a time.Time to an 8 byte slice. Guarantees 8 bytes
func timeToBytes(t time.Time) []byte {
	timestamp := t.Unix()
	var b = make([]byte, 8)
	binary.LittleEndian.PutUint64(b, uint64(timestamp))
	return b
}
