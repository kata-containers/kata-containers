package ext4

const (
	// default mount options
	mountPrintDebugInfo                 mountOption = 0x1
	mountNewFilesGIDContainingDirectory mountOption = 0x2
	mountUserspaceExtendedAttributes    mountOption = 0x4
	mountPosixACLs                      mountOption = 0x8
	mount16BitUIDs                      mountOption = 0x10
	mountJournalDataAndMetadata         mountOption = 0x20
	mountFlushBeforeJournal             mountOption = 0x40
	mountUnorderingDataMetadata         mountOption = 0x60
	mountDisableWriteFlushes            mountOption = 0x100
	mountTrackMetadataBlocks            mountOption = 0x200
	mountDiscardDeviceSupport           mountOption = 0x400
	mountDisableDelayedAllocation       mountOption = 0x800
)

// mountOptions is a structure holding which default mount options are set
type mountOptions struct {
	printDebugInfo                 bool
	newFilesGIDContainingDirectory bool
	userspaceExtendedAttributes    bool
	posixACLs                      bool
	use16BitUIDs                   bool
	journalDataAndMetadata         bool
	flushBeforeJournal             bool
	unorderingDataMetadata         bool
	disableWriteFlushes            bool
	trackMetadataBlocks            bool
	discardDeviceSupport           bool
	disableDelayedAllocation       bool
}

type mountOption uint32

func (m mountOption) included(a uint32) bool {
	return a&uint32(m) == uint32(m)
}

type MountOpt func(*mountOptions)

func WithDefaultMountOptionPrintDebuggingInfo(enable bool) MountOpt {
	return func(o *mountOptions) {
		o.printDebugInfo = enable
	}
}

func WithDefaultMountOptionGIDFromDirectory(enable bool) MountOpt {
	return func(o *mountOptions) {
		o.newFilesGIDContainingDirectory = enable
	}
}

func WithDefaultMountOptionUserspaceXattrs(enable bool) MountOpt {
	return func(o *mountOptions) {
		o.userspaceExtendedAttributes = enable
	}
}

func WithDefaultMountOptionPOSIXACLs(enable bool) MountOpt {
	return func(o *mountOptions) {
		o.posixACLs = enable
	}
}

func WithDefaultMountOptionUID16Bit(enable bool) MountOpt {
	return func(o *mountOptions) {
		o.use16BitUIDs = enable
	}
}

func WithDefaultMountOptionJournalModeData(enable bool) MountOpt {
	return func(o *mountOptions) {
		o.journalDataAndMetadata = enable
	}
}

func WithDefaultMountOptionJournalModeOrdered(enable bool) MountOpt {
	return func(o *mountOptions) {
		o.flushBeforeJournal = enable
	}
}

func WithDefaultMountOptionJournalModeWriteback(enable bool) MountOpt {
	return func(o *mountOptions) {
		o.unorderingDataMetadata = enable
	}
}

func WithDefaultMountOptionDisableWriteFlushes(enable bool) MountOpt {
	return func(o *mountOptions) {
		o.disableWriteFlushes = enable
	}
}

func WithDefaultMountOptionBlockValidity(enable bool) MountOpt {
	return func(o *mountOptions) {
		o.trackMetadataBlocks = enable
	}
}

func WithDefaultMountOptionDiscardSupport(enable bool) MountOpt {
	return func(o *mountOptions) {
		o.discardDeviceSupport = enable
	}
}

func WithDefaultMountOptionDisableDelayedAllocation(enable bool) MountOpt {
	return func(o *mountOptions) {
		o.disableDelayedAllocation = enable
	}
}

func defaultMountOptionsFromOpts(opts []MountOpt) *mountOptions {
	o := &mountOptions{}
	for _, opt := range opts {
		opt(o)
	}
	return o
}

func parseMountOptions(flags uint32) mountOptions {
	m := mountOptions{
		printDebugInfo:                 mountPrintDebugInfo.included(flags),
		newFilesGIDContainingDirectory: mountNewFilesGIDContainingDirectory.included(flags),
		userspaceExtendedAttributes:    mountUserspaceExtendedAttributes.included(flags),
		posixACLs:                      mountPosixACLs.included(flags),
		use16BitUIDs:                   mount16BitUIDs.included(flags),
		journalDataAndMetadata:         mountJournalDataAndMetadata.included(flags),
		flushBeforeJournal:             mountFlushBeforeJournal.included(flags),
		unorderingDataMetadata:         mountUnorderingDataMetadata.included(flags),
		disableWriteFlushes:            mountDisableWriteFlushes.included(flags),
		trackMetadataBlocks:            mountTrackMetadataBlocks.included(flags),
		discardDeviceSupport:           mountDiscardDeviceSupport.included(flags),
		disableDelayedAllocation:       mountDisableDelayedAllocation.included(flags),
	}
	return m
}

func (m *mountOptions) toInt() uint32 {
	var flags uint32

	if m.printDebugInfo {
		flags |= uint32(mountPrintDebugInfo)
	}
	if m.newFilesGIDContainingDirectory {
		flags |= uint32(mountNewFilesGIDContainingDirectory)
	}
	if m.userspaceExtendedAttributes {
		flags |= uint32(mountUserspaceExtendedAttributes)
	}
	if m.posixACLs {
		flags |= uint32(mountPosixACLs)
	}
	if m.use16BitUIDs {
		flags |= uint32(mount16BitUIDs)
	}
	if m.journalDataAndMetadata {
		flags |= uint32(mountJournalDataAndMetadata)
	}
	if m.flushBeforeJournal {
		flags |= uint32(mountFlushBeforeJournal)
	}
	if m.unorderingDataMetadata {
		flags |= uint32(mountUnorderingDataMetadata)
	}
	if m.disableWriteFlushes {
		flags |= uint32(mountDisableWriteFlushes)
	}
	if m.trackMetadataBlocks {
		flags |= uint32(mountTrackMetadataBlocks)
	}
	if m.discardDeviceSupport {
		flags |= uint32(mountDiscardDeviceSupport)
	}
	if m.disableDelayedAllocation {
		flags |= uint32(mountDisableDelayedAllocation)
	}

	return flags
}
