package ext4

// features are defined
// beginning at https://git.kernel.org/pub/scm/fs/ext2/e2fsprogs.git/tree/lib/ext2fs/ext2_fs.h#n820

// featureFlags is a structure holding which flags are set - compatible, incompatible and read-only compatible
type featureFlags struct {
	// compatible, incompatible, and compatibleReadOnly feature flags
	directoryPreAllocate             bool
	imagicInodes                     bool
	hasJournal                       bool
	extendedAttributes               bool
	reservedGDTBlocksForExpansion    bool
	directoryIndices                 bool
	lazyBlockGroup                   bool
	excludeInode                     bool
	excludeBitmap                    bool
	sparseSuperBlockV2               bool
	fastCommit                       bool
	stableInodes                     bool
	orphanFile                       bool
	compression                      bool
	directoryEntriesRecordFileType   bool
	recoveryNeeded                   bool
	separateJournalDevice            bool
	metaBlockGroups                  bool
	extents                          bool
	fs64Bit                          bool
	multipleMountProtection          bool
	flexBlockGroups                  bool
	extendedAttributeInodes          bool
	dataInDirectoryEntries           bool
	metadataChecksumSeedInSuperblock bool
	largeDirectory                   bool
	dataInInode                      bool
	encryptInodes                    bool
	sparseSuperblock                 bool
	largeFile                        bool
	btreeDirectory                   bool
	hugeFile                         bool
	gdtChecksum                      bool
	largeSubdirectoryCount           bool
	largeInodes                      bool
	snapshot                         bool
	quota                            bool
	bigalloc                         bool
	metadataChecksums                bool
	replicas                         bool
	readOnly                         bool
	projectQuotas                    bool
}

func parseFeatureFlags(compatFlags, incompatFlags, roCompatFlags uint32) featureFlags {
	f := featureFlags{
		directoryPreAllocate:             compatFeatureDirectoryPreAllocate.included(compatFlags),
		imagicInodes:                     compatFeatureImagicInodes.included(compatFlags),
		hasJournal:                       compatFeatureHasJournal.included(compatFlags),
		extendedAttributes:               compatFeatureExtendedAttributes.included(compatFlags),
		reservedGDTBlocksForExpansion:    compatFeatureReservedGDTBlocksForExpansion.included(compatFlags),
		directoryIndices:                 compatFeatureDirectoryIndices.included(compatFlags),
		lazyBlockGroup:                   compatFeatureLazyBlockGroup.included(compatFlags),
		excludeInode:                     compatFeatureExcludeInode.included(compatFlags),
		excludeBitmap:                    compatFeatureExcludeBitmap.included(compatFlags),
		sparseSuperBlockV2:               compatFeatureSparseSuperBlockV2.included(compatFlags),
		fastCommit:                       compatFeatureFastCommit.included(compatFlags),
		stableInodes:                     compatFeatureStableInodes.included(compatFlags),
		orphanFile:                       compatFeatureOrphanFile.included(compatFlags),
		compression:                      incompatFeatureCompression.included(incompatFlags),
		directoryEntriesRecordFileType:   incompatFeatureDirectoryEntriesRecordFileType.included(incompatFlags),
		recoveryNeeded:                   incompatFeatureRecoveryNeeded.included(incompatFlags),
		separateJournalDevice:            incompatFeatureSeparateJournalDevice.included(incompatFlags),
		metaBlockGroups:                  incompatFeatureMetaBlockGroups.included(incompatFlags),
		extents:                          incompatFeatureExtents.included(incompatFlags),
		fs64Bit:                          incompatFeature64Bit.included(incompatFlags),
		multipleMountProtection:          incompatFeatureMultipleMountProtection.included(incompatFlags),
		flexBlockGroups:                  incompatFeatureFlexBlockGroups.included(incompatFlags),
		extendedAttributeInodes:          incompatFeatureExtendedAttributeInodes.included(incompatFlags),
		dataInDirectoryEntries:           incompatFeatureDataInDirectoryEntries.included(incompatFlags),
		metadataChecksumSeedInSuperblock: incompatFeatureMetadataChecksumSeedInSuperblock.included(incompatFlags),
		largeDirectory:                   incompatFeatureLargeDirectory.included(incompatFlags),
		dataInInode:                      incompatFeatureDataInInode.included(incompatFlags),
		encryptInodes:                    incompatFeatureEncryptInodes.included(incompatFlags),
		sparseSuperblock:                 roCompatFeatureSparseSuperblock.included(roCompatFlags),
		largeFile:                        roCompatFeatureLargeFile.included(roCompatFlags),
		btreeDirectory:                   roCompatFeatureBtreeDirectory.included(roCompatFlags),
		hugeFile:                         roCompatFeatureHugeFile.included(roCompatFlags),
		gdtChecksum:                      roCompatFeatureGDTChecksum.included(roCompatFlags),
		largeSubdirectoryCount:           roCompatFeatureLargeSubdirectoryCount.included(roCompatFlags),
		largeInodes:                      roCompatFeatureLargeInodes.included(roCompatFlags),
		snapshot:                         roCompatFeatureSnapshot.included(roCompatFlags),
		quota:                            roCompatFeatureQuota.included(roCompatFlags),
		bigalloc:                         roCompatFeatureBigalloc.included(roCompatFlags),
		metadataChecksums:                roCompatFeatureMetadataChecksums.included(roCompatFlags),
		replicas:                         roCompatFeatureReplicas.included(roCompatFlags),
		readOnly:                         roCompatFeatureReadOnly.included(roCompatFlags),
		projectQuotas:                    roCompatFeatureProjectQuotas.included(roCompatFlags),
	}

	return f
}

//nolint:gocyclo // we know this has cyclomatic complexity, but not worth breaking apart
func (f *featureFlags) toInts() (compatFlags, incompatFlags, roCompatFlags uint32) {
	// compatible flags
	if f.directoryPreAllocate {
		compatFlags |= uint32(compatFeatureDirectoryPreAllocate)
	}
	if f.imagicInodes {
		compatFlags |= uint32(compatFeatureImagicInodes)
	}
	if f.hasJournal {
		compatFlags |= uint32(compatFeatureHasJournal)
	}
	if f.extendedAttributes {
		compatFlags |= uint32(compatFeatureExtendedAttributes)
	}
	if f.reservedGDTBlocksForExpansion {
		compatFlags |= uint32(compatFeatureReservedGDTBlocksForExpansion)
	}
	if f.directoryIndices {
		compatFlags |= uint32(compatFeatureDirectoryIndices)
	}
	if f.lazyBlockGroup {
		compatFlags |= uint32(compatFeatureLazyBlockGroup)
	}
	if f.excludeInode {
		compatFlags |= uint32(compatFeatureExcludeInode)
	}
	if f.excludeBitmap {
		compatFlags |= uint32(compatFeatureExcludeBitmap)
	}
	if f.sparseSuperBlockV2 {
		compatFlags |= uint32(compatFeatureSparseSuperBlockV2)
	}
	if f.fastCommit {
		compatFlags |= uint32(compatFeatureFastCommit)
	}
	if f.stableInodes {
		compatFlags |= uint32(compatFeatureStableInodes)
	}
	if f.orphanFile {
		compatFlags |= uint32(compatFeatureOrphanFile)
	}

	// incompatible flags
	if f.compression {
		incompatFlags |= uint32(incompatFeatureCompression)
	}
	if f.directoryEntriesRecordFileType {
		incompatFlags |= uint32(incompatFeatureDirectoryEntriesRecordFileType)
	}
	if f.recoveryNeeded {
		incompatFlags |= uint32(incompatFeatureRecoveryNeeded)
	}
	if f.separateJournalDevice {
		incompatFlags |= uint32(incompatFeatureSeparateJournalDevice)
	}
	if f.metaBlockGroups {
		incompatFlags |= uint32(incompatFeatureMetaBlockGroups)
	}
	if f.extents {
		incompatFlags |= uint32(incompatFeatureExtents)
	}
	if f.fs64Bit {
		incompatFlags |= uint32(incompatFeature64Bit)
	}
	if f.multipleMountProtection {
		incompatFlags |= uint32(incompatFeatureMultipleMountProtection)
	}
	if f.flexBlockGroups {
		incompatFlags |= uint32(incompatFeatureFlexBlockGroups)
	}
	if f.extendedAttributeInodes {
		incompatFlags |= uint32(incompatFeatureExtendedAttributeInodes)
	}
	if f.dataInDirectoryEntries {
		incompatFlags |= uint32(incompatFeatureDataInDirectoryEntries)
	}
	if f.metadataChecksumSeedInSuperblock {
		incompatFlags |= uint32(incompatFeatureMetadataChecksumSeedInSuperblock)
	}
	if f.largeDirectory {
		incompatFlags |= uint32(incompatFeatureLargeDirectory)
	}
	if f.dataInInode {
		incompatFlags |= uint32(incompatFeatureDataInInode)
	}
	if f.encryptInodes {
		incompatFlags |= uint32(incompatFeatureEncryptInodes)
	}

	// read only compatible flags
	if f.sparseSuperblock {
		roCompatFlags |= uint32(roCompatFeatureSparseSuperblock)
	}
	if f.largeFile {
		roCompatFlags |= uint32(roCompatFeatureLargeFile)
	}
	if f.btreeDirectory {
		roCompatFlags |= uint32(roCompatFeatureBtreeDirectory)
	}
	if f.hugeFile {
		roCompatFlags |= uint32(roCompatFeatureHugeFile)
	}
	if f.gdtChecksum {
		roCompatFlags |= uint32(roCompatFeatureGDTChecksum)
	}
	if f.largeSubdirectoryCount {
		roCompatFlags |= uint32(roCompatFeatureLargeSubdirectoryCount)
	}
	if f.largeInodes {
		roCompatFlags |= uint32(roCompatFeatureLargeInodes)
	}
	if f.snapshot {
		roCompatFlags |= uint32(roCompatFeatureSnapshot)
	}
	if f.quota {
		roCompatFlags |= uint32(roCompatFeatureQuota)
	}
	if f.bigalloc {
		roCompatFlags |= uint32(roCompatFeatureBigalloc)
	}
	if f.metadataChecksums {
		roCompatFlags |= uint32(roCompatFeatureMetadataChecksums)
	}
	if f.replicas {
		roCompatFlags |= uint32(roCompatFeatureReplicas)
	}
	if f.readOnly {
		roCompatFlags |= uint32(roCompatFeatureReadOnly)
	}
	if f.projectQuotas {
		roCompatFlags |= uint32(roCompatFeatureProjectQuotas)
	}

	return compatFlags, incompatFlags, roCompatFlags
}

// default features
/*
	base_features = sparse_super,large_file,filetype,resize_inode,dir_index,ext_attr
	features = has_journal,extent,huge_file,flex_bg,uninit_bg,64bit,dir_nlink,extra_isize
*/
var defaultFeatureFlags = featureFlags{
	largeFile:          true,
	hugeFile:           true,
	sparseSuperblock:   true,
	flexBlockGroups:    true,
	hasJournal:         true,
	extents:            true,
	fs64Bit:            true,
	extendedAttributes: true,
}

type FeatureOpt func(*featureFlags)

func WithFeatureDirectoryPreAllocate(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.directoryPreAllocate = enable
	}
}
func WithFeatureImagicInodes(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.imagicInodes = enable
	}
}
func WithFeatureHasJournal(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.hasJournal = enable
	}
}
func WithFeatureExtendedAttributes(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.extendedAttributes = enable
	}
}
func WithFeatureReservedGDTBlocksForExpansion(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.reservedGDTBlocksForExpansion = enable
	}
}
func WithFeatureDirectoryIndices(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.directoryIndices = enable
	}
}
func WithFeatureLazyBlockGroup(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.lazyBlockGroup = enable
	}
}
func WithFeatureExcludeInode(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.excludeInode = enable
	}
}
func WithFeatureExcludeBitmap(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.excludeBitmap = enable
	}
}
func WithFeatureSparseSuperBlockV2(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.sparseSuperBlockV2 = enable
	}
}
func WithFeatureCompression(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.compression = enable
	}
}
func WithFeatureDirectoryEntriesRecordFileType(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.directoryEntriesRecordFileType = enable
	}
}
func WithFeatureRecoveryNeeded(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.recoveryNeeded = enable
	}
}
func WithFeatureSeparateJournalDevice(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.separateJournalDevice = enable
	}
}
func WithFeatureMetaBlockGroups(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.metaBlockGroups = enable
	}
}
func WithFeatureExtents(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.extents = enable
	}
}
func WithFeatureFS64Bit(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.fs64Bit = enable
	}
}
func WithFeatureMultipleMountProtection(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.multipleMountProtection = enable
	}
}
func WithFeatureFlexBlockGroups(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.flexBlockGroups = enable
	}
}
func WithFeatureExtendedAttributeInodes(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.extendedAttributeInodes = enable
	}
}
func WithFeatureDataInDirectoryEntries(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.dataInDirectoryEntries = enable
	}
}
func WithFeatureMetadataChecksumSeedInSuperblock(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.metadataChecksumSeedInSuperblock = enable
	}
}
func WithFeatureLargeDirectory(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.largeDirectory = enable
	}
}
func WithFeatureDataInInode(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.dataInInode = enable
	}
}
func WithFeatureEncryptInodes(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.encryptInodes = enable
	}
}
func WithFeatureSparseSuperblock(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.sparseSuperblock = enable
	}
}
func WithFeatureLargeFile(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.largeFile = enable
	}
}
func WithFeatureBTreeDirectory(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.btreeDirectory = enable
	}
}
func WithFeatureHugeFile(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.hugeFile = enable
	}
}
func WithFeatureGDTChecksum(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.gdtChecksum = enable
	}
}
func WithFeatureLargeSubdirectoryCount(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.largeSubdirectoryCount = enable
	}
}
func WithFeatureLargeInodes(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.largeInodes = enable
	}
}
func WithFeatureSnapshot(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.snapshot = enable
	}
}
func WithFeatureQuota(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.quota = enable
	}
}
func WithFeatureBigalloc(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.bigalloc = enable
	}
}
func WithFeatureMetadataChecksums(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.metadataChecksums = enable
	}
}
func WithFeatureReplicas(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.replicas = enable
	}
}
func WithFeatureReadOnly(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.readOnly = enable
	}
}
func WithFeatureProjectQuotas(enable bool) FeatureOpt {
	return func(o *featureFlags) {
		o.projectQuotas = enable
	}
}
