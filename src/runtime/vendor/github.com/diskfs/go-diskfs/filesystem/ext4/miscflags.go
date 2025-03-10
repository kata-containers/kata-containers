package ext4

// miscFlags is a structure holding various miscellaneous flags
type miscFlags struct {
	signedDirectoryHash   bool
	unsignedDirectoryHash bool
	developmentTest       bool
}

func parseMiscFlags(flags uint32) miscFlags {
	m := miscFlags{
		signedDirectoryHash:   flagSignedDirectoryHash.included(flags),
		unsignedDirectoryHash: flagUnsignedDirectoryHash.included(flags),
		developmentTest:       flagTestDevCode.included(flags),
	}
	return m
}

func (m *miscFlags) toInt() uint32 {
	var flags uint32

	if m.signedDirectoryHash {
		flags |= uint32(flagSignedDirectoryHash)
	}
	if m.unsignedDirectoryHash {
		flags |= uint32(flagUnsignedDirectoryHash)
	}
	if m.developmentTest {
		flags |= uint32(flagTestDevCode)
	}
	return flags
}

var defaultMiscFlags = miscFlags{}
