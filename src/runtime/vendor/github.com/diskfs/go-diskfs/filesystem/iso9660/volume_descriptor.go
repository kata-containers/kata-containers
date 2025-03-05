package iso9660

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"strconv"
	"time"
)

type volumeDescriptorType uint8

const (
	volumeDescriptorBoot          volumeDescriptorType = 0x00
	volumeDescriptorPrimary       volumeDescriptorType = 0x01
	volumeDescriptorSupplementary volumeDescriptorType = 0x02
	volumeDescriptorPartition     volumeDescriptorType = 0x03
	volumeDescriptorTerminator    volumeDescriptorType = 0xff
)

const (
	isoIdentifier        uint64 = 0x4344303031 // string "CD001"
	isoVersion           uint8  = 0x01
	bootSystemIdentifier        = "EL TORITO SPECIFICATION"
)

// volumeDescriptor interface for any given type of volume descriptor
type volumeDescriptor interface {
	Type() volumeDescriptorType
	toBytes() []byte
	equal(volumeDescriptor) bool
}

type primaryVolumeDescriptor struct {
	systemIdentifier           string // length 32 bytes
	volumeIdentifier           string // length 32 bytes
	volumeSize                 uint32 // in blocks
	setSize                    uint16
	sequenceNumber             uint16
	blocksize                  uint16
	pathTableSize              uint32
	pathTableLLocation         uint32
	pathTableLOptionalLocation uint32
	pathTableMLocation         uint32
	pathTableMOptionalLocation uint32
	rootDirectoryEntry         *directoryEntry
	volumeSetIdentifier        string // 128 bytes
	publisherIdentifier        string // 128 bytes
	preparerIdentifier         string // 128 bytes
	applicationIdentifier      string // 128 bytes
	copyrightFile              string // 37 bytes
	abstractFile               string // 37 bytes
	bibliographicFile          string // 37 bytes
	creation                   time.Time
	modification               time.Time
	expiration                 time.Time
	effective                  time.Time
}

type bootVolumeDescriptor struct {
	location uint32 // length 1977 bytes; trailing 0x00 are stripped off
}
type terminatorVolumeDescriptor struct {
}

//nolint:structcheck // we accept some unused fields as useful for reference
type supplementaryVolumeDescriptor struct {
	volumeFlags                uint8
	systemIdentifier           string // length 32 bytes
	volumeIdentifier           string // length 32 bytes
	volumeSize                 uint64 // in bytes
	escapeSequences            []byte // 32 bytes
	setSize                    uint16
	sequenceNumber             uint16
	blocksize                  uint16
	pathTableSize              uint32
	pathTableLLocation         uint32
	pathTableLOptionalLocation uint32
	pathTableMLocation         uint32
	pathTableMOptionalLocation uint32
	rootDirectoryEntry         *directoryEntry
	volumeSetIdentifier        string // 128 bytes
	publisherIdentifier        string // 128 bytes
	preparerIdentifier         string // 128 bytes
	applicationIdentifier      string // 128 bytes
	copyrightFile              string // 37 bytes
	abstractFile               string // 37 bytes
	bibliographicFile          string // 37 bytes
	creation                   time.Time
	modification               time.Time
	expiration                 time.Time
	effective                  time.Time
}
type partitionVolumeDescriptor struct {
	data []byte // length 2048 bytes; trailing 0x00 are stripped off
}

type volumeDescriptors struct {
	descriptors []volumeDescriptor
	primary     *primaryVolumeDescriptor
}

func (v *volumeDescriptors) equal(a *volumeDescriptors) bool {
	if len(v.descriptors) != len(a.descriptors) {
		return false
	}
	// just convert everything to bytes and compare
	return bytes.Equal(v.toBytes(), a.toBytes())
}

func (v *volumeDescriptors) toBytes() []byte {
	b := make([]byte, 0, 20)
	for _, d := range v.descriptors {
		b = append(b, d.toBytes()...)
	}
	return b
}

// primaryVolumeDescriptor
func (v *primaryVolumeDescriptor) Type() volumeDescriptorType {
	return volumeDescriptorPrimary
}
func (v *primaryVolumeDescriptor) equal(a volumeDescriptor) bool {
	return bytes.Equal(v.toBytes(), a.toBytes())
}
func (v *primaryVolumeDescriptor) toBytes() []byte {
	b := volumeDescriptorFirstBytes(volumeDescriptorPrimary)

	copy(b[8:40], v.systemIdentifier)
	copy(b[40:72], v.volumeIdentifier)
	binary.LittleEndian.PutUint32(b[80:84], v.volumeSize)
	binary.BigEndian.PutUint32(b[84:88], v.volumeSize)
	binary.LittleEndian.PutUint16(b[120:122], v.setSize)
	binary.BigEndian.PutUint16(b[122:124], v.setSize)
	binary.LittleEndian.PutUint16(b[124:126], v.sequenceNumber)
	binary.BigEndian.PutUint16(b[126:128], v.sequenceNumber)
	binary.LittleEndian.PutUint16(b[128:130], v.blocksize)
	binary.BigEndian.PutUint16(b[130:132], v.blocksize)
	binary.LittleEndian.PutUint32(b[132:136], v.pathTableSize)
	binary.BigEndian.PutUint32(b[136:140], v.pathTableSize)
	binary.LittleEndian.PutUint32(b[140:144], v.pathTableLLocation)
	binary.LittleEndian.PutUint32(b[144:148], v.pathTableLOptionalLocation)
	binary.BigEndian.PutUint32(b[148:152], v.pathTableMLocation)
	binary.BigEndian.PutUint32(b[152:156], v.pathTableMOptionalLocation)

	rootDirEntry := make([]byte, 34)
	if v.rootDirectoryEntry != nil {
		// we will skip the extensions anyways, so the CE blocks do not matter
		rootDirEntrySlice, _ := v.rootDirectoryEntry.toBytes(true, []uint32{})
		rootDirEntry = rootDirEntrySlice[0]
	}
	copy(b[156:156+34], rootDirEntry)

	copy(b[190:190+128], v.volumeSetIdentifier)
	copy(b[318:318+128], v.publisherIdentifier)
	copy(b[446:446+128], v.preparerIdentifier)
	copy(b[574:574+128], v.applicationIdentifier)
	copy(b[702:702+37], v.copyrightFile)
	copy(b[739:739+37], v.abstractFile)
	copy(b[776:776+37], v.bibliographicFile)
	copy(b[813:813+17], timeToDecBytes(v.creation))
	copy(b[830:830+17], timeToDecBytes(v.modification))
	copy(b[847:847+17], timeToDecBytes(v.expiration))
	copy(b[864:864+17], timeToDecBytes(v.effective))

	// these two are set by the standard
	b[881] = 1
	b[882] = 0

	return b
}

// volumeDescriptorFromBytes create a volumeDescriptor struct from bytes
func volumeDescriptorFromBytes(b []byte) (volumeDescriptor, error) {
	if len(b) != int(volumeDescriptorSize) {
		return nil, fmt.Errorf("cannot read volume descriptor from bytes of length %d, must be %d", len(b), volumeDescriptorSize)
	}
	// validate the signature
	tmpb := make([]byte, 8)
	copy(tmpb[3:8], b[1:6])
	signature := binary.BigEndian.Uint64(tmpb)
	if signature != isoIdentifier {
		return nil, fmt.Errorf("mismatched ISO identifier in Volume Descriptor. Found %x expected %x", signature, isoIdentifier)
	}
	// validate the version
	version := b[6]
	if version != isoVersion {
		return nil, fmt.Errorf("mismatched ISO version in Volume Descriptor. Found %x expected %x", version, isoVersion)
	}
	// get the type and data - later we will be more intelligent about this and read actual primary volume info
	vdType := volumeDescriptorType(b[0])
	var vd volumeDescriptor
	var err error

	switch vdType {
	case volumeDescriptorPrimary:
		vd, err = parsePrimaryVolumeDescriptor(b)
		if err != nil {
			return nil, fmt.Errorf("unable to parse primary volume descriptor bytes: %v", err)
		}
	case volumeDescriptorBoot:
		vd, err = parseBootVolumeDescriptor(b)
		if err != nil {
			return nil, fmt.Errorf("unable to parse primary volume descriptor bytes: %v", err)
		}
	case volumeDescriptorTerminator:
		vd = &terminatorVolumeDescriptor{}
	case volumeDescriptorPartition:
		vd = &partitionVolumeDescriptor{
			data: b[8:volumeDescriptorSize],
		}
	case volumeDescriptorSupplementary:
		vd, err = parseSupplementaryVolumeDescriptor(b)
		if err != nil {
			return nil, fmt.Errorf("unable to parse primary volume descriptor bytes: %v", err)
		}
	default:
		return nil, fmt.Errorf("unknown volume descriptor type %d", vdType)
	}
	return vd, nil
}

func parsePrimaryVolumeDescriptor(b []byte) (*primaryVolumeDescriptor, error) {
	blocksize := binary.LittleEndian.Uint16(b[128:130])

	creation, err := decBytesToTime(b[813 : 813+17])
	if err != nil {
		return nil, fmt.Errorf("unable to convert creation date/time from bytes: %v", err)
	}
	modification, err := decBytesToTime(b[830 : 830+17])
	if err != nil {
		return nil, fmt.Errorf("unable to convert modification date/time from bytes: %v", err)
	}
	// expiration can be never
	nullBytes := []byte{48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 0}
	var expiration, effective time.Time
	expirationBytes := b[847 : 847+17]
	effectiveBytes := b[864 : 864+17]
	if !bytes.Equal(expirationBytes, nullBytes) {
		expiration, err = decBytesToTime(expirationBytes)
		if err != nil {
			return nil, fmt.Errorf("unable to convert expiration date/time from bytes: %v", err)
		}
	}
	if !bytes.Equal(effectiveBytes, nullBytes) {
		effective, err = decBytesToTime(effectiveBytes)
		if err != nil {
			return nil, fmt.Errorf("unable to convert effective date/time from bytes: %v", err)
		}
	}

	rootDirEntry, err := dirEntryFromBytes(b[156:156+34], nil)
	if err != nil {
		return nil, fmt.Errorf("unable to read root directory entry: %v", err)
	}

	return &primaryVolumeDescriptor{
		systemIdentifier:           string(b[8:40]),
		volumeIdentifier:           string(b[40:72]),
		volumeSize:                 binary.LittleEndian.Uint32(b[80:84]),
		setSize:                    binary.LittleEndian.Uint16(b[120:122]),
		sequenceNumber:             binary.LittleEndian.Uint16(b[124:126]),
		blocksize:                  blocksize,
		pathTableSize:              binary.LittleEndian.Uint32(b[132:136]),
		pathTableLLocation:         binary.LittleEndian.Uint32(b[140:144]),
		pathTableLOptionalLocation: binary.LittleEndian.Uint32(b[144:148]),
		pathTableMLocation:         binary.BigEndian.Uint32(b[148:152]),
		pathTableMOptionalLocation: binary.BigEndian.Uint32(b[152:156]),
		volumeSetIdentifier:        string(b[190 : 190+128]),
		publisherIdentifier:        string(b[318 : 318+128]),
		preparerIdentifier:         string(b[446 : 446+128]),
		applicationIdentifier:      string(b[574 : 574+128]),
		copyrightFile:              string(b[702 : 702+37]),
		abstractFile:               string(b[739 : 739+37]),
		bibliographicFile:          string(b[776 : 776+37]),
		creation:                   creation,
		modification:               modification,
		expiration:                 expiration,
		effective:                  effective,
		rootDirectoryEntry:         rootDirEntry,
	}, nil
}

// terminatorVolumeDescriptor
func (v *terminatorVolumeDescriptor) Type() volumeDescriptorType {
	return volumeDescriptorTerminator
}
func (v *terminatorVolumeDescriptor) equal(a volumeDescriptor) bool {
	return bytes.Equal(v.toBytes(), a.toBytes())
}
func (v *terminatorVolumeDescriptor) toBytes() []byte {
	b := volumeDescriptorFirstBytes(volumeDescriptorTerminator)
	return b
}

// bootVolumeDescriptor
func (v *bootVolumeDescriptor) Type() volumeDescriptorType {
	return volumeDescriptorBoot
}
func (v *bootVolumeDescriptor) equal(a volumeDescriptor) bool {
	return bytes.Equal(v.toBytes(), a.toBytes())
}
func (v *bootVolumeDescriptor) toBytes() []byte {
	b := volumeDescriptorFirstBytes(volumeDescriptorBoot)
	copy(b[7:39], bootSystemIdentifier)
	binary.LittleEndian.PutUint32(b[0x47:0x4b], v.location)

	return b
}

// parseBootVolumeDescriptor
func parseBootVolumeDescriptor(b []byte) (*bootVolumeDescriptor, error) {
	systemIdentifier := string(b[0x7 : 0x7+len(bootSystemIdentifier)])
	if systemIdentifier != bootSystemIdentifier {
		return nil, fmt.Errorf("incorrect specification, actual '%s' expected '%s'", systemIdentifier, bootSystemIdentifier)
	}
	location := binary.LittleEndian.Uint32(b[0x47:0x4b])
	return &bootVolumeDescriptor{location: location}, nil
}

// supplementaryVolumeDescriptor
func parseSupplementaryVolumeDescriptor(b []byte) (*supplementaryVolumeDescriptor, error) {
	blocksize := binary.LittleEndian.Uint16(b[128:130])
	volumesize := binary.LittleEndian.Uint32(b[80:84])
	volumesizeBytes := uint64(blocksize) * uint64(volumesize)

	creation, err := decBytesToTime(b[813 : 813+17])
	if err != nil {
		return nil, fmt.Errorf("unable to convert creation date/time from bytes: %v", err)
	}
	modification, err := decBytesToTime(b[830 : 830+17])
	if err != nil {
		return nil, fmt.Errorf("unable to convert modification date/time from bytes: %v", err)
	}
	// expiration can be never
	nullBytes := []byte{48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 0}
	var expiration, effective time.Time
	expirationBytes := b[847 : 847+17]
	effectiveBytes := b[864 : 864+17]
	if !bytes.Equal(expirationBytes, nullBytes) {
		expiration, err = decBytesToTime(expirationBytes)
		if err != nil {
			return nil, fmt.Errorf("unable to convert expiration date/time from bytes: %v", err)
		}
	}
	if !bytes.Equal(effectiveBytes, nullBytes) {
		effective, err = decBytesToTime(effectiveBytes)
		if err != nil {
			return nil, fmt.Errorf("unable to convert effective date/time from bytes: %v", err)
		}
	}

	// no susp extensions for the dir entry in the volume descriptor
	rootDirEntry, err := dirEntryFromBytes(b[156:156+34], nil)
	if err != nil {
		return nil, fmt.Errorf("unable to read root directory entry: %v", err)
	}

	return &supplementaryVolumeDescriptor{
		systemIdentifier:           string(b[8:40]),
		volumeIdentifier:           string(b[40:72]),
		volumeSize:                 volumesizeBytes,
		setSize:                    binary.LittleEndian.Uint16(b[120:122]),
		sequenceNumber:             binary.LittleEndian.Uint16(b[124:126]),
		blocksize:                  blocksize,
		pathTableSize:              binary.LittleEndian.Uint32(b[132:136]),
		pathTableLLocation:         binary.LittleEndian.Uint32(b[140:144]),
		pathTableLOptionalLocation: binary.LittleEndian.Uint32(b[144:148]),
		pathTableMLocation:         binary.BigEndian.Uint32(b[148:152]),
		pathTableMOptionalLocation: binary.BigEndian.Uint32(b[152:156]),
		volumeSetIdentifier:        bytesToUCS2String(b[190 : 190+128]),
		publisherIdentifier:        bytesToUCS2String(b[318 : 318+128]),
		preparerIdentifier:         bytesToUCS2String(b[446 : 446+128]),
		applicationIdentifier:      bytesToUCS2String(b[574 : 574+128]),
		copyrightFile:              bytesToUCS2String(b[702 : 702+37]),
		abstractFile:               bytesToUCS2String(b[739 : 739+37]),
		bibliographicFile:          bytesToUCS2String(b[776 : 776+37]),
		creation:                   creation,
		modification:               modification,
		expiration:                 expiration,
		effective:                  effective,
		rootDirectoryEntry:         rootDirEntry,
	}, nil
}
func (v *supplementaryVolumeDescriptor) Type() volumeDescriptorType {
	return volumeDescriptorSupplementary
}
func (v *supplementaryVolumeDescriptor) equal(a volumeDescriptor) bool {
	return bytes.Equal(v.toBytes(), a.toBytes())
}
func (v *supplementaryVolumeDescriptor) toBytes() []byte {
	b := volumeDescriptorFirstBytes(volumeDescriptorSupplementary)

	copy(b[8:40], v.systemIdentifier)
	copy(b[40:72], v.volumeIdentifier)
	blockcount := uint32(v.volumeSize / uint64(v.blocksize))
	binary.LittleEndian.PutUint32(b[80:84], blockcount)
	binary.BigEndian.PutUint32(b[84:88], blockcount)
	binary.LittleEndian.PutUint16(b[120:122], v.setSize)
	binary.BigEndian.PutUint16(b[122:124], v.setSize)
	binary.LittleEndian.PutUint16(b[124:126], v.sequenceNumber)
	binary.BigEndian.PutUint16(b[126:128], v.sequenceNumber)
	binary.LittleEndian.PutUint16(b[128:130], v.blocksize)
	binary.BigEndian.PutUint16(b[130:132], v.blocksize)
	binary.LittleEndian.PutUint32(b[132:136], v.pathTableSize)
	binary.BigEndian.PutUint32(b[136:140], v.pathTableSize)
	binary.LittleEndian.PutUint32(b[140:144], v.pathTableLLocation)
	binary.LittleEndian.PutUint32(b[144:148], v.pathTableLOptionalLocation)
	binary.BigEndian.PutUint32(b[148:152], v.pathTableMLocation)
	binary.BigEndian.PutUint32(b[152:156], v.pathTableMOptionalLocation)

	rootDirEntry := make([]byte, 34)
	if v.rootDirectoryEntry != nil {
		// we will skip the extensions anyways, so the CE blocks do not matter
		rootDirEntrySlice, _ := v.rootDirectoryEntry.toBytes(true, []uint32{})
		rootDirEntry = rootDirEntrySlice[0]
	}
	copy(b[156:156+34], rootDirEntry)

	copy(b[190:190+128], ucs2StringToBytes(v.volumeSetIdentifier))
	copy(b[318:318+128], ucs2StringToBytes(v.publisherIdentifier))
	copy(b[446:446+128], ucs2StringToBytes(v.preparerIdentifier))
	copy(b[574:574+128], ucs2StringToBytes(v.applicationIdentifier))
	copy(b[702:702+37], ucs2StringToBytes(v.copyrightFile))
	copy(b[739:739+37], ucs2StringToBytes(v.abstractFile))
	copy(b[776:776+37], ucs2StringToBytes(v.bibliographicFile))
	copy(b[813:813+17], timeToDecBytes(v.creation))
	copy(b[830:830+17], timeToDecBytes(v.modification))
	copy(b[847:847+17], timeToDecBytes(v.expiration))
	copy(b[864:864+17], timeToDecBytes(v.effective))

	return b
}

// partitionVolumeDescriptor
func (v *partitionVolumeDescriptor) Type() volumeDescriptorType {
	return volumeDescriptorPartition
}
func (v *partitionVolumeDescriptor) equal(a volumeDescriptor) bool {
	return bytes.Equal(v.toBytes(), a.toBytes())
}
func (v *partitionVolumeDescriptor) toBytes() []byte {
	b := volumeDescriptorFirstBytes(volumeDescriptorPartition)
	copy(b[7:], v.data)
	return b
}

// utilities
func volumeDescriptorFirstBytes(t volumeDescriptorType) []byte {
	b := make([]byte, volumeDescriptorSize)

	b[0] = byte(t)
	tmpb := make([]byte, 8)
	binary.BigEndian.PutUint64(tmpb, isoIdentifier)
	copy(b[1:6], tmpb[3:8])
	b[6] = isoVersion
	return b
}

func decBytesToTime(b []byte) (time.Time, error) {
	year := string(b[0:4])
	month := string(b[4:6])
	date := string(b[6:8])
	hour := string(b[8:10])
	minute := string(b[10:12])
	second := string(b[12:14])
	csec := string(b[14:16])
	offset := int(int8(b[16]))
	location := offset * 15
	format := "2006-01-02T15:04:05-07:00"
	offsetHr := location / 60
	offsetMin := location % 60
	offsetString := ""
	// if negative offset, show it just on the hour part, not twice, so we end up with "-06:30" and not "-06:-30"
	switch {
	case offset == 0:
		offsetString = "+00:00"
	case offset < 0:
		offsetString = fmt.Sprintf("-%02d:%02d", -offsetHr, -offsetMin)
	case offset > 0:
		offsetString = fmt.Sprintf("+%02d:%02d", offsetHr, offsetMin)
	}
	return time.Parse(format, fmt.Sprintf("%s-%s-%sT%s:%s:%s.%s%s", year, month, date, hour, minute, second, csec, offsetString))
}
func timeToDecBytes(t time.Time) []byte {
	year := strconv.Itoa(t.Year())
	month := strconv.Itoa(int(t.Month()))
	date := strconv.Itoa(t.Day())
	hour := strconv.Itoa(t.Hour())
	minute := strconv.Itoa(t.Minute())
	second := strconv.Itoa(t.Second())
	csec := strconv.Itoa(t.Nanosecond() / 1e+7)
	_, offset := t.Zone()
	b := make([]byte, 17)
	copy(b[0:4], fmt.Sprintf("%04s", year))
	copy(b[4:6], fmt.Sprintf("%02s", month))
	copy(b[6:8], fmt.Sprintf("%02s", date))
	copy(b[8:10], fmt.Sprintf("%02s", hour))
	copy(b[10:12], fmt.Sprintf("%02s", minute))
	copy(b[12:14], fmt.Sprintf("%02s", second))
	copy(b[14:16], fmt.Sprintf("%02s", csec))
	b[16] = byte(offset / 60 / 15)
	return b
}
