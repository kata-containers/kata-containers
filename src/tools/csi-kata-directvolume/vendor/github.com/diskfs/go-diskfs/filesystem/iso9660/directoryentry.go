package iso9660

import (
	"encoding/binary"
	"errors"
	"fmt"
	"os"
	"path"
	"regexp"
	"strings"
	"time"
)

const (
	directoryEntryMinSize uint8 = 34  // min size is all the required fields (33 bytes) plus 1 byte for the filename
	directoryEntryMaxSize int   = 254 // max size allowed
)

// directoryEntry is a single directory entry
// also fulfills os.FileInfo
//
//	Name() string       // base name of the file
//	Size() int64        // length in bytes for regular files; system-dependent for others
//	Mode() FileMode     // file mode bits
//	ModTime() time.Time // modification time
//	IsDir() bool        // abbreviation for Mode().IsDir()
//	Sys() interface{}   // underlying data source (can return nil)
type directoryEntry struct {
	extAttrSize              uint8
	location                 uint32
	size                     uint32
	creation                 time.Time
	isHidden                 bool
	isSubdirectory           bool
	isAssociated             bool
	hasExtendedAttrs         bool
	hasOwnerGroupPermissions bool
	hasMoreEntries           bool
	isSelf                   bool
	isParent                 bool
	volumeSequence           uint16
	filesystem               *FileSystem
	filename                 string
	extensions               []directoryEntrySystemUseExtension
}

func (de *directoryEntry) countNamelenBytes() int {
	// size includes the ";1" at the end as two bytes if a filename
	var namelen int
	switch {
	case de.isSelf:
		namelen = 1
	case de.isParent:
		namelen = 1
	default:
		namelen = len(de.filename)
	}

	return namelen
}

func (de *directoryEntry) countBaseBytes() int {
	namelen := de.countNamelenBytes()
	// if even, we add one byte of padding to always end on an even byte
	if namelen%2 == 0 {
		namelen++
	}

	return 33 + namelen
}

func (de *directoryEntry) toBytes(skipExt bool, ceBlocks []uint32) ([][]byte, error) {
	baseRecordSize := de.countBaseBytes()
	namelen := de.countNamelenBytes()

	b := make([]byte, baseRecordSize)

	b[1] = de.extAttrSize
	binary.LittleEndian.PutUint32(b[2:6], de.location)
	binary.BigEndian.PutUint32(b[6:10], de.location)
	binary.LittleEndian.PutUint32(b[10:14], de.size)
	binary.BigEndian.PutUint32(b[14:18], de.size)
	copy(b[18:25], timeToBytes(de.creation))

	// set the flags
	var flagByte byte = 0x00
	if de.isHidden {
		flagByte |= 0x01
	}
	if de.isSubdirectory {
		flagByte |= 0x02
	}
	if de.isAssociated {
		flagByte |= 0x04
	}
	if de.hasExtendedAttrs {
		flagByte |= 0x08
	}
	if de.hasOwnerGroupPermissions {
		flagByte |= 0x10
	}
	if de.hasMoreEntries {
		flagByte |= 0x80
	}
	b[25] = flagByte
	// volume sequence number - uint16 in both endian
	binary.LittleEndian.PutUint16(b[28:30], de.volumeSequence)
	binary.BigEndian.PutUint16(b[30:32], de.volumeSequence)

	b[32] = uint8(namelen)

	// save the filename
	var filenameBytes []byte
	var err error
	switch {
	case de.isSelf:
		filenameBytes = []byte{0x00}
	case de.isParent:
		filenameBytes = []byte{0x01}
	default:
		// first validate the filename
		err = validateFilename(de.filename, de.isSubdirectory)
		if err != nil {
			nametype := "filename"
			if de.isSubdirectory {
				nametype = "directory"
			}
			return nil, fmt.Errorf("invalid %s %s: %v", nametype, de.filename, err)
		}
		filenameBytes, err = stringToASCIIBytes(de.filename)
		if err != nil {
			return nil, fmt.Errorf("error converting filename to bytes: %v", err)
		}
	}

	// copy it over
	copy(b[33:], filenameBytes)

	// output directory entry extensions - but only if we did not skip it
	var extBytes [][]byte
	if !skipExt {
		extBytes, err = dirEntryExtensionsToBytes(de.extensions, directoryEntryMaxSize-len(b), de.filesystem.blocksize, ceBlocks)
		if err != nil {
			return nil, fmt.Errorf("enable to convert directory entry SUSP extensions to bytes: %v", err)
		}
		b = append(b, extBytes[0]...)
	}
	// always end on an even
	if len(b)%2 != 0 {
		b = append(b, 0x00)
	}
	// update the record size
	b[0] = uint8(len(b))

	recWithCE := [][]byte{b}
	if len(extBytes) > 1 {
		recWithCE = append(recWithCE, extBytes[1:]...)
	}
	return recWithCE, nil
}

// dirEntryExtensionsToBytes converts slice of SUSP extensions to slice ot []byte: first is dir entry, rest are continuation areas
// returns:
//
//	slice of []byte
func dirEntryExtensionsToBytes(extensions []directoryEntrySystemUseExtension, maxSize int, blocksize int64, ceBlocks []uint32) ([][]byte, error) {
	// output directory entries
	var (
		err            error
		b              []byte
		continuedBytes [][]byte
	)
	ret := make([][]byte, 0)
	for i, e := range extensions {
		b2 := e.Bytes()
		// do we overrun the size
		if len(b)+len(b2) > maxSize {
			// we need an extension, so pop the first one off the slice, use it as a pointer, and pass the rest
			nextCeBlock := ceBlocks[0]
			continuedBytes, err = dirEntryExtensionsToBytes(extensions[i:], int(blocksize), blocksize, ceBlocks[1:])
			if err != nil {
				return nil, err
			}
			// use a continuation entry until the end of the
			ce := &directoryEntrySystemUseContinuation{
				offset:             0,
				location:           nextCeBlock,
				continuationLength: uint32(len(continuedBytes[0])),
			}
			b = append(b, ce.Bytes()...)
			break
		} else {
			b = append(b, b2...)
		}
	}
	ret = append(ret, b)
	if len(continuedBytes) > 0 {
		ret = append(ret, continuedBytes...)
	}
	return ret, nil
}

func dirEntryFromBytes(b []byte, ext []suspExtension) (*directoryEntry, error) {
	// has to be at least 34 bytes
	if len(b) < int(directoryEntryMinSize) {
		return nil, fmt.Errorf("cannot read directoryEntry from %d bytes, fewer than minimum of %d bytes", len(b), directoryEntryMinSize)
	}
	recordSize := b[0]
	// what if it is not the right size?
	if len(b) != int(recordSize) {
		return nil, fmt.Errorf("directoryEntry should be size %d bytes according to first byte, but have %d bytes", recordSize, len(b))
	}
	extAttrSize := b[1]
	location := binary.LittleEndian.Uint32(b[2:6])
	size := binary.LittleEndian.Uint32(b[10:14])
	creation := bytesToTime(b[18:25])

	// get the flags
	flagByte := b[25]
	isHidden := flagByte&0x01 == 0x01
	isSubdirectory := flagByte&0x02 == 0x02
	isAssociated := flagByte&0x04 == 0x04
	hasExtendedAttrs := flagByte&0x08 == 0x08
	hasOwnerGroupPermissions := flagByte&0x10 == 0x10
	hasMoreEntries := flagByte&0x80 == 0x80

	volumeSequence := binary.LittleEndian.Uint16(b[28:30])

	// size includes the ";1" at the end as two bytes and any padding
	namelen := b[32]
	nameLenWithPadding := namelen

	// get the filename itself
	nameBytes := b[33 : 33+namelen]
	if namelen > 1 && namelen%2 == 0 {
		nameLenWithPadding++
	}
	var filename string
	var isSelf, isParent bool
	switch {
	case namelen == 1 && nameBytes[0] == 0x00:
		filename = ""
		isSelf = true
	case namelen == 1 && nameBytes[0] == 0x01:
		filename = ""
		isParent = true
	default:
		filename = string(nameBytes)
	}

	// and now for extensions in the system use area
	suspFields := make([]directoryEntrySystemUseExtension, 0)
	if len(b) > 33+int(nameLenWithPadding) {
		var err error
		suspFields, err = parseDirectoryEntryExtensions(b[33+nameLenWithPadding:], ext)
		if err != nil {
			return nil, fmt.Errorf("unable to parse directory entry extensions: %v", err)
		}
	}

	return &directoryEntry{
		extAttrSize:              extAttrSize,
		location:                 location,
		size:                     size,
		creation:                 creation,
		isHidden:                 isHidden,
		isSubdirectory:           isSubdirectory,
		isAssociated:             isAssociated,
		hasExtendedAttrs:         hasExtendedAttrs,
		hasOwnerGroupPermissions: hasOwnerGroupPermissions,
		hasMoreEntries:           hasMoreEntries,
		isSelf:                   isSelf,
		isParent:                 isParent,
		volumeSequence:           volumeSequence,
		filename:                 filename,
		extensions:               suspFields,
	}, nil
}

// parseDirEntry takes the bytes of a single directory entry
// and parses it, including pulling in continuation entry bytes
func parseDirEntry(b []byte, f *FileSystem) (*directoryEntry, error) {
	// empty entry means nothing more to read - this might not actually be accurate, but work with it for now
	if len(b) < 1 {
		return nil, errors.New("cannot parse zero length directory entry")
	}
	entryLen := int(b[0])
	if entryLen == 0 {
		return nil, nil
	}
	// get the bytes
	de, err := dirEntryFromBytes(b[:entryLen], f.suspExtensions)
	if err != nil {
		return nil, fmt.Errorf("invalid directory entry : %v", err)
	}
	de.filesystem = f

	if f.suspEnabled && len(de.extensions) > 0 {
		// if the last entry is a continuation SUSP entry and SUSP is enabled, we need to follow and parse them
		// because the extensions can be a linked list directory -> CE area -> CE area ...
		//   we need to loop until it is no more
		for {
			if ce, ok := de.extensions[len(de.extensions)-1].(directoryEntrySystemUseContinuation); ok {
				location := int64(ce.Location())
				size := int(ce.ContinuationLength())
				offset := int64(ce.Offset())
				// read it from disk
				continuationBytes := make([]byte, size)
				read, err := f.file.ReadAt(continuationBytes, location*f.blocksize+offset)
				if err != nil {
					return nil, fmt.Errorf("error reading continuation entry data at %d: %v", location, err)
				}
				if read != size {
					return nil, fmt.Errorf("read continuation entry data %d bytes instead of expected %d", read, size)
				}
				// parse and append
				entries, err := parseDirectoryEntryExtensions(continuationBytes, f.suspExtensions)
				if err != nil {
					return nil, fmt.Errorf("error parsing continuation entry data at %d: %v", location, err)
				}
				// remove the CE one from the extensions array and append our new ones
				de.extensions = append(de.extensions[:len(de.extensions)-1], entries...)
			} else {
				break
			}
		}
	}
	return de, nil
}

// parseDirEntries takes all of the bytes in a special file (i.e. a directory)
// and gets all of the DirectoryEntry for that directory
// this is, essentially, the equivalent of `ls -l` or if you prefer `dir`
func parseDirEntries(b []byte, f *FileSystem) ([]*directoryEntry, error) {
	dirEntries := make([]*directoryEntry, 0, 20)
	count := 0
	for i := 0; i < len(b); count++ {
		// empty entry means nothing more to read - this might not actually be accurate, but work with it for now
		entryLen := int(b[i+0])
		if entryLen == 0 {
			i += (int(f.blocksize) - i%int(f.blocksize))
			continue
		}
		de, err := parseDirEntry(b[i+0:i+entryLen], f)
		if err != nil {
			return nil, fmt.Errorf("invalid directory entry %d at byte %d: %v", count, i, err)
		}
		// some extensions to directory relocation, so check if we should ignore it
		if f.suspEnabled {
			for _, e := range f.suspExtensions {
				if e.Relocated(de) {
					de = nil
					break
				}
			}
		}

		if de != nil {
			dirEntries = append(dirEntries, de)
		}
		i += entryLen
	}
	return dirEntries, nil
}

// get the location of a particular path relative to this directory
func (de *directoryEntry) getLocation(p string) (location, size uint32, err error) {
	// break path down into parts and levels
	parts := splitPath(p)
	if len(parts) == 0 {
		location = de.location
		size = de.size
	} else {
		current := parts[0]
		// read the directory bytes
		dirb := make([]byte, de.size)
		n, err := de.filesystem.file.ReadAt(dirb, int64(de.location)*de.filesystem.blocksize)
		if err != nil {
			return 0, 0, fmt.Errorf("could not read directory: %v", err)
		}
		if n != len(dirb) {
			return 0, 0, fmt.Errorf("read %d bytes instead of expected %d", n, len(dirb))
		}
		// parse those entries
		dirEntries, err := parseDirEntries(dirb, de.filesystem)
		if err != nil {
			return 0, 0, fmt.Errorf("could not parse directory: %v", err)
		}
		// find the entry among the children that has the desired name
		for _, entry := range dirEntries {
			// do we have an alternate name?
			// only care if not self or parent entry
			checkFilename := entry.filename
			if de.filesystem.suspEnabled && !entry.isSelf && !entry.isParent {
				for _, e := range de.filesystem.suspExtensions {
					filename, err2 := e.GetFilename(entry)
					switch {
					case err2 != nil && err2 == ErrSuspFilenameUnsupported:
						continue
					case err2 != nil:
						return 0, 0, fmt.Errorf("extension %s count not find a filename property: %v", e.ID(), err2)
					default:
						checkFilename = filename
						//nolint:gosimple // redundant break, but we want this explicit
						break
					}
				}
			}
			if checkFilename == current {
				if len(parts) > 1 {
					// just dig down further - what if it looks like a file, but is a relocated directory?
					if !entry.isSubdirectory && de.filesystem.suspEnabled && !entry.isSelf && !entry.isParent {
						for _, e := range de.filesystem.suspExtensions {
							location2 := e.GetDirectoryLocation(entry)
							if location2 != 0 {
								// need to get the directory entry for the child
								dirb := make([]byte, de.filesystem.blocksize)
								n, err2 := de.filesystem.file.ReadAt(dirb, int64(location2)*de.filesystem.blocksize)
								if err2 != nil {
									return 0, 0, fmt.Errorf("could not read bytes of relocated directory %s from block %d: %v", checkFilename, location2, err2)
								}
								if n != len(dirb) {
									return 0, 0, fmt.Errorf("read %d bytes instead of expected %d for relocated directory %s from block %d: %v", n, len(dirb), checkFilename, location2, err)
								}
								// get the size of the actual directory entry
								size2 := dirb[0]
								entry, err2 = parseDirEntry(dirb[:size2], de.filesystem)
								if err2 != nil {
									return 0, 0, fmt.Errorf("error converting bytes to a directory entry for relocated directory %s from block %d: %v", checkFilename, location2, err2)
								}
								break
							}
						}
					}
					location, size, err = entry.getLocation(path.Join(parts[1:]...))
					if err != nil {
						return 0, 0, fmt.Errorf("could not get location: %v", err)
					}
				} else {
					// this is the final one, we found it, keep it
					location = entry.location
					size = entry.size
				}
				break
			}
		}
	}

	return location, size, nil
}

// Name() string       // base name of the file
func (de *directoryEntry) Name() string {
	name := de.filename
	if de.filesystem.suspEnabled {
		for _, e := range de.filesystem.suspExtensions {
			filename, err := e.GetFilename(de)
			switch {
			case err != nil:
				continue
			default:
				name = filename
				//nolint:gosimple // redundant break, but we want this explicit
				break
			}
		}
	}
	// check if we have an extension that overrides it
	// filenames should have the ';1' stripped off, as well as the leading or trailing '.'
	if !de.IsDir() {
		name = strings.TrimSuffix(name, ";1")
		name = strings.TrimSuffix(name, ".")
		name = strings.TrimPrefix(name, ".")
	}
	return name
}

// Size() int64        // length in bytes for regular files; system-dependent for others
func (de *directoryEntry) Size() int64 {
	return int64(de.size)
}

// Mode() FileMode     // file mode bits
func (de *directoryEntry) Mode() os.FileMode {
	return 0o755
}

// ModTime() time.Time // modification time
func (de *directoryEntry) ModTime() time.Time {
	return de.creation
}

// IsDir() bool        // abbreviation for Mode().IsDir()
func (de *directoryEntry) IsDir() bool {
	return de.isSubdirectory
}

// Sys() interface{}   // underlying data source (can return nil)
func (de *directoryEntry) Sys() interface{} {
	return nil
}

// utilities

func bytesToTime(b []byte) time.Time {
	year := int(b[0])
	month := time.Month(b[1])
	date := int(b[2])
	hour := int(b[3])
	minute := int(b[4])
	second := int(b[5])
	offset := int(int8(b[6]))
	location := time.FixedZone("iso", offset*15*60)
	return time.Date(year+1900, month, date, hour, minute, second, 0, location)
}

func timeToBytes(t time.Time) []byte {
	year := t.Year()
	month := t.Month()
	date := t.Day()
	second := t.Second()
	minute := t.Minute()
	hour := t.Hour()
	_, offset := t.Zone()
	b := make([]byte, 7)
	b[0] = byte(year - 1900)
	b[1] = byte(month)
	b[2] = byte(date)
	b[3] = byte(hour)
	b[4] = byte(minute)
	b[5] = byte(second)
	b[6] = byte(int8(offset / 60 / 15))
	return b
}

// convert a string to ascii bytes, but only accept valid d-characters
func validateFilename(s string, isDir bool) error {
	var err error
	if isDir {
		// directory only allowed up to 8 characters of A-Z,0-9,_
		re := regexp.MustCompile("^[A-Z0-9_]{1,30}$")
		if !re.MatchString(s) {
			err = fmt.Errorf("directory name must be of up to 30 characters from A-Z0-9_")
		}
	} else {
		// filename only allowed up to 8 characters of A-Z,0-9,_, plus an optional '.' plus up to 3 characters of A-Z,0-9,_, plus must have ";1"
		re := regexp.MustCompile("^[A-Z0-9_]+(.[A-Z0-9_]*)?;1$")
		switch {
		case !re.MatchString(s):
			err = fmt.Errorf("file name must be of characters from A-Z0-9_, followed by an optional '.' and an extension of the same characters")
		case len(strings.ReplaceAll(s, ".", "")) > 30:
			err = fmt.Errorf("file name must be at most 30 characters, not including the separator '.'")
		}
	}
	return err
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

// converts a string into upper-case with only valid characters
func uCaseValid(name string) string {
	// easiest way to do this is to go through the name one char at a time
	r := []rune(name)
	r2 := make([]rune, 0, len(r))
	for _, val := range r {
		switch {
		case (0x30 <= val && val <= 0x39) || (0x41 <= val && val <= 0x5a) || (val == 0x7e):
			// naturally valid characters
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
