package ext4

import (
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	iofs "io/fs"
	"math"
	"os"
	"path"
	"sort"
	"strings"
	"time"

	"github.com/diskfs/go-diskfs/backend"
	"github.com/diskfs/go-diskfs/filesystem"
	"github.com/diskfs/go-diskfs/filesystem/ext4/crc"
	"github.com/diskfs/go-diskfs/util"
	"github.com/google/uuid"
)

// SectorSize indicates what the sector size in bytes is
type SectorSize uint16

// BlockSize indicates how many sectors are in a block
type BlockSize uint8

// BlockGroupSize indicates how many blocks are in a group, standardly 8*block_size_in_bytes

const (
	// SectorSize512 is a sector size of 512 bytes, used as the logical size for all ext4 filesystems
	SectorSize512                SectorSize = 512
	minBlocksPerGroup            uint32     = 256
	BootSectorSize               SectorSize = 2 * SectorSize512
	SuperblockSize               SectorSize = 2 * SectorSize512
	BlockGroupFactor             int        = 8
	DefaultInodeRatio            int64      = 8192
	DefaultInodeSize             int64      = 256
	DefaultReservedBlocksPercent uint8      = 5
	DefaultVolumeName                       = "diskfs_ext4"
	minClusterSize               int        = 128
	maxClusterSize               int        = 65529
	bytesPerSlot                 int        = 32
	maxCharsLongFilename         int        = 13
	maxBlocksPerExtent           uint16     = 32768
	million                      int        = 1000000
	billion                      int        = 1000 * million
	firstNonReservedInode        uint32     = 11 // traditional

	minBlockLogSize int = 10 /* 1024 */
	maxBlockLogSize int = 16 /* 65536 */
	minBlockSize    int = (1 << minBlockLogSize)
	maxBlockSize    int = (1 << maxBlockLogSize)

	max32Num uint64 = math.MaxUint32
	max64Num uint64 = math.MaxUint64

	maxFilesystemSize32Bit uint64 = 16 << 40
	maxFilesystemSize64Bit uint64 = 1 << 60

	checksumType uint8 = 1

	// default for log groups per flex group
	defaultLogGroupsPerFlex int = 3

	// fixed inodes
	rootInode       uint32 = 2
	userQuotaInode  uint32 = 3
	groupQuotaInode uint32 = 4
	journalInode    uint32 = 8
	lostFoundInode         = 11 // traditional
)

type Params struct {
	UUID                  *uuid.UUID
	SectorsPerBlock       uint8
	BlocksPerGroup        uint32
	InodeRatio            int64
	InodeCount            uint32
	SparseSuperVersion    uint8
	Checksum              bool
	ClusterSize           int64
	ReservedBlocksPercent uint8
	VolumeName            string
	// JournalDevice external journal device, only checked if WithFeatureSeparateJournalDevice(true) is set
	JournalDevice      string
	LogFlexBlockGroups int
	Features           []FeatureOpt
	DefaultMountOpts   []MountOpt
}

// FileSystem implememnts the FileSystem interface
type FileSystem struct {
	bootSector       []byte
	superblock       *superblock
	groupDescriptors *groupDescriptors
	blockGroups      int64
	size             int64
	start            int64
	backend          backend.Storage
}

// Equal compare if two filesystems are equal
func (fs *FileSystem) Equal(a *FileSystem) bool {
	localMatch := fs.backend == a.backend
	sbMatch := fs.superblock.equal(a.superblock)
	gdMatch := fs.groupDescriptors.equal(a.groupDescriptors)
	return localMatch && sbMatch && gdMatch
}

// Create creates an ext4 filesystem in a given file or device
//
// requires the backend.Storage where to create the filesystem, size is the size of the filesystem in bytes,
// start is how far in bytes from the beginning of the backend.Storage to create the filesystem,
// and sectorsize is is the logical sector size to use for creating the filesystem
//
// blocksize is the size of the ext4 blocks, and is calculated as sectorsPerBlock * sectorsize.
// By ext4 specification, it must be between 512 and 4096 bytes,
// where sectorsize is the provided parameter, and sectorsPerBlock is part of `p *Params`.
// If either sectorsize or p.SectorsPerBlock is 0, it will calculate the optimal size for both.
//
// note that you are *not* required to create the filesystem on the entire disk. You could have a disk of size
// 20GB, and create a small filesystem of size 50MB that begins 2GB into the disk.
// This is extremely useful for creating filesystems on disk partitions.
//
// Note, however, that it is much easier to do this using the higher-level APIs at github.com/diskfs/go-diskfs
// which allow you to work directly with partitions, rather than having to calculate (and hopefully not make any errors)
// where a partition starts and ends.
//
// If the provided blocksize is 0, it will use the default of 512 bytes. If it is any number other than 0
// or 512, it will return an error.
//
//nolint:gocyclo // yes, this has high cyclomatic complexity, but we can accept it
func Create(b backend.Storage, size, start, sectorsize int64, p *Params) (*FileSystem, error) {
	// be safe about the params pointer
	if p == nil {
		p = &Params{}
	}

	// sectorsize must be <=0 or exactly SectorSize512 or error
	// because of this, we know we can scale it down to a uint32, since it only can be 512 bytes
	if sectorsize != int64(SectorSize512) && sectorsize > 0 {
		return nil, fmt.Errorf("sectorsize for ext4 must be either 512 bytes or 0, not %d", sectorsize)
	}
	if sectorsize == 0 {
		sectorsize = int64(SectorSize512)
	}
	var sectorsize32 = uint32(sectorsize)
	// there almost are no limits on an ext4 fs - theoretically up to 1 YB
	// but we do have to check the max and min size per the requested parameters
	//   if size < minSizeGivenParameters {
	// 	    return nil, fmt.Errorf("requested size is smaller than minimum allowed ext4 size %d for given parameters", minSizeGivenParameters*4)
	//   }
	//   if size > maxSizeGivenParameters {
	//	     return nil, fmt.Errorf("requested size is bigger than maximum ext4 size %d for given parameters", maxSizeGivenParameters*4)
	//   }

	// uuid
	fsuuid := p.UUID
	if fsuuid == nil {
		fsuuid2, _ := uuid.NewRandom()
		fsuuid = &fsuuid2
	}

	// blocksize
	sectorsPerBlock := p.SectorsPerBlock
	// whether or not the user provided a blocksize
	// if they did, we will stick with it, as long as it is valid.
	// if they did not, then we are free to calculate it
	var userProvidedBlocksize bool
	switch {
	case sectorsPerBlock == 0:
		sectorsPerBlock = 2
		userProvidedBlocksize = false
	case sectorsPerBlock > 128 || sectorsPerBlock < 2:
		return nil, fmt.Errorf("invalid sectors per block %d, must be between %d and %d sectors", sectorsPerBlock, 2, 128)
	default:
		userProvidedBlocksize = true
	}
	blocksize := uint32(sectorsPerBlock) * sectorsize32

	// how many whole blocks is that?
	numblocks := size / int64(blocksize)

	// recalculate if it was not user provided
	if !userProvidedBlocksize {
		sectorsPerBlockR, blocksizeR, numblocksR := recalculateBlocksize(numblocks, size)
		_, blocksize, numblocks = uint8(sectorsPerBlockR), blocksizeR, numblocksR
	}

	// how many blocks in each block group (and therefore how many block groups)
	// if not provided, by default it is 8*blocksize (in bytes)
	blocksPerGroup := p.BlocksPerGroup
	switch {
	case blocksPerGroup <= 0:
		blocksPerGroup = blocksize * 8
	case blocksPerGroup < minBlocksPerGroup:
		return nil, fmt.Errorf("invalid number of blocks per group %d, must be at least %d", blocksPerGroup, minBlocksPerGroup)
	case blocksPerGroup > 8*blocksize:
		return nil, fmt.Errorf("invalid number of blocks per group %d, must be no larger than 8*blocksize of %d", blocksPerGroup, blocksize)
	case blocksPerGroup%8 != 0:
		return nil, fmt.Errorf("invalid number of blocks per group %d, must be divisible by 8", blocksPerGroup)
	}

	// how many block groups do we have?
	blockGroups := numblocks / int64(blocksPerGroup)

	// track how many free blocks we have
	freeBlocks := numblocks

	clusterSize := p.ClusterSize

	// use our inode ratio to determine how many inodes we should have
	inodeRatio := p.InodeRatio
	if inodeRatio <= 0 {
		inodeRatio = DefaultInodeRatio
	}
	if inodeRatio < int64(blocksize) {
		inodeRatio = int64(blocksize)
	}
	if inodeRatio < clusterSize {
		inodeRatio = clusterSize
	}

	inodeCount := p.InodeCount
	switch {
	case inodeCount <= 0:
		// calculate how many inodes are needed
		inodeCount64 := (numblocks * int64(blocksize)) / inodeRatio
		if uint64(inodeCount64) > max32Num {
			return nil, fmt.Errorf("requested %d inodes, greater than max %d", inodeCount64, max32Num)
		}
		inodeCount = uint32(inodeCount64)
	case uint64(inodeCount) > max32Num:
		return nil, fmt.Errorf("requested %d inodes, greater than max %d", inodeCount, max32Num)
	}

	inodesPerGroup := int64(inodeCount) / blockGroups

	// track how many free inodes we have
	freeInodes := inodeCount

	// which blocks have superblock and GDT?
	var (
		backupSuperblocks            []int64
		backupSuperblockGroupsSparse [2]uint32
	)
	//  0 - primary
	//  ?? - backups
	switch p.SparseSuperVersion {
	case 2:
		// backups in first and last block group
		backupSuperblockGroupsSparse = [2]uint32{0, uint32(blockGroups) - 1}
		backupSuperblocks = []int64{0, 1, blockGroups - 1}
	default:
		backupSuperblockGroups := calculateBackupSuperblockGroups(blockGroups)
		backupSuperblocks = []int64{0}
		for _, bg := range backupSuperblockGroups {
			backupSuperblocks = append(backupSuperblocks, bg*int64(blocksPerGroup))
		}
	}

	freeBlocks -= int64(len(backupSuperblocks))

	var firstDataBlock uint32
	if blocksize == 1024 {
		firstDataBlock = 1
	}

	/*
		size calculations
		we have the total size of the disk from `size uint64`
		we have the sectorsize fixed at SectorSize512

		what do we need to determine or calculate?
		- block size
		- number of blocks
		- number of block groups
		- block groups for superblock and gdt backups
		- in each block group:
				- number of blocks in gdt
				- number of reserved blocks in gdt
				- number of blocks in inode table
				- number of data blocks

		config info:

		[defaults]
			base_features = sparse_super,large_file,filetype,resize_inode,dir_index,ext_attr
			default_mntopts = acl,user_xattr
			enable_periodic_fsck = 0
			blocksize = 4096
			inode_size = 256
			inode_ratio = 16384

		[fs_types]
			ext3 = {
				features = has_journal
			}
			ext4 = {
				features = has_journal,extent,huge_file,flex_bg,uninit_bg,64bit,dir_nlink,extra_isize
				inode_size = 256
			}
			ext4dev = {
				features = has_journal,extent,huge_file,flex_bg,uninit_bg,inline_data,64bit,dir_nlink,extra_isize
				inode_size = 256
				options = test_fs=1
			}
			small = {
				blocksize = 1024
				inode_size = 128
				inode_ratio = 4096
			}
			floppy = {
				blocksize = 1024
				inode_size = 128
				inode_ratio = 8192
			}
			big = {
				inode_ratio = 32768
			}
			huge = {
				inode_ratio = 65536
			}
			news = {
				inode_ratio = 4096
			}
			largefile = {
				inode_ratio = 1048576
				blocksize = -1
			}
			largefile4 = {
				inode_ratio = 4194304
				blocksize = -1
			}
			hurd = {
			     blocksize = 4096
			     inode_size = 128
			}
	*/

	// allocate root directory, single inode
	freeInodes--

	// how many reserved blocks?
	reservedBlocksPercent := p.ReservedBlocksPercent
	if reservedBlocksPercent <= 0 {
		reservedBlocksPercent = DefaultReservedBlocksPercent
	}

	// are checksums enabled?
	gdtChecksumType := gdtChecksumNone
	if p.Checksum {
		gdtChecksumType = gdtChecksumMetadata
	}

	// we do not yet support bigalloc
	var clustersPerGroup = blocksPerGroup

	// inodesPerGroup: once we know how many inodes per group, and how many groups
	//   we will have the total inode count

	volumeName := p.VolumeName
	if volumeName == "" {
		volumeName = DefaultVolumeName
	}

	fflags := defaultFeatureFlags
	for _, flagopt := range p.Features {
		flagopt(&fflags)
	}

	mflags := defaultMiscFlags

	// generate hash seed
	hashSeed, _ := uuid.NewRandom()
	hashSeedBytes := hashSeed[:]
	htreeSeed := make([]uint32, 0, 4)
	htreeSeed = append(htreeSeed,
		binary.LittleEndian.Uint32(hashSeedBytes[:4]),
		binary.LittleEndian.Uint32(hashSeedBytes[4:8]),
		binary.LittleEndian.Uint32(hashSeedBytes[8:12]),
		binary.LittleEndian.Uint32(hashSeedBytes[12:16]),
	)

	// create a UUID for the journal
	journalSuperblockUUID, _ := uuid.NewRandom()

	// group descriptor size could be 32 or 64, depending on option
	var gdSize uint16
	if fflags.fs64Bit {
		gdSize = groupDescriptorSize64Bit
	}

	var firstMetaBG uint32
	if fflags.metaBlockGroups {
		return nil, fmt.Errorf("meta block groups not yet supported")
	}

	// calculate the maximum number of block groups
	// maxBlockGroups = (maxFSSize) / (blocksPerGroup * blocksize)
	var (
		maxBlockGroups uint64
	)
	if fflags.fs64Bit {
		maxBlockGroups = maxFilesystemSize64Bit / (uint64(blocksPerGroup) * uint64(blocksize))
	} else {
		maxBlockGroups = maxFilesystemSize32Bit / (uint64(blocksPerGroup) * uint64(blocksize))
	}
	reservedGDTBlocks := maxBlockGroups * 32 / maxBlockGroups
	if reservedGDTBlocks > math.MaxUint16 {
		return nil, fmt.Errorf("too many reserved blocks calculated for group descriptor table")
	}

	var (
		journalDeviceNumber uint32
		err                 error
	)
	if fflags.separateJournalDevice && p.JournalDevice != "" {
		journalDeviceNumber, err = journalDevice(p.JournalDevice)
		if err != nil {
			return nil, fmt.Errorf("unable to get journal device: %w", err)
		}
	}

	// get default mount options
	mountOptions := defaultMountOptionsFromOpts(p.DefaultMountOpts)

	// initial KB written. This must be adjusted over time to include:
	// - superblock itself (1KB bytes)
	// - GDT
	// - block bitmap (1KB per block group)
	// - inode bitmap (1KB per block group)
	// - inode tables (inodes per block group * bytes per inode)
	// - root directory

	// for now, we just make it 1024 = 1 KB
	initialKB := 1024

	// only set a project quota inode if the feature was enabled
	var projectQuotaInode uint32
	if fflags.projectQuotas {
		projectQuotaInode = lostFoundInode + 1
		freeInodes--
	}

	// how many log groups per flex group? Depends on if we have flex groups
	logGroupsPerFlex := 0
	if fflags.flexBlockGroups {
		logGroupsPerFlex = defaultLogGroupsPerFlex
		if p.LogFlexBlockGroups > 0 {
			logGroupsPerFlex = p.LogFlexBlockGroups
		}
	}

	// create the superblock - MUST ADD IN OPTIONS
	now, epoch := time.Now(), time.Unix(0, 0)
	sb := superblock{
		inodeCount:                   inodeCount,
		blockCount:                   uint64(numblocks),
		reservedBlocks:               uint64(reservedBlocksPercent) / 100 * uint64(numblocks),
		freeBlocks:                   uint64(freeBlocks),
		freeInodes:                   freeInodes,
		firstDataBlock:               firstDataBlock,
		blockSize:                    blocksize,
		clusterSize:                  uint64(clusterSize),
		blocksPerGroup:               blocksPerGroup,
		clustersPerGroup:             clustersPerGroup,
		inodesPerGroup:               uint32(inodesPerGroup),
		mountTime:                    now,
		writeTime:                    now,
		mountCount:                   0,
		mountsToFsck:                 0,
		filesystemState:              fsStateCleanlyUnmounted,
		errorBehaviour:               errorsContinue,
		minorRevision:                0,
		lastCheck:                    now,
		checkInterval:                0,
		creatorOS:                    osLinux,
		revisionLevel:                1,
		reservedBlocksDefaultUID:     0,
		reservedBlocksDefaultGID:     0,
		firstNonReservedInode:        firstNonReservedInode,
		inodeSize:                    uint16(DefaultInodeSize),
		blockGroup:                   0,
		features:                     fflags,
		uuid:                         fsuuid,
		volumeLabel:                  volumeName,
		lastMountedDirectory:         "/",
		algorithmUsageBitmap:         0, // not used in Linux e2fsprogs
		preallocationBlocks:          0, // not used in Linux e2fsprogs
		preallocationDirectoryBlocks: 0, // not used in Linux e2fsprogs
		reservedGDTBlocks:            uint16(reservedGDTBlocks),
		journalSuperblockUUID:        &journalSuperblockUUID,
		journalInode:                 journalInode,
		journalDeviceNumber:          journalDeviceNumber,
		orphanedInodesStart:          0,
		hashTreeSeed:                 htreeSeed,
		hashVersion:                  hashHalfMD4,
		groupDescriptorSize:          gdSize,
		defaultMountOptions:          *mountOptions,
		firstMetablockGroup:          firstMetaBG,
		mkfsTime:                     now,
		journalBackup:                nil,
		// 64-bit mode features
		inodeMinBytes:                minInodeExtraSize,
		inodeReserveBytes:            wantInodeExtraSize,
		miscFlags:                    mflags,
		raidStride:                   0,
		multiMountPreventionInterval: 0,
		multiMountProtectionBlock:    0,
		raidStripeWidth:              0,
		checksumType:                 checksumType,
		totalKBWritten:               uint64(initialKB),
		errorCount:                   0,
		errorFirstTime:               epoch,
		errorFirstInode:              0,
		errorFirstBlock:              0,
		errorFirstFunction:           "",
		errorFirstLine:               0,
		errorLastTime:                epoch,
		errorLastInode:               0,
		errorLastLine:                0,
		errorLastBlock:               0,
		errorLastFunction:            "",
		mountOptions:                 "", // no mount options until it is mounted
		backupSuperblockBlockGroups:  backupSuperblockGroupsSparse,
		lostFoundInode:               lostFoundInode,
		overheadBlocks:               0,
		checksumSeed:                 crc.CRC32c(0, fsuuid[:]), // according to docs, this should be crc32c(~0, $orig_fs_uuid)
		snapshotInodeNumber:          0,
		snapshotID:                   0,
		snapshotReservedBlocks:       0,
		snapshotStartInode:           0,
		userQuotaInode:               userQuotaInode,
		groupQuotaInode:              groupQuotaInode,
		projectQuotaInode:            projectQuotaInode,
		logGroupsPerFlex:             uint64(logGroupsPerFlex),
	}
	gdt := groupDescriptors{}

	superblockBytes, err := sb.toBytes()
	if err != nil {
		return nil, fmt.Errorf("error converting Superblock to bytes: %v", err)
	}

	g := gdt.toBytes(gdtChecksumType, sb.checksumSeed)
	// how big should the GDT be?
	gdSize = groupDescriptorSize
	if sb.features.fs64Bit {
		gdSize = groupDescriptorSize64Bit
	}
	gdtSize := int64(gdSize) * numblocks
	// write the superblock and GDT to the various locations on disk
	for _, bg := range backupSuperblocks {
		block := bg * int64(blocksPerGroup)
		blockStart := block * int64(blocksize)
		// allow that the first one requires an offset
		incr := int64(0)
		if block == 0 {
			incr = int64(SectorSize512) * 2
		}

		writable, err := b.Writable()
		if err != nil {
			return nil, err
		}

		// write the superblock
		count, err := writable.WriteAt(superblockBytes, incr+blockStart+start)
		if err != nil {
			return nil, fmt.Errorf("error writing Superblock for block %d to disk: %v", block, err)
		}
		if count != int(SuperblockSize) {
			return nil, fmt.Errorf("wrote %d bytes of Superblock for block %d to disk instead of expected %d", count, block, SuperblockSize)
		}

		// write the GDT
		count, err = writable.WriteAt(g, incr+blockStart+int64(SuperblockSize)+start)
		if err != nil {
			return nil, fmt.Errorf("error writing GDT for block %d to disk: %v", block, err)
		}
		if count != int(gdtSize) {
			return nil, fmt.Errorf("wrote %d bytes of GDT for block %d to disk instead of expected %d", count, block, gdtSize)
		}
	}

	// create root directory
	// there is nothing in there
	return &FileSystem{
		bootSector:       []byte{},
		superblock:       &sb,
		groupDescriptors: &gdt,
		blockGroups:      blockGroups,
		size:             size,
		start:            start,
		backend:          b,
	}, nil
}

// Read reads a filesystem from a given disk.
//
// requires the backend.File where to read the filesystem, size is the size of the filesystem in bytes,
// start is how far in bytes from the beginning of the backend.File the filesystem is expected to begin,
// and blocksize is is the logical blocksize to use for creating the filesystem
//
// note that you are *not* required to read a filesystem on the entire disk. You could have a disk of size
// 20GB, and a small filesystem of size 50MB that begins 2GB into the disk.
// This is extremely useful for working with filesystems on disk partitions.
//
// Note, however, that it is much easier to do this using the higher-level APIs at github.com/diskfs/go-diskfs
// which allow you to work directly with partitions, rather than having to calculate (and hopefully not make any errors)
// where a partition starts and ends.
//
// If the provided blocksize is 0, it will use the default of 512 bytes. If it is any number other than 0
// or 512, it will return an error.
func Read(b backend.Storage, size, start, sectorsize int64) (*FileSystem, error) {
	// blocksize must be <=0 or exactly SectorSize512 or error
	if sectorsize != int64(SectorSize512) && sectorsize > 0 {
		return nil, fmt.Errorf("sectorsize for ext4 must be either 512 bytes or 0, not %d", sectorsize)
	}
	// we do not check for ext4 max size because it is theoreticallt 1YB, which is bigger than an int64! Even 1ZB is!
	if size < Ext4MinSize {
		return nil, fmt.Errorf("requested size is smaller than minimum allowed ext4 size %d", Ext4MinSize)
	}

	// load the information from the disk
	// read boot sector code
	bs := make([]byte, BootSectorSize)
	n, err := b.ReadAt(bs, start)
	if err != nil {
		return nil, fmt.Errorf("could not read boot sector bytes from file: %v", err)
	}
	if uint16(n) < uint16(BootSectorSize) {
		return nil, fmt.Errorf("only could read %d boot sector bytes from file", n)
	}

	// read the superblock
	// the superblock is one minimal block, i.e. 2 sectors
	superblockBytes := make([]byte, SuperblockSize)
	n, err = b.ReadAt(superblockBytes, start+int64(BootSectorSize))
	if err != nil {
		return nil, fmt.Errorf("could not read superblock bytes from file: %v", err)
	}
	if uint16(n) < uint16(SuperblockSize) {
		return nil, fmt.Errorf("only could read %d superblock bytes from file", n)
	}

	// convert the bytes into a superblock structure
	sb, err := superblockFromBytes(superblockBytes)
	if err != nil {
		return nil, fmt.Errorf("could not interpret superblock data: %v", err)
	}

	// now read the GDT
	// how big should the GDT be?
	gdtSize := uint64(sb.groupDescriptorSize) * sb.blockGroupCount()

	gdtBytes := make([]byte, gdtSize)
	// where do we find the GDT?
	// - if blocksize is 1024, then 1024 padding for BootSector is block 0, 1024 for superblock is block 1
	//   and then the GDT starts at block 2
	// - if blocksize is larger than 1024, then 1024 padding for BootSector followed by 1024 for superblock
	//   is block 0, and then the GDT starts at block 1
	gdtBlock := 1
	if sb.blockSize == 1024 {
		gdtBlock = 2
	}
	n, err = b.ReadAt(gdtBytes, start+int64(gdtBlock)*int64(sb.blockSize))
	if err != nil {
		return nil, fmt.Errorf("could not read Group Descriptor Table bytes from file: %v", err)
	}
	if uint64(n) < gdtSize {
		return nil, fmt.Errorf("only could read %d Group Descriptor Table bytes from file instead of %d", n, gdtSize)
	}
	gdt, err := groupDescriptorsFromBytes(gdtBytes, sb.groupDescriptorSize, sb.checksumSeed, sb.gdtChecksumType())
	if err != nil {
		return nil, fmt.Errorf("could not interpret Group Descriptor Table data: %v", err)
	}

	return &FileSystem{
		bootSector:       bs,
		superblock:       sb,
		groupDescriptors: gdt,
		blockGroups:      int64(sb.blockGroupCount()),
		size:             size,
		start:            start,
		backend:          b,
	}, nil
}

// interface guard
var _ filesystem.FileSystem = (*FileSystem)(nil)

// Type returns the type code for the filesystem. Always returns filesystem.TypeExt4
func (fs *FileSystem) Type() filesystem.Type {
	return filesystem.TypeExt4
}

// Mkdir make a directory at the given path. It is equivalent to `mkdir -p`, i.e. idempotent, in that:
//
// * It will make the entire tree path if it does not exist
// * It will not return an error if the path already exists
func (fs *FileSystem) Mkdir(p string) error {
	_, err := fs.readDirWithMkdir(p, true)
	// we are not interesting in returning the entries
	return err
}

// creates a filesystem node (file, device special file, or named pipe) named pathname,
// with attributes specified by mode and dev
//
//nolint:revive // parameters will be used eventually
func (fs *FileSystem) Mknod(pathname string, mode uint32, dev int) error {
	return filesystem.ErrNotImplemented
}

// creates a new link (also known as a hard link) to an existing file.
//
//nolint:revive // parameters will be used eventually
func (fs *FileSystem) Link(oldpath, newpath string) error {
	return filesystem.ErrNotImplemented
}

// creates a symbolic link named linkpath which contains the string target.
//
//nolint:revive // parameters will be used eventually
func (fs *FileSystem) Symlink(oldpath, newpath string) error {
	return filesystem.ErrNotImplemented
}

// Chmod changes the mode of the named file to mode. If the file is a symbolic link,
// it changes the mode of the link's target.
//
//nolint:revive // parameters will be used eventually
func (fs *FileSystem) Chmod(name string, mode os.FileMode) error {
	return filesystem.ErrNotImplemented
}

// Chown changes the numeric uid and gid of the named file. If the file is a symbolic link,
// it changes the uid and gid of the link's target. A uid or gid of -1 means to not change that value
//
//nolint:revive // parameters will be used eventually
func (fs *FileSystem) Chown(name string, uid, gid int) error {
	return filesystem.ErrNotImplemented
}

// ReadDir return the contents of a given directory in a given filesystem.
//
// Returns a slice of os.FileInfo with all of the entries in the directory.
//
// Will return an error if the directory does not exist or is a regular file and not a directory
func (fs *FileSystem) ReadDir(p string) ([]os.FileInfo, error) {
	dir, err := fs.readDirWithMkdir(p, false)
	if err != nil {
		return nil, fmt.Errorf("error reading directory %s: %v", p, err)
	}
	// once we have made it here, looping is done. We have found the final entry
	// we need to return all of the file info
	count := len(dir.entries)
	ret := make([]os.FileInfo, count)
	for i, e := range dir.entries {
		in, err := fs.readInode(e.inode)
		if err != nil {
			return nil, fmt.Errorf("could not read inode %d at position %d in directory: %v", e.inode, i, err)
		}
		ret[i] = &FileInfo{
			modTime: in.modifyTime,
			name:    e.filename,
			size:    int64(in.size),
			isDir:   e.fileType == dirFileTypeDirectory,
		}
	}

	return ret, nil
}

// OpenFile returns an io.ReadWriter from which you can read the contents of a file
// or write contents to the file
//
// accepts normal os.OpenFile flags
//
// returns an error if the file does not exist
func (fs *FileSystem) OpenFile(p string, flag int) (filesystem.File, error) {
	filename := path.Base(p)
	dir := path.Dir(p)
	parentDir, entry, err := fs.getEntryAndParent(p)
	if err != nil {
		return nil, err
	}
	if entry != nil && entry.fileType == dirFileTypeDirectory {
		return nil, fmt.Errorf("cannot open directory %s as file", p)
	}

	// see if the file exists
	// if the file does not exist, and is not opened for os.O_CREATE, return an error
	if entry == nil {
		if flag&os.O_CREATE == 0 {
			return nil, fmt.Errorf("target file %s does not exist and was not asked to create", p)
		}
		// else create it
		entry, err = fs.mkFile(parentDir, filename)
		if err != nil {
			return nil, fmt.Errorf("failed to create file %s: %v", p, err)
		}
	}
	// get the inode
	inodeNumber := entry.inode
	inode, err := fs.readInode(inodeNumber)
	if err != nil {
		return nil, fmt.Errorf("could not read inode number %d: %v", inodeNumber, err)
	}

	// if a symlink, read the target, rather than the inode itself, which does not point to anything
	if inode.fileType == fileTypeSymbolicLink {
		// is the symlink relative or absolute?
		linkTarget := inode.linkTarget
		if !path.IsAbs(linkTarget) {
			// convert it into an absolute path
			// and start the process again
			linkTarget = path.Join(dir, linkTarget)
			// we probably could make this more efficient by checking if the final linkTarget
			// is in the same directory as we already are parsing, rather than walking the whole thing again
			// leave that for the future.
			linkTarget = path.Clean(linkTarget)
		}
		return fs.OpenFile(linkTarget, flag)
	}
	offset := int64(0)
	if flag&os.O_APPEND == os.O_APPEND {
		offset = int64(inode.size)
	}
	// when we open a file, we load the inode but also all of the extents
	extents, err := inode.extents.blocks(fs)
	if err != nil {
		return nil, fmt.Errorf("could not read extent tree for inode %d: %v", inodeNumber, err)
	}
	return &File{
		directoryEntry: entry,
		inode:          inode,
		isReadWrite:    flag&os.O_RDWR != 0,
		isAppend:       flag&os.O_APPEND != 0,
		offset:         offset,
		filesystem:     fs,
		extents:        extents,
	}, nil
}

// Label read the volume label
func (fs *FileSystem) Label() string {
	if fs.superblock == nil {
		return ""
	}
	return fs.superblock.volumeLabel
}

// Rename renames (moves) oldpath to newpath. If newpath already exists and is not a directory, Rename replaces it.
//
//nolint:revive // parameters will be used eventually
func (fs *FileSystem) Rename(oldpath, newpath string) error {
	return filesystem.ErrNotImplemented
}

// Deprecated: use filesystem.Remove(p string) instead
func (fs *FileSystem) Rm(p string) error {
	return fs.Remove(p)
}

// Removes file or directory at path.
// If path is directory, it only will remove if it is empty.
// If path is a file, it will remove the file.
// Will not remove any parents.
// Error if the file does not exist or is not an empty directory
func (fs *FileSystem) Remove(p string) error {
	parentDir, entry, err := fs.getEntryAndParent(p)
	if err != nil {
		return err
	}
	if parentDir.root && entry == &parentDir.directoryEntry {
		return fmt.Errorf("cannot remove root directory")
	}
	if entry == nil {
		return fmt.Errorf("file does not exist: %s", p)
	}

	writableFile, err := fs.backend.Writable()

	if err != nil {
		return err
	}
	// if it is a directory, it must be empty
	if entry.fileType == dirFileTypeDirectory {
		// read the directory
		entries, err := fs.readDirectory(entry.inode)
		if err != nil {
			return fmt.Errorf("could not read directory %s: %v", p, err)
		}
		if len(entries) > 2 {
			return fmt.Errorf("directory not empty: %s", p)
		}
	}
	// at this point, it is either a file or an empty directory, so remove it

	// free up the blocks
	// read the inode to find the blocks
	removedInode, err := fs.readInode(entry.inode)
	if err != nil {
		return fmt.Errorf("could not read inode %d for %s: %v", entry.inode, p, err)
	}
	extents, err := removedInode.extents.blocks(fs)
	if err != nil {
		return fmt.Errorf("could not read extents for inode %d for %s: %v", entry.inode, p, err)
	}
	// clear the inode from the inode bitmap
	inodeBG := blockGroupForInode(int(entry.inode), fs.superblock.inodesPerGroup)
	inodeBitmap, err := fs.readInodeBitmap(inodeBG)
	if err != nil {
		return fmt.Errorf("could not read inode bitmap: %v", err)
	}
	// clear up the blocks from the block bitmap. We are not clearing the block content, just the bitmap.
	// keep a cache of bitmaps, so we do not have to read them again and again
	blockBitmaps := make(map[int]*util.Bitmap)
	for _, e := range extents {
		for i := e.startingBlock; i < e.startingBlock+uint64(e.count); i++ {
			// determine what block group this block is in, and read the bitmap for that blockgroup
			bg := blockGroupForBlock(int(i), fs.superblock.blocksPerGroup)
			dataBlockBitmap, ok := blockBitmaps[bg]
			if !ok {
				dataBlockBitmap, err = fs.readBlockBitmap(bg)
				if err != nil {
					return fmt.Errorf("could not read block bitmap: %v", err)
				}
				blockBitmaps[bg] = dataBlockBitmap
			}
			// the extent lists the absolute block number, but the bitmap is relative to the block group
			blockInBG := int(i) - int(fs.superblock.blocksPerGroup)*bg
			if err := dataBlockBitmap.Clear(blockInBG); err != nil {
				return fmt.Errorf("could not clear block bitmap for block %d: %v", i, err)
			}
		}
	}
	for bg, dataBlockBitmap := range blockBitmaps {
		if err := fs.writeBlockBitmap(dataBlockBitmap, bg); err != nil {
			return fmt.Errorf("could not write block bitmap back to disk: %v", err)
		}
	}

	// remove the directory entry from the parent
	newEntries := make([]*directoryEntry, 0, len(parentDir.entries)-1)
	for _, e := range parentDir.entries {
		if e.inode == entry.inode {
			continue
		}
		newEntries = append(newEntries, e)
	}
	parentDir.entries = newEntries
	// write the parent directory back
	dirBytes := parentDir.toBytes(fs.superblock.blockSize, directoryChecksumAppender(fs.superblock.checksumSeed, parentDir.inode, 0))
	parentInode, err := fs.readInode(parentDir.inode)
	if err != nil {
		return fmt.Errorf("could not read inode %d for %s: %v", entry.inode, path.Base(p), err)
	}
	extents, err = parentInode.extents.blocks(fs)
	if err != nil {
		return fmt.Errorf("could not read extents for inode %d for %s: %v", entry.inode, path.Base(p), err)
	}
	for _, e := range extents {
		for i := 0; i < int(e.count); i++ {
			b := dirBytes[i:fs.superblock.blockSize]
			if _, err := writableFile.WriteAt(b, (int64(i)+int64(e.startingBlock))*int64(fs.superblock.blockSize)); err != nil {
				return fmt.Errorf("could not write inode bitmap back to disk: %v", err)
			}
		}
	}

	// remove the inode from the bitmap and write the inode bitmap back
	// inode is absolute, but bitmap is relative to block group
	inodeInBG := int(entry.inode) - int(fs.superblock.inodesPerGroup)*inodeBG
	if err := inodeBitmap.Clear(inodeInBG); err != nil {
		return fmt.Errorf("could not clear inode bitmap for inode %d: %v", entry.inode, err)
	}

	// write the inode bitmap back
	if err := fs.writeInodeBitmap(inodeBitmap, inodeBG); err != nil {
		return fmt.Errorf("could not write inode bitmap back to disk: %v", err)
	}
	// update the group descriptor
	gd := fs.groupDescriptors.descriptors[inodeBG]

	// update the group descriptor inodes and blocks
	gd.freeInodes++
	gd.freeBlocks += uint32(removedInode.blocks)
	// write the group descriptor back
	gdBytes := gd.toBytes(fs.superblock.gdtChecksumType(), fs.superblock.uuid.ID())
	gdtBlock := 1
	if fs.superblock.blockSize == 1024 {
		gdtBlock = 2
	}
	if _, err := writableFile.WriteAt(gdBytes, fs.start+int64(gdtBlock)*int64(fs.superblock.blockSize)+int64(gd.number)*int64(fs.superblock.groupDescriptorSize)); err != nil {
		return fmt.Errorf("could not write Group Descriptor bytes to file: %v", err)
	}

	// we could remove the inode from the inode table in the group descriptor,
	// but we do not need to do so. Since we are not reusing the inode, we can just leave it there,
	// the bitmap always is checked before reusing an inode location.
	fs.superblock.freeInodes++
	fs.superblock.freeBlocks += removedInode.blocks
	return fs.writeSuperblock()
}

func (fs *FileSystem) Truncate(p string, size int64) error {
	_, entry, err := fs.getEntryAndParent(p)
	if err != nil {
		return err
	}
	if entry == nil {
		return fmt.Errorf("file does not exist: %s", p)
	}
	if entry.fileType == dirFileTypeDirectory {
		return fmt.Errorf("cannot truncate directory %s", p)
	}
	// it is not a directory, and it exists, so truncate it
	inode, err := fs.readInode(entry.inode)
	if err != nil {
		return fmt.Errorf("could not read inode %d in directory: %v", entry.inode, err)
	}
	// change the file size
	inode.size = uint64(size)

	// free used blocks if shrank, or reserve new blocks if grew
	// both of which mean updating the superblock, and the extents tree in the inode

	// write the inode back
	return fs.writeInode(inode)
}

// getEntryAndParent given a path, get the Directory for the parent and the directory entry for the file.
// If the directory does not exist, returns an error.
// If the file does not exist, does not return an error, but rather returns a nil entry.
func (fs *FileSystem) getEntryAndParent(p string) (parent *Directory, entry *directoryEntry, err error) {
	dir := path.Dir(p)
	filename := path.Base(p)
	// get the directory entries
	parentDir, err := fs.readDirWithMkdir(dir, false)
	if err != nil {
		return nil, nil, fmt.Errorf("could not read directory entries for %s", dir)
	}
	// we now know that the directory exists, see if the file exists
	var targetEntry *directoryEntry
	if parentDir.root && filename == "/" {
		// root directory
		return parentDir, &parentDir.directoryEntry, nil
	}

	for _, e := range parentDir.entries {
		if e.filename != filename {
			continue
		}
		// if we got this far, we have found the file
		targetEntry = e
		break
	}
	return parentDir, targetEntry, nil
}

// Stat return fs.FileInfo about a specific file path.
func (fs *FileSystem) Stat(p string) (iofs.FileInfo, error) {
	_, entry, err := fs.getEntryAndParent(p)
	if err != nil {
		return nil, err
	}
	if entry == nil {
		return nil, fmt.Errorf("file does not exist: %s", p)
	}
	in, err := fs.readInode(entry.inode)
	if err != nil {
		return nil, fmt.Errorf("could not read inode %d in directory: %v", entry.inode, err)
	}
	return &FileInfo{
		modTime: in.modifyTime,
		name:    entry.filename,
		size:    int64(in.size),
		isDir:   entry.fileType == dirFileTypeDirectory,
	}, nil
}

// SetLabel changes the label on the writable filesystem. Different file system may hav different
// length constraints.
func (fs *FileSystem) SetLabel(label string) error {
	fs.superblock.volumeLabel = label
	return fs.writeSuperblock()
}

// readInode read a single inode from disk
func (fs *FileSystem) readInode(inodeNumber uint32) (*inode, error) {
	if inodeNumber == 0 {
		return nil, fmt.Errorf("cannot read inode 0")
	}
	sb := fs.superblock
	inodeSize := sb.inodeSize
	inodesPerGroup := sb.inodesPerGroup
	// figure out which block group the inode is on
	bg := (inodeNumber - 1) / inodesPerGroup
	// read the group descriptor to find out the location of the inode table
	gd := fs.groupDescriptors.descriptors[bg]
	inodeTableBlock := gd.inodeTableLocation
	inodeBytes := make([]byte, inodeSize)
	// bytesStart is beginning byte for the inodeTableBlock
	byteStart := inodeTableBlock * uint64(sb.blockSize)
	// offsetInode is how many inodes in our inode is
	offsetInode := (inodeNumber - 1) % inodesPerGroup
	// offset is how many bytes in our inode is
	offset := offsetInode * uint32(inodeSize)
	read, err := fs.backend.ReadAt(inodeBytes, int64(byteStart)+int64(offset))
	if err != nil {
		return nil, fmt.Errorf("failed to read inode %d from offset %d of block %d from block group %d: %v", inodeNumber, offset, inodeTableBlock, bg, err)
	}
	if read != int(inodeSize) {
		return nil, fmt.Errorf("read %d bytes for inode %d instead of inode size of %d", read, inodeNumber, inodeSize)
	}
	inode, err := inodeFromBytes(inodeBytes, sb, inodeNumber)
	if err != nil {
		return nil, fmt.Errorf("could not interpret inode data: %v", err)
	}
	// fill in symlink target if needed
	if inode.fileType == fileTypeSymbolicLink && inode.linkTarget == "" {
		// read the symlink target
		extents, err := inode.extents.blocks(fs)
		if err != nil {
			return nil, fmt.Errorf("could not read extent tree for symlink inode %d: %v", inodeNumber, err)
		}
		b, err := fs.readFileBytes(extents, inode.size)
		if err != nil {
			return nil, fmt.Errorf("could not read symlink target for inode %d: %v", inodeNumber, err)
		}
		inode.linkTarget = string(b)
	}
	return inode, nil
}

// writeInode write a single inode to disk
func (fs *FileSystem) writeInode(i *inode) error {
	writableFile, err := fs.backend.Writable()

	if err != nil {
		return err
	}

	sb := fs.superblock
	inodeSize := sb.inodeSize
	inodesPerGroup := sb.inodesPerGroup
	// figure out which block group the inode is on
	bg := (i.number - 1) / inodesPerGroup
	// read the group descriptor to find out the location of the inode table
	gd := fs.groupDescriptors.descriptors[bg]
	inodeTableBlock := gd.inodeTableLocation
	// bytesStart is beginning byte for the inodeTableBlock
	//   byteStart := inodeTableBlock * sb.blockSize
	// offsetInode is how many inodes in our inode is
	offsetInode := (i.number - 1) % inodesPerGroup
	byteStart := inodeTableBlock * uint64(sb.blockSize)
	// offsetInode is how many inodes in our inode is
	// offset is how many bytes in our inode is
	// offset is how many bytes in our inode is
	offset := int64(offsetInode) * int64(inodeSize)
	inodeBytes := i.toBytes(sb)
	wrote, err := writableFile.WriteAt(inodeBytes, int64(byteStart)+offset)
	if err != nil {
		return fmt.Errorf("failed to write inode %d at offset %d of block %d from block group %d: %v", i.number, offset, inodeTableBlock, bg, err)
	}
	if wrote != int(inodeSize) {
		return fmt.Errorf("wrote %d bytes for inode %d instead of inode size of %d", wrote, i.number, inodeSize)
	}
	return nil
}

// read directory entries for a given directory
func (fs *FileSystem) readDirectory(inodeNumber uint32) ([]*directoryEntry, error) {
	// read the inode for the directory
	in, err := fs.readInode(inodeNumber)
	if err != nil {
		return nil, fmt.Errorf("could not read inode %d for directory: %v", inodeNumber, err)
	}
	// convert the extent tree into a sorted list of extents
	extents, err := in.extents.blocks(fs)
	if err != nil {
		return nil, fmt.Errorf("unable to get blocks for inode %d: %w", in.number, err)
	}
	// read the contents of the file across all blocks
	b, err := fs.readFileBytes(extents, in.size)
	if err != nil {
		return nil, fmt.Errorf("error reading file bytes for inode %d: %v", inodeNumber, err)
	}

	var dirEntries []*directoryEntry
	// TODO: none of this works for hashed dir entries, indicated by in.flags.hashedDirectoryIndexes == true
	if in.flags.hashedDirectoryIndexes {
		treeRoot, err := parseDirectoryTreeRoot(b[:fs.superblock.blockSize], fs.superblock.features.largeDirectory)
		if err != nil {
			return nil, fmt.Errorf("failed to parse directory tree root: %v", err)
		}
		subDirEntries, err := parseDirEntriesHashed(b, treeRoot.depth, treeRoot, fs.superblock.blockSize, fs.superblock.features.metadataChecksums, in.number, in.nfsFileVersion, fs.superblock.checksumSeed)
		if err != nil {
			return nil, fmt.Errorf("failed to parse hashed directory entries: %v", err)
		}
		// include the dot and dotdot entries from treeRoot; they do not show up in the hashed entries
		dirEntries = []*directoryEntry{treeRoot.dotEntry, treeRoot.dotDotEntry}
		dirEntries = append(dirEntries, subDirEntries...)
	} else {
		// convert into directory entries
		dirEntries, err = parseDirEntriesLinear(b, fs.superblock.features.metadataChecksums, fs.superblock.blockSize, in.number, in.nfsFileVersion, fs.superblock.checksumSeed)
	}

	return dirEntries, err
}

// readFileBytes read all of the bytes for an individual file pointed at by a given inode
// normally not very useful, but helpful when reading an entire directory.
func (fs *FileSystem) readFileBytes(extents extents, filesize uint64) ([]byte, error) {
	// walk through each one, gobbling up the bytes
	b := make([]byte, 0, fs.superblock.blockSize)
	for i, e := range extents {
		start := e.startingBlock * uint64(fs.superblock.blockSize)
		count := uint64(e.count) * uint64(fs.superblock.blockSize)
		if uint64(len(b))+count > filesize {
			count = filesize - uint64(len(b))
		}
		b2 := make([]byte, count)
		read, err := fs.backend.ReadAt(b2, int64(start))
		if err != nil {
			return nil, fmt.Errorf("failed to read bytes for extent %d: %v", i, err)
		}
		if read != int(count) {
			return nil, fmt.Errorf("read %d bytes instead of %d for extent %d", read, count, i)
		}
		b = append(b, b2...)
		if uint64(len(b)) >= filesize {
			break
		}
	}
	return b, nil
}

// mkFile make a file with a given name in the given directory.
func (fs *FileSystem) mkFile(parent *Directory, name string) (*directoryEntry, error) {
	return fs.mkDirEntry(parent, name, false)
}

// readDirWithMkdir - walks down a directory tree to the last entry in p.
// For example, if p is /a/b/c, it will walk down to c.
// Expects c to be a directory.
// If each step in the tree does not exist, it will either make it if doMake is true, or return an error.
func (fs *FileSystem) readDirWithMkdir(p string, doMake bool) (*Directory, error) {
	paths := splitPath(p)

	// walk down the directory tree until all paths have been walked or we cannot find something
	// start with the root directory
	var entries []*directoryEntry
	currentDir := &Directory{
		directoryEntry: directoryEntry{
			inode:    rootInode,
			filename: "",
			fileType: dirFileTypeDirectory,
		},
		root: true,
	}
	entries, err := fs.readDirectory(rootInode)
	if err != nil {
		return nil, fmt.Errorf("failed to read directory %s", "/")
	}
	currentDir.entries = entries
	for i, subp := range paths {
		// do we have an entry whose name is the same as this name?
		found := false
		for _, e := range entries {
			if e.filename != subp {
				continue
			}
			if e.fileType != dirFileTypeDirectory {
				return nil, fmt.Errorf("cannot create directory at %s since it is a file", "/"+strings.Join(paths[0:i+1], "/"))
			}
			// the filename matches, and it is a subdirectory, so we can break after saving the directory entry, which contains the inode
			found = true
			currentDir = &Directory{
				directoryEntry: *e,
			}
			break
		}

		// if not, either make it, retrieve its cluster and entries, and loop;
		//  or error out
		if !found {
			if doMake {
				var subdirEntry *directoryEntry
				subdirEntry, err = fs.mkSubdir(currentDir, subp)
				if err != nil {
					return nil, fmt.Errorf("failed to create subdirectory %s", "/"+strings.Join(paths[0:i+1], "/"))
				}
				// save where we are to search next
				currentDir = &Directory{
					directoryEntry: *subdirEntry,
				}
			} else {
				return nil, fmt.Errorf("path %s not found", "/"+strings.Join(paths[0:i+1], "/"))
			}
		}
		// get all of the entries in this directory
		entries, err = fs.readDirectory(currentDir.inode)
		if err != nil {
			return nil, fmt.Errorf("failed to read directory %s", "/"+strings.Join(paths[0:i+1], "/"))
		}
		currentDir.entries = entries
	}
	// once we have made it here, looping is done; we have found the final entry
	currentDir.entries = entries
	return currentDir, nil
}

// readBlock read a single block from disk
func (fs *FileSystem) readBlock(blockNumber uint64) ([]byte, error) {
	sb := fs.superblock
	// bytesStart is beginning byte for the inodeTableBlock
	byteStart := blockNumber * uint64(sb.blockSize)
	blockBytes := make([]byte, sb.blockSize)
	read, err := fs.backend.ReadAt(blockBytes, int64(byteStart))
	if err != nil {
		return nil, fmt.Errorf("failed to read block %d: %v", blockNumber, err)
	}
	if read != int(sb.blockSize) {
		return nil, fmt.Errorf("read %d bytes for block %d instead of size of %d", read, blockNumber, sb.blockSize)
	}
	return blockBytes, nil
}

// recalculate blocksize based on the existing number of blocks
// -      0 <= blocks <   3MM         : floppy - blocksize = 1024
// -    3MM <= blocks < 512MM         : small - blocksize = 1024
// - 512MM <= blocks < 4*1024*1024MM  : default - blocksize =
// - 4*1024*1024MM <= blocks < 16*1024*1024MM  : big - blocksize =
// - 16*1024*1024MM <= blocks   : huge - blocksize =
//
// the original code from e2fsprogs https://git.kernel.org/pub/scm/fs/ext2/e2fsprogs.git/tree/misc/mke2fs.c
func recalculateBlocksize(numblocks, size int64) (sectorsPerBlock int, blocksize uint32, numBlocksAdjusted int64) {
	var (
		million64     = int64(million)
		sectorSize512 = uint32(SectorSize512)
	)
	switch {
	case 0 <= numblocks && numblocks < 3*million64:
		sectorsPerBlock = 2
		blocksize = 2 * sectorSize512
	case 3*million64 <= numblocks && numblocks < 512*million64:
		sectorsPerBlock = 2
		blocksize = 2 * sectorSize512
	case 512*million64 <= numblocks && numblocks < 4*1024*1024*million64:
		sectorsPerBlock = 2
		blocksize = 2 * sectorSize512
	case 4*1024*1024*million64 <= numblocks && numblocks < 16*1024*1024*million64:
		sectorsPerBlock = 2
		blocksize = 2 * sectorSize512
	case numblocks > 16*1024*1024*million64:
		sectorsPerBlock = 2
		blocksize = 2 * sectorSize512
	}
	return sectorsPerBlock, blocksize, size / int64(blocksize)
}

// mkSubdir make a subdirectory of a given name inside the parent
// 1- allocate a single data block for the directory
// 2- create an inode in the inode table pointing to that data block
// 3- mark the inode in the inode bitmap
// 4- mark the data block in the data block bitmap
// 5- create a directory entry in the parent directory data blocks
func (fs *FileSystem) mkSubdir(parent *Directory, name string) (*directoryEntry, error) {
	return fs.mkDirEntry(parent, name, true)
}

func (fs *FileSystem) mkDirEntry(parent *Directory, name string, isDir bool) (*directoryEntry, error) {
	// still to do:
	//  - write directory entry in parent
	//  - write inode to disk

	// create an inode
	inodeNumber, err := fs.allocateInode(parent.inode)
	if err != nil {
		return nil, fmt.Errorf("could not allocate inode for file %s: %w", name, err)
	}
	// get extents for the file - prefer in the same block group as the inode, if possible
	newExtents, err := fs.allocateExtents(1, nil)
	if err != nil {
		return nil, fmt.Errorf("could not allocate disk space for file %s: %w", name, err)
	}
	extentTreeParsed, err := extendExtentTree(nil, newExtents, fs, nil)
	if err != nil {
		return nil, fmt.Errorf("could not convert extents into tree: %w", err)
	}
	// normally, after getting a tree from extents, you would need to then allocate all of the blocks
	//    in the extent tree - leafs and intermediate. However, because we are allocating a new directory
	//    with a single extent, we *know* it can fit in the inode itself (which has a max of 4), so no need

	// create a directory entry for the file
	deFileType := dirFileTypeRegular
	fileType := fileTypeRegularFile
	var contentSize uint64
	if isDir {
		deFileType = dirFileTypeDirectory
		fileType = fileTypeDirectory
		contentSize = uint64(fs.superblock.blockSize)
	}
	de := directoryEntry{
		inode:    inodeNumber,
		filename: name,
		fileType: deFileType,
	}
	parent.entries = append(parent.entries, &de)
	// write the parent out to disk
	bytesPerBlock := fs.superblock.blockSize
	parentDirBytes := parent.toBytes(bytesPerBlock, directoryChecksumAppender(fs.superblock.checksumSeed, parent.inode, 0))
	// check if parent has increased in size beyond allocated blocks
	parentInode, err := fs.readInode(parent.inode)
	if err != nil {
		return nil, fmt.Errorf("could not read inode %d of parent directory: %w", parent.inode, err)
	}

	// write the directory entry in the parent
	// figure out which block it goes into, and possibly rebalance the directory entries hash tree
	parentExtents, err := parentInode.extents.blocks(fs)
	if err != nil {
		return nil, fmt.Errorf("could not read parent extents for directory: %w", err)
	}
	dirFile := &File{
		inode: parentInode,
		directoryEntry: &directoryEntry{
			inode:    parent.inode,
			filename: name,
			fileType: dirFileTypeDirectory,
		},
		filesystem:  fs,
		isReadWrite: true,
		isAppend:    true,
		offset:      0,
		extents:     parentExtents,
	}
	wrote, err := dirFile.Write(parentDirBytes)
	if err != nil && err != io.EOF {
		return nil, fmt.Errorf("unable to write new directory: %w", err)
	}
	if wrote != len(parentDirBytes) {
		return nil, fmt.Errorf("wrote only %d bytes instead of expected %d for new directory", wrote, len(parentDirBytes))
	}

	// write the inode for the new entry out
	now := time.Now()
	in := inode{
		number:                 inodeNumber,
		permissionsGroup:       parentInode.permissionsGroup,
		permissionsOwner:       parentInode.permissionsOwner,
		permissionsOther:       parentInode.permissionsOther,
		fileType:               fileType,
		owner:                  parentInode.owner,
		group:                  parentInode.group,
		size:                   contentSize,
		hardLinks:              2,
		blocks:                 newExtents.blockCount(),
		flags:                  &inodeFlags{},
		nfsFileVersion:         0,
		version:                0,
		inodeSize:              parentInode.inodeSize,
		deletionTime:           0,
		accessTime:             now,
		changeTime:             now,
		createTime:             now,
		modifyTime:             now,
		extendedAttributeBlock: 0,
		project:                0,
		extents:                extentTreeParsed,
	}
	// write the inode to disk
	if err := fs.writeInode(&in); err != nil {
		return nil, fmt.Errorf("could not write inode for new directory: %w", err)
	}
	// if a directory, put entries for . and .. in the first block for the new directory
	if isDir {
		initialEntries := []*directoryEntry{
			{
				inode:    inodeNumber,
				filename: ".",
				fileType: dirFileTypeDirectory,
			},
			{
				inode:    parent.inode,
				filename: "..",
				fileType: dirFileTypeDirectory,
			},
		}
		newDir := Directory{
			directoryEntry: de,
			root:           false,
			entries:        initialEntries,
		}
		dirBytes := newDir.toBytes(fs.superblock.blockSize, directoryChecksumAppender(fs.superblock.checksumSeed, inodeNumber, 0))
		// write the bytes out to disk
		dirFile = &File{
			inode: &in,
			directoryEntry: &directoryEntry{
				inode:    inodeNumber,
				filename: name,
				fileType: dirFileTypeDirectory,
			},
			filesystem:  fs,
			isReadWrite: true,
			isAppend:    true,
			offset:      0,
			extents:     *newExtents,
		}
		wrote, err := dirFile.Write(dirBytes)
		if err != nil && err != io.EOF {
			return nil, fmt.Errorf("unable to write new directory: %w", err)
		}
		if wrote != len(dirBytes) {
			return nil, fmt.Errorf("wrote only %d bytes instead of expected %d for new entry", wrote, len(dirBytes))
		}
	}

	// return
	return &de, nil
}

// allocateInode allocate a single inode
// passed the parent, so it can know where to allocate it
// logic:
//   - parent is  0 : root inode, will allocate at 2
//   - parent is  2 : child of root, will try to spread out
//   - else         : try to collocate with parent, if possible
func (fs *FileSystem) allocateInode(parent uint32) (uint32, error) {
	var (
		inodeNumber = -1
	)
	if parent == 0 {
		inodeNumber = 2
	}
	// load the inode bitmap
	var (
		bg int
		gd groupDescriptor
	)

	writableFile, err := fs.backend.Writable()
	if err != nil {
		return 0, err
	}

	for _, gd = range fs.groupDescriptors.descriptors {
		if inodeNumber != -1 {
			break
		}
		bg := int(gd.number)
		bm, err := fs.readInodeBitmap(bg)
		if err != nil {
			return 0, fmt.Errorf("could not read inode bitmap: %w", err)
		}
		// get first free inode
		inodeNumber = bm.FirstFree(0)
		// if we found a
		if inodeNumber == -1 {
			continue
		}
		// set it as marked
		if err := bm.Set(inodeNumber); err != nil {
			return 0, fmt.Errorf("could not set inode bitmap: %w", err)
		}
		// write the inode bitmap bytes
		if err := fs.writeInodeBitmap(bm, bg); err != nil {
			return 0, fmt.Errorf("could not write inode bitmap: %w", err)
		}
	}
	if inodeNumber == -1 {
		return 0, errors.New("no free inodes available")
	}

	// reduce number of free inodes in that descriptor in the group descriptor table
	gd.freeInodes--

	// get the group descriptor as bytes
	gdBytes := gd.toBytes(fs.superblock.gdtChecksumType(), fs.superblock.uuid.ID())

	// write the group descriptor bytes
	// gdt starts in block 1 of any redundant copies, specifically in BG 0
	gdtBlock := 1
	blockByteLocation := gdtBlock * int(fs.superblock.blockSize)
	gdOffset := fs.start + int64(blockByteLocation) + int64(bg)*int64(fs.superblock.groupDescriptorSize)
	wrote, err := writableFile.WriteAt(gdBytes, gdOffset)
	if err != nil {
		return 0, fmt.Errorf("unable to write group descriptor bytes for blockgroup %d: %v", bg, err)
	}
	if wrote != len(gdBytes) {
		return 0, fmt.Errorf("wrote only %d bytes instead of expected %d for group descriptor of block group %d", wrote, len(gdBytes), bg)
	}

	return uint32(inodeNumber), nil
}

// allocateExtents allocate the data blocks in extents that are
// to be used for a file of a given size
// arguments are file size in bytes and existing extents
// if previous is nil, then we are not (re)sizing an existing file but creating a new one
// returns the extents to be used in order
func (fs *FileSystem) allocateExtents(size uint64, previous *extents) (*extents, error) {
	// 1- calculate how many blocks are needed
	required := size / uint64(fs.superblock.blockSize)
	remainder := size % uint64(fs.superblock.blockSize)
	if remainder > 0 {
		required++
	}
	// 2- see how many blocks already are allocated
	var allocated uint64
	if previous != nil {
		allocated = previous.blockCount()
	}
	// 3- if needed, allocate new blocks in extents
	extraBlockCount := required - allocated
	// if we have enough, do not add anything
	if extraBlockCount <= 0 {
		return previous, nil
	}

	// if there are not enough blocks left on the filesystem, return an error
	if fs.superblock.freeBlocks < extraBlockCount {
		return nil, fmt.Errorf("only %d blocks free, requires additional %d", fs.superblock.freeBlocks, extraBlockCount)
	}

	// now we need to look for as many contiguous blocks as possible
	// first calculate the minimum number of extents needed

	// if all of the extents, except possibly the last, are maximum size, then we need minExtents extents
	// we loop through, trying to allocate an extent as large as our remaining blocks or maxBlocksPerExtent,
	//   whichever is smaller
	blockGroupCount := fs.blockGroups
	// TODO: instead of starting with BG 0, should start with BG where the inode for this file/dir is located
	var (
		newExtents       []extent
		datablockBitmaps = map[int]*util.Bitmap{}
		blocksPerGroup   = fs.superblock.blocksPerGroup
	)

	var i int64
	for i = 0; i < blockGroupCount && allocated < extraBlockCount; i++ {
		// keep track if we allocated anything in this blockgroup
		// 1- read the GDT for this blockgroup to find the location of the block bitmap
		//    and total free blocks
		// 2- read the block bitmap from disk
		// 3- find the maximum contiguous space available
		bs, err := fs.readBlockBitmap(int(i))
		if err != nil {
			return nil, fmt.Errorf("could not read block bitmap for block group %d: %v", i, err)
		}
		// now find our unused blocks and how many there are in a row as potential extents
		if extraBlockCount > maxUint16 {
			return nil, fmt.Errorf("cannot allocate more than %d blocks in a single extent", maxUint16)
		}
		// get the list of free blocks
		blockList := bs.FreeList()

		// create possible extents by size
		// Step 3: Group contiguous blocks into extents
		var extents []extent
		for _, freeBlock := range blockList {
			start, length := freeBlock.Position, freeBlock.Count
			for length > 0 {
				extentLength := min(length, int(maxBlocksPerExtent))
				extents = append(extents, extent{startingBlock: uint64(start) + uint64(i)*uint64(blocksPerGroup), count: uint16(extentLength)})
				start += extentLength
				length -= extentLength
			}
		}

		// sort in descending order
		sort.Slice(extents, func(i, j int) bool {
			return extents[i].count > extents[j].count
		})

		var allocatedBlocks uint64
		for _, ext := range extents {
			if extraBlockCount <= 0 {
				break
			}
			extentToAdd := ext
			if uint64(ext.count) >= extraBlockCount {
				extentToAdd = extent{startingBlock: ext.startingBlock, count: uint16(extraBlockCount)}
			}
			newExtents = append(newExtents, extentToAdd)
			allocatedBlocks += uint64(extentToAdd.count)
			extraBlockCount -= uint64(extentToAdd.count)
			// set the marked blocks in the bitmap, and save the bitmap
			for block := extentToAdd.startingBlock; block < extentToAdd.startingBlock+uint64(extentToAdd.count); block++ {
				// determine what block group this block is in, and read the bitmap for that blockgroup
				// the extent lists the absolute block number, but the bitmap is relative to the block group
				blockInGroup := block - uint64(i)*uint64(blocksPerGroup)
				if err := bs.Set(int(blockInGroup)); err != nil {
					return nil, fmt.Errorf("could not clear block bitmap for block %d: %v", i, err)
				}
			}

			// do *not* write the bitmap back yet, as we do not yet know if we will be able to fulfill the entire request.
			// instead save it for later
			datablockBitmaps[int(i)] = bs
		}
	}
	if extraBlockCount > 0 {
		return nil, fmt.Errorf("could not allocate %d blocks", extraBlockCount)
	}

	// write the block bitmaps back to disk
	for bg, bs := range datablockBitmaps {
		if err := fs.writeBlockBitmap(bs, bg); err != nil {
			return nil, fmt.Errorf("could not write block bitmap for block group %d: %v", bg, err)
		}
	}

	// need to update the total blocks used/free in superblock
	fs.superblock.freeBlocks -= allocated
	// update the blockBitmapChecksum for any updated block groups in GDT
	// write updated superblock and GDT to disk
	if err := fs.writeSuperblock(); err != nil {
		return nil, fmt.Errorf("could not write superblock: %w", err)
	}
	// write backup copies
	var exten extents = newExtents
	return &exten, nil
}

// readInodeBitmap read the inode bitmap off the disk.
// This would be more efficient if we just read one group descriptor's bitmap
// but for now we are about functionality, not efficiency, so it will read the whole thing.
func (fs *FileSystem) readInodeBitmap(group int) (*util.Bitmap, error) {
	if group >= len(fs.groupDescriptors.descriptors) {
		return nil, fmt.Errorf("block group %d does not exist", group)
	}
	gd := fs.groupDescriptors.descriptors[group]
	bitmapLocation := gd.inodeBitmapLocation
	bitmapByteCount := fs.superblock.inodesPerGroup / 8
	b := make([]byte, bitmapByteCount)
	offset := int64(bitmapLocation*uint64(fs.superblock.blockSize) + uint64(fs.start))
	read, err := fs.backend.ReadAt(b, offset)
	if err != nil {
		return nil, fmt.Errorf("unable to read inode bitmap for blockgroup %d: %w", gd.number, err)
	}
	if read != int(bitmapByteCount) {
		return nil, fmt.Errorf("Read %d bytes instead of expected %d for inode bitmap of block group %d", read, bitmapByteCount, gd.number)
	}
	// only take bytes corresponding to the number of inodes per group

	// create a bitmap
	bs := util.NewBitmap(int(fs.superblock.blockSize) * len(fs.groupDescriptors.descriptors))
	bs.FromBytes(b)
	return bs, nil
}

// writeInodeBitmap write the inode bitmap to the disk.
func (fs *FileSystem) writeInodeBitmap(bm *util.Bitmap, group int) error {
	if group >= len(fs.groupDescriptors.descriptors) {
		return fmt.Errorf("block group %d does not exist", group)
	}
	writableFile, err := fs.backend.Writable()
	if err != nil {
		return err
	}
	b := bm.ToBytes()
	gd := fs.groupDescriptors.descriptors[group]
	bitmapByteCount := fs.superblock.inodesPerGroup / 8
	bitmapLocation := gd.inodeBitmapLocation
	offset := int64(bitmapLocation*uint64(fs.superblock.blockSize) + uint64(fs.start))
	wrote, err := writableFile.WriteAt(b, offset)
	if err != nil {
		return fmt.Errorf("unable to write inode bitmap for blockgroup %d: %w", gd.number, err)
	}
	if wrote != int(bitmapByteCount) {
		return fmt.Errorf("wrote %d bytes instead of expected %d for inode bitmap of block group %d", wrote, bitmapByteCount, gd.number)
	}

	return nil
}

func (fs *FileSystem) readBlockBitmap(group int) (*util.Bitmap, error) {
	if group >= len(fs.groupDescriptors.descriptors) {
		return nil, fmt.Errorf("block group %d does not exist", group)
	}
	gd := fs.groupDescriptors.descriptors[group]
	bitmapLocation := gd.blockBitmapLocation
	b := make([]byte, fs.superblock.blockSize)
	offset := int64(bitmapLocation*uint64(fs.superblock.blockSize) + uint64(fs.start))
	read, err := fs.backend.ReadAt(b, offset)
	if err != nil {
		return nil, fmt.Errorf("unable to read block bitmap for blockgroup %d: %w", gd.number, err)
	}
	if read != int(fs.superblock.blockSize) {
		return nil, fmt.Errorf("Read %d bytes instead of expected %d for block bitmap of block group %d", read, fs.superblock.blockSize, gd.number)
	}
	// create a bitmap
	bs := util.NewBitmap(int(fs.superblock.blockSize) * len(fs.groupDescriptors.descriptors))
	bs.FromBytes(b)
	return bs, nil
}

// writeBlockBitmap write the inode bitmap to the disk.
func (fs *FileSystem) writeBlockBitmap(bm *util.Bitmap, group int) error {
	if group >= len(fs.groupDescriptors.descriptors) {
		return fmt.Errorf("block group %d does not exist", group)
	}
	writableFile, err := fs.backend.Writable()
	if err != nil {
		return err
	}
	b := bm.ToBytes()
	gd := fs.groupDescriptors.descriptors[group]
	bitmapLocation := gd.blockBitmapLocation
	offset := int64(bitmapLocation*uint64(fs.superblock.blockSize) + uint64(fs.start))
	wrote, err := writableFile.WriteAt(b, offset)
	if err != nil {
		return fmt.Errorf("unable to write block bitmap for blockgroup %d: %w", gd.number, err)
	}
	if wrote != int(fs.superblock.blockSize) {
		return fmt.Errorf("wrote %d bytes instead of expected %d for block bitmap of block group %d", wrote, fs.superblock.blockSize, gd.number)
	}

	return nil
}

func (fs *FileSystem) writeSuperblock() error {
	writableFile, err := fs.backend.Writable()
	if err != nil {
		return err
	}
	superblockBytes, err := fs.superblock.toBytes()
	if err != nil {
		return fmt.Errorf("could not convert superblock to bytes: %v", err)
	}
	_, err = writableFile.WriteAt(superblockBytes, fs.start+int64(BootSectorSize))
	return err
}

func blockGroupForInode(inodeNumber int, inodesPerGroup uint32) int {
	return (inodeNumber - 1) / int(inodesPerGroup)
}
func blockGroupForBlock(blockNumber int, blocksPerGroup uint32) int {
	return (blockNumber - 1) / int(blocksPerGroup)
}
