package fat32

import (
	"encoding/binary"
	"fmt"
	"regexp"
	"strings"
	"time"

	"github.com/elliotwutingfeng/asciiset"
)

// AccessRights is the byte mask representing access rights to a FAT file
type accessRights uint16

// AccessRightsUnlimited represents unrestricted access
const (
	accessRightsUnlimited accessRights = 0x0000
	charsPerSlot          int          = 13
)

// valid shortname characters - [A-F][0-9][$%'-_@~`!(){}^#&]
var validShortNameCharacters, _ = asciiset.MakeASCIISet("!#$%&'()-0123456789@ABCDEFGHIJKLMNOPQRSTUVWXYZ^_`{}~")

// directoryEntry is a single directory entry
//
//nolint:structcheck // we are willing to leave unused elements here so that we can know their reference
type directoryEntry struct {
	filenameShort      string
	fileExtension      string
	filenameLong       string
	isReadOnly         bool
	isHidden           bool
	isSystem           bool
	isVolumeLabel      bool
	isSubdirectory     bool
	isArchiveDirty     bool
	isDevice           bool
	lowercaseShortname bool
	lowercaseExtension bool
	createTime         time.Time
	modifyTime         time.Time
	accessTime         time.Time
	acccessRights      accessRights
	clusterLocation    uint32
	fileSize           uint32
	filesystem         *FileSystem
	longFilenameSlots  int
	isNew              bool
}

func (de *directoryEntry) toBytes() ([]byte, error) {
	b := make([]byte, 0, bytesPerSlot)

	// do we have a long filename?
	if de.filenameLong != "" {
		lfnBytes, err := longFilenameBytes(de.filenameLong, de.filenameShort, de.fileExtension)
		if err != nil {
			return nil, fmt.Errorf("could not convert long filename to directory entries: %v", err)
		}
		b = append(b, lfnBytes...)
	}

	// this is for the regular 8.3 entry
	dosBytes := make([]byte, bytesPerSlot)
	createDate, createTime := timeToDateTime(de.createTime)
	modifyDate, modifyTime := timeToDateTime(de.modifyTime)
	accessDate, _ := timeToDateTime(de.accessTime)
	binary.LittleEndian.PutUint16(dosBytes[14:16], createTime)
	binary.LittleEndian.PutUint16(dosBytes[16:18], createDate)
	binary.LittleEndian.PutUint16(dosBytes[18:20], accessDate)
	binary.LittleEndian.PutUint16(dosBytes[22:24], modifyTime)
	binary.LittleEndian.PutUint16(dosBytes[24:26], modifyDate)
	// convert the short filename and extension to ascii bytes
	shortName, err := stringToASCIIBytes(fmt.Sprintf("% -8s", de.filenameShort))
	if err != nil {
		return nil, fmt.Errorf("error converting short filename to bytes: %v", err)
	}
	// convert the short filename and extension to ascii bytes
	extension, err := stringToASCIIBytes(fmt.Sprintf("% -3s", de.fileExtension))
	if err != nil {
		return nil, fmt.Errorf("error converting file extension to bytes: %v", err)
	}
	copy(dosBytes[0:8], shortName)
	copy(dosBytes[8:11], extension)
	binary.LittleEndian.PutUint32(dosBytes[28:32], de.fileSize)
	clusterLocation := make([]byte, 4)
	binary.LittleEndian.PutUint32(clusterLocation, de.clusterLocation)
	dosBytes[26] = clusterLocation[0]
	dosBytes[27] = clusterLocation[1]
	dosBytes[20] = clusterLocation[2]
	dosBytes[21] = clusterLocation[3]

	// set the flags
	if de.isVolumeLabel {
		dosBytes[11] |= 0x08
	}
	if de.isSubdirectory {
		dosBytes[11] |= 0x10
	}
	if de.isArchiveDirty {
		dosBytes[11] |= 0x20
	}

	if de.lowercaseExtension {
		dosBytes[12] |= 0x04
	}
	if de.lowercaseShortname {
		dosBytes[12] |= 0x08
	}

	b = append(b, dosBytes...)

	return b, nil
}

// parseDirEntries takes all of the bytes in a special file (i.e. a directory)
// and gets all of the DirectoryEntry for that directory
// this is, essentially, the equivalent of `ls -l` or if you prefer `dir`
func parseDirEntries(b []byte) ([]*directoryEntry, error) {
	dirEntries := make([]*directoryEntry, 0, 20)
	// parse the data into Fat32DirectoryEntry
	lfn := ""
	// this should be used to count the LFN entries and that they make sense
	//     lfnCount := 0
byteLoop:
	for i := 0; i < len(b); i += 32 {
		// is this the beginning of all empty entries?
		switch b[i+0] {
		case 0:
			// need to break "byteLoop" else break will break the switches
			break byteLoop
		case 0xe5:
			continue
		}
		// is this an LFN entry?
		if b[i+11] == 0x0f {
			// check if this is the last logical / first physical and how many there are
			if b[i]&0x40 == 0x40 {
				lfn = ""
			}
			// parse the long filename
			tmpLfn, err := longFilenameEntryFromBytes(b[i : i+32])
			// an error is impossible since we pass exactly 32, but we leave the handler here anyways
			if err != nil {
				return nil, fmt.Errorf("error parsing long filename at position %d: %v", i, err)
			}
			lfn = tmpLfn + lfn
			continue
		}
		// not LFN, so parse regularly
		createTime := binary.LittleEndian.Uint16(b[i+14 : i+16])
		createDate := binary.LittleEndian.Uint16(b[i+16 : i+18])
		accessDate := binary.LittleEndian.Uint16(b[i+18 : i+20])
		modifyTime := binary.LittleEndian.Uint16(b[i+22 : i+24])
		modifyDate := binary.LittleEndian.Uint16(b[i+24 : i+26])
		re := regexp.MustCompile(" +$")
		sfn := re.ReplaceAllString(string(b[i:i+8]), "")
		extension := re.ReplaceAllString(string(b[i+8:i+11]), "")
		isSubdirectory := b[i+11]&0x10 == 0x10
		isArchiveDirty := b[i+11]&0x20 == 0x20
		isVolumeLabel := b[i+11]&0x08 == 0x08
		lowercaseShortname := b[i+12]&0x08 == 0x08
		lowercaseExtension := b[i+12]&0x04 == 0x04

		entry := directoryEntry{
			filenameLong:       lfn,
			longFilenameSlots:  calculateSlots(lfn),
			filenameShort:      sfn,
			fileExtension:      extension,
			fileSize:           binary.LittleEndian.Uint32(b[i+28 : i+32]),
			clusterLocation:    binary.LittleEndian.Uint32(append(b[i+26:i+28], b[i+20:i+22]...)),
			createTime:         dateTimeToTime(createDate, createTime),
			modifyTime:         dateTimeToTime(modifyDate, modifyTime),
			accessTime:         dateTimeToTime(accessDate, 0),
			isSubdirectory:     isSubdirectory,
			isArchiveDirty:     isArchiveDirty,
			isVolumeLabel:      isVolumeLabel,
			lowercaseShortname: lowercaseShortname,
			lowercaseExtension: lowercaseExtension,
		}
		lfn = ""
		dirEntries = append(dirEntries, &entry)
	}
	return dirEntries, nil
}

func dateTimeToTime(d, t uint16) time.Time {
	year := int(d>>9) + 1980
	month := time.Month((d >> 5) & 0x0f)
	date := int(d & 0x1f)
	second := int((t & 0x1f) * 2)
	minute := int((t >> 5) & 0x3f)
	hour := int(t >> 11)
	return time.Date(year, month, date, hour, minute, second, 0, time.UTC)
}
func timeToDateTime(t time.Time) (datePart, timePart uint16) {
	year := t.Year()
	month := int(t.Month())
	day := t.Day()
	second := t.Second()
	minute := t.Minute()
	hour := t.Hour()
	retDate := (year-1980)<<9 + (month << 5) + day
	retTime := hour<<11 + minute<<5 + (second / 2)
	return uint16(retDate), uint16(retTime)
}

func longFilenameBytes(s, shortName, extension string) ([]byte, error) {
	// we need the checksum of the short name
	checksum, err := lfnChecksum(shortName, extension)
	if err != nil {
		return nil, fmt.Errorf("could not calculate checksum for 8.3 filename: %v", err)
	}
	// should be multiple of exactly 32 bytes
	slots := calculateSlots(s)
	// convert our string into runes
	r := []rune(s)
	b2SlotLength := maxCharsLongFilename * 2
	maxChars := slots * maxCharsLongFilename
	b2 := make([]byte, 0, maxChars*2)
	// convert the rune slice into a byte slice with 2 bytes per rune
	// vfat long filenames support UCS-2 *only*
	// so it is *very* important we do not try to parse them otherwise
	for i := 0; i < maxChars; i++ {
		// do we have a rune at this point?
		var tmpb []byte
		switch {
		case i == len(r):
			tmpb = []byte{0x00, 0x00}
		case i > len(r):
			tmpb = []byte{0xff, 0xff}
		default:
			val := uint16(r[i])
			// little endian
			tmpb = []byte{byte(val & 0x00ff), byte(val >> 8)}
		}
		b2 = append(b2, tmpb...)
	}

	// this makes our byte array
	maxBytes := slots * bytesPerSlot
	b := make([]byte, 0, maxBytes)
	// now just place the bytes in the right places
	for count := slots; count > 0; count-- {
		// how far from the start of the byte slice?
		offset := (count - 1) * b2SlotLength
		// enter the right bytes in the right places
		tmpb := make([]byte, 0, 32)
		// first byte is our index
		tmpb = append(tmpb, byte(count))
		// next 10 bytes are 5 chars of data
		tmpb = append(tmpb, b2[offset:offset+10]...)
		// next is a single byte indicating LFN, followed by single byte 0x00
		//nolint:gocritic // gocritic complains about the ability to combine 2 appends into one; we want to be more explicit here
		tmpb = append(tmpb, 0x0f, 0x00)
		// next is checksum
		tmpb = append(tmpb, checksum)
		// next 12 bytes are 6 chars of data
		tmpb = append(tmpb, b2[offset+10:offset+22]...)
		// next are 2 bytes of 0x00
		tmpb = append(tmpb, 0x00, 0x00)
		// next are 4 bytes, last 2 chars of LFN
		tmpb = append(tmpb, b2[offset+22:offset+26]...)
		b = append(b, tmpb...)
	}

	// the first byte should have bit 6 set
	b[0] |= 0x40

	return b, nil
}

// longFilenameEntryFromBytes takes a single slice of 32 bytes and extracts the long filename component from it
func longFilenameEntryFromBytes(b []byte) (string, error) {
	// should be exactly 32 bytes
	bLen := len(b)
	if bLen != 32 {
		return "", fmt.Errorf("longFilenameEntryFromBytes only can parse byte of length 32, not %d", bLen)
	}
	b2 := make([]byte, 0, maxCharsLongFilename*2)
	// strip out the unused ones
	b2 = append(b2, b[1:11]...)
	b2 = append(b2, b[14:26]...)
	b2 = append(b2, b[28:32]...)
	// parse the bytes of the long filename
	// vfat long filenames support UCS-2 *only*
	// so it is *very* important we do not try to parse them otherwise
	r := make([]rune, 0, maxCharsLongFilename)
	// now we can iterate
	for i := 0; i < maxCharsLongFilename; i++ {
		// little endian
		val := uint16(b2[2*i+1])<<8 + uint16(b2[2*i])
		// stop at all 0
		if val == 0 {
			break
		}
		r = append(r, rune(val))
	}
	return string(r), nil
}

// takes the short form of the name and checksums it
// the period between the 8 characters and the 3 character extension is dropped
// any unused chars are replaced by space ASCII 0x20
func lfnChecksum(name, extension string) (byte, error) {
	nameBytes, err := stringToValidASCIIBytes(name)
	if err != nil {
		return 0x00, fmt.Errorf("invalid shortname character in filename: %s", name)
	}
	extensionBytes, err := stringToValidASCIIBytes(extension)
	if err != nil {
		return 0x00, fmt.Errorf("invalid shortname character in extension: %s", extension)
	}

	// now make sure we don't have too many - and fill in blanks
	length := len(nameBytes)
	if length > 8 {
		return 0x00, fmt.Errorf("short name for file is longer than allowed 8 bytes: %s", name)
	}
	for i := 8; i > length; i-- {
		nameBytes = append(nameBytes, 0x20)
	}

	length = len(extensionBytes)
	if length > 3 {
		return 0x00, fmt.Errorf("extension for file is longer than allowed 3 bytes: %s", extension)
	}
	for i := 3; i > length; i-- {
		extensionBytes = append(extensionBytes, 0x20)
	}
	b := make([]byte, len(nameBytes))
	copy(b, nameBytes)
	b = append(b, extensionBytes...)

	// calculate the checksum
	var sum byte = 0x00
	for i := 11; i > 0; i-- {
		sum = ((sum & 0x01) << 7) + (sum >> 1) + b[11-i]
	}
	return sum, nil
}

// convert a string to ascii bytes, but only accept valid 8.3 bytes
func stringToValidASCIIBytes(s string) ([]byte, error) {
	b, err := stringToASCIIBytes(s)
	if err != nil {
		return b, err
	}
	// now make sure every byte is valid
	for _, b2 := range b {
		// only valid chars - 0-9, A-Z, _, ~
		if validShortNameCharacters.Contains(b2) {
			continue
		}
		return nil, fmt.Errorf("invalid 8.3 character")
	}
	return b, nil
}

// convert a string to a byte array, if all characters are valid ascii
func stringToASCIIBytes(s string) ([]byte, error) {
	length := len(s)
	b := make([]byte, length)
	// convert the name into 11 bytes
	r := []rune(s)
	// take the first 8 characters
	for i := 0; i < length; i++ {
		val := int(r[i])
		// we only can handle values less than max byte = 255
		if val > 255 {
			return nil, fmt.Errorf("non-ASCII character in name: %s", s)
		}
		b[i] = byte(val)
	}
	return b, nil
}

// calculate how many vfat slots a long filename takes up
// this does NOT include the slot for the true DOS 8.3 entry
func calculateSlots(s string) int {
	sLen := len(s)
	slots := sLen / charsPerSlot
	if sLen%charsPerSlot != 0 {
		slots++
	}
	return slots
}

// convert LFN to short name
// returns shortName, extension, isLFN, isTruncated
//
//	isLFN : was there an LFN that had to be converted
//	isTruncated : was the shortname longer than 8 chars and had to be converted?
func convertLfnSfn(name string) (shortName, extension string, isLFN, isTruncated bool) {
	// get last period in name
	lastDot := strings.LastIndex(name, ".")
	// now convert it
	var rawShortName, rawExtension string
	rawShortName = name
	// get the extension
	if lastDot > -1 {
		rawExtension = name[lastDot+1:]
		// too long?
		if len(rawExtension) > 3 {
			rawExtension = rawExtension[0:3]
			isLFN = true
		}
		// convert the extension
		extension = uCaseValid(rawExtension)
	}
	if extension != rawExtension {
		isLFN = true
	}

	// convert the short name
	if lastDot > -1 {
		rawShortName = name[:lastDot]
	}
	shortName = uCaseValid(rawShortName)
	if rawShortName != shortName {
		isLFN = true
	}

	// convert shortName to 8 chars
	if len(shortName) > 8 {
		isLFN = true
		isTruncated = true
		shortName = shortName[:6] + "~" + "1"
	}
	return shortName, extension, isLFN, isTruncated
}

// converts a string into upper-case with only valid characters
func uCaseValid(name string) string {
	// easiest way to do this is to go through the name one char at a time
	r := []rune(name)
	r2 := make([]rune, 0, len(r))
	for _, val := range r {
		switch {
		case validShortNameCharacters.Contains(byte(val)):
			r2 = append(r2, val)
		case (0x61 <= val && val <= 0x7a):
			// lower-case characters should be upper-cased
			r2 = append(r2, val-32)
		case val == ' ' || val == '.':
			// remove spaces and periods
			continue
		default:
			// replace the rest with _
			r2 = append(r2, '_')
		}
	}
	return string(r2)
}
