package iso9660

import (
	"encoding/binary"
	"fmt"
	"os"
	"sort"
	"time"

	"gopkg.in/djherbis/times.v1"
)

const (
	rockRidgeSignaturePosixAttributes    = "PX"
	rockRidgeSignaturePosixDeviceNumber  = "PN"
	rockRidgeSignatureSymbolicLink       = "SL"
	rockRidgeSignatureName               = "NM"
	rockRidgeSignatureChild              = "CL"
	rockRidgeSignatureParent             = "PL"
	rockRidgeSignatureRelocatedDirectory = "RE"
	rockRidgeSignatureTimestamps         = "TF"
	rockRidgeSignatureSparseFile         = "SF"
	rockRidge110                         = "RRIP_1991A"
	rockRidge112                         = "IEEE_P1282"
)

// rockRidgeExtension implements suspExtension interface
type rockRidgeExtension struct {
	version    string
	id         string
	descriptor string
	source     string
	pxLength   int
	sfLength   int
}

func (r *rockRidgeExtension) ID() string {
	return r.id
}
func (r *rockRidgeExtension) Descriptor() string {
	return r.descriptor
}
func (r *rockRidgeExtension) Source() string {
	return r.source
}
func (r *rockRidgeExtension) Version() uint8 {
	return 1
}
func (r *rockRidgeExtension) Process(signature string, b []byte) (directoryEntrySystemUseExtension, error) {
	// if we have a parser, use it, else use the raw parser
	var (
		entry directoryEntrySystemUseExtension
		err   error
	)
	switch signature {
	case rockRidgeSignaturePosixAttributes:
		entry, err = r.parsePosixAttributes(b)
	case rockRidgeSignaturePosixDeviceNumber:
		entry, err = r.parsePosixDeviceNumber(b)
	case rockRidgeSignatureSymbolicLink:
		entry, err = r.parseSymlink(b)
	case rockRidgeSignatureName:
		entry, err = r.parseName(b)
	case rockRidgeSignatureChild:
		entry, err = r.parseChildDirectory(b)
	case rockRidgeSignatureParent:
		entry, err = r.parseParentDirectory(b)
	case rockRidgeSignatureRelocatedDirectory:
		entry, err = r.parseRelocatedDirectory(b)
	case rockRidgeSignatureTimestamps:
		entry, err = r.parseTimestamps(b)
	case rockRidgeSignatureSparseFile:
		entry, err = r.parseSparseFile(b)
	default:
		return nil, ErrSuspNoHandler
	}
	if err != nil {
		return nil, fmt.Errorf("error parsing %s extension by Rock Ridge : %v", signature, err)
	}
	return entry, nil
}

// get the rock ridge filename for a directory entry
func (r *rockRidgeExtension) GetFilename(de *directoryEntry) (string, error) {
	found := false
	name := ""
	for _, e := range de.extensions {
		if nm, ok := e.(rockRidgeName); ok {
			found = true
			name = nm.name
			break
		}
	}
	if !found {
		return "", fmt.Errorf("could not find Rock Ridge filename property")
	}
	return name, nil
}
func (r *rockRidgeExtension) GetFileExtensions(fp string, isSelf, isParent bool) ([]directoryEntrySystemUseExtension, error) {
	// we always do PX, TF, NM, SL order
	ret := []directoryEntrySystemUseExtension{}
	// do not follow symlinks
	fi, err := os.Lstat(fp)
	if err != nil {
		return nil, fmt.Errorf("error reading file %s: %v", fp, err)
	}

	t, err := times.Lstat(fp)
	if err != nil {
		return nil, fmt.Errorf("error reading times %s: %v", fp, err)
	}

	// PX
	nlink, uid, gid := statt(fi)
	mtime := fi.ModTime()
	atime := t.AccessTime()
	ctime := t.ChangeTime()

	ret = append(ret, rockRidgePosixAttributes{
		mode:      fi.Mode(),
		linkCount: nlink,
		uid:       uid,
		gid:       gid,
		length:    r.pxLength,
	})
	// TF
	tf := rockRidgeTimestamps{longForm: false, stamps: []rockRidgeTimestamp{
		{timestampType: rockRidgeTimestampModify, time: mtime},
		{timestampType: rockRidgeTimestampAccess, time: atime},
		{timestampType: rockRidgeTimestampAttribute, time: ctime},
	}}

	ret = append(ret, tf)
	// NM
	if !isSelf && !isParent {
		ret = append(ret, rockRidgeName{name: fi.Name()})
	}
	// SL
	if fi.Mode()&os.ModeSymlink == os.ModeSymlink {
		// need the target if it is a symlink
		target, err := os.Readlink(fp)
		if err != nil {
			return nil, fmt.Errorf("error reading symlink target at %s", fp)
		}
		ret = append(ret, rockRidgeSymlink{continued: false, name: target})
	}

	return ret, nil
}

func (r *rockRidgeExtension) GetFinalizeExtensions(fi *finalizeFileInfo) ([]directoryEntrySystemUseExtension, error) {
	// we look for CL, PL, RE entries
	ret := []directoryEntrySystemUseExtension{}
	if fi.trueParent != nil {
		ret = append(ret, rockRidgeRelocatedDirectory{}, rockRidgeParentDirectory{location: fi.trueParent.location})
	}
	if fi.trueChild != nil {
		ret = append(ret, rockRidgeChildDirectory{location: fi.trueChild.location})
	}
	return ret, nil
}

// determine if a directory entry was relocated
func (r *rockRidgeExtension) Relocated(de *directoryEntry) bool {
	relocated := false
	for _, e := range de.extensions {
		if _, ok := e.(rockRidgeRelocatedDirectory); ok {
			relocated = true
			break
		}
	}
	return relocated
}

// Relocatable can rock ridge handle deep directory relocations? yes
func (r *rockRidgeExtension) Relocatable() bool {
	return true
}

func (r *rockRidgeExtension) UsePathtable() bool {
	return false
}

// Relocate restructure so that all directories are at a depth of 8 or fewer
func (r *rockRidgeExtension) Relocate(dirs map[string]*finalizeFileInfo) ([]*finalizeFileInfo, map[string]*finalizeFileInfo, error) {
	files := make([]*finalizeFileInfo, 0)
	root := dirs["."]
	relocationDir := root
	if relocationDir.depth == 8 {
		return nil, nil, fmt.Errorf("cannot relocate when relocation parent already is max depth 8")
	}
	/* logic:
	 * 1. go down the directories
	 * 2. as soon as we find one whose depth > 8, move it to the parent
	 * 3. change the depth of it and all of its children to its new depth
	 * 4. reparent it
	 * 5. change child entries in the parent
	 *
	 * repeat until none is deeper than 8
	 */
	// deepers contains the list of all dirs exactly at one too deep, i.e. 9
	deepers := make([]*finalizeFileInfo, 0)
	for _, e := range dirs {
		if e.depth == 9 {
			deepers = append(deepers, e)
		}
	}
	// repeat until deepers has no children of depth > 8
	for {
		if len(deepers) < 1 {
			break
		}
		for _, e := range deepers {
			// we have a depth greater than 8, so move it
			e.trueParent = e.parent
			e.parent = relocationDir
			// create the file that represents it
			children := make([]*finalizeFileInfo, 0)
			for _, c := range e.trueParent.children {
				if c != e {
					children = append(children, e)
					continue
				}
				// copy over but replace a few key items
				content := []byte("Rock Ridge relocated")
				replacer := &finalizeFileInfo{}
				*replacer = *c
				replacer.isDir = false
				replacer.mode = c.mode & (^os.ModeDir)
				replacer.size = int64(len(content))
				replacer.content = content
				replacer.trueChild = e
				children = append(children, replacer)
			}
			e.trueParent.children = children
			// cycle down and update the depth for all children
			e.updateDepth(relocationDir.depth + 1)
		}
		// go through deepers, remove all of those that are not >8
		deepers = make([]*finalizeFileInfo, 0)
		for _, e := range dirs {
			if e.depth == 9 {
				deepers = append(deepers, e)
			}
		}
	}
	return files, dirs, nil
}

// find the directory location
func (r *rockRidgeExtension) GetDirectoryLocation(de *directoryEntry) uint32 {
	newEntry := uint32(0)
	for _, e := range de.extensions {
		if child, ok := e.(rockRidgeChildDirectory); ok {
			newEntry = child.location
			break
		}
	}
	return newEntry
}

func getRockRidgeExtension(id string) *rockRidgeExtension {
	var ret *rockRidgeExtension // defaults to nil
	switch id {
	case rockRidge110:
		ret = &rockRidgeExtension{
			id:         id,
			version:    "1.10",
			descriptor: "THE ROCK RIDGE INTERCHANGE PROTOCOL PROVIDES SUPPORT FOR POSIX FILE SYSTEM SEMANTICS",
			source:     "PLEASE CONTACT DISC PUBLISHER FOR SPECIFICATION SOURCE. SEE PUBLISHER IDENTIFIER IN PRIMARY VOLUME DESCRIPTOR FOR CONTACT INFORMATION.",
			pxLength:   36,
			sfLength:   12,
		}
	case rockRidge112:
		ret = &rockRidgeExtension{
			id:         id,
			version:    "1.12",
			descriptor: "THE IEEE P1282 PROTOCOL PROVIDES SUPPORT FOR POSIX FILE SYSTEM SEMANTICS.",
			source:     "PLEASE CONTACT THE IEEE STANDARDS DEPARTMENT, PISCATAWAY, NJ, USA FOR THE P1282 SPECIFICATION.",
			pxLength:   44,
			sfLength:   21,
		}
	}
	return ret
}

// rockRidgePosixAttributes
type rockRidgePosixAttributes struct {
	mode         os.FileMode
	saveSwapText bool
	length       int

	linkCount uint32
	uid       uint32
	gid       uint32
	serial    uint32
}

func (d rockRidgePosixAttributes) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(rockRidgePosixAttributes)
	return ok && t == d
}

func (d rockRidgePosixAttributes) Signature() string {
	return rockRidgeSignaturePosixAttributes
}
func (d rockRidgePosixAttributes) Length() int {
	return d.length
}
func (d rockRidgePosixAttributes) Version() uint8 {
	return 1
}
func (d rockRidgePosixAttributes) Data() []byte {
	ret := make([]byte, d.length-4)
	modes := uint32(0)
	regular := true
	m := d.mode
	// get Unix permission bits - golang and Rock Ridge use the same ones
	modes |= uint32(m & 0o777)
	// get setuid and setgid
	modes |= uint32(m & os.ModeSetuid)
	modes |= uint32(m & os.ModeSetgid)
	// save swapped text mode seems to have no parallel
	if d.saveSwapText {
		modes |= 0o1000
	}
	// the rest of the modes do not use the same bits on Rock Ridge and on golang
	if m&os.ModeSocket == os.ModeSocket {
		modes |= 0o140000
		regular = false
	}
	if m&os.ModeSymlink == os.ModeSymlink {
		modes |= 0o120000
		regular = false
	}
	if m&os.ModeDevice == os.ModeDevice {
		regular = false
		if m&os.ModeCharDevice == os.ModeCharDevice {
			modes |= 0o20000
		} else {
			modes |= 0o60000
		}
	}
	if m&os.ModeDir == os.ModeDir {
		modes |= 0o40000
		regular = false
	}
	if m&os.ModeNamedPipe == os.ModeNamedPipe {
		modes |= 0o10000
		regular = false
	}
	if regular {
		modes |= 0o100000
	}

	binary.LittleEndian.PutUint32(ret[0:4], modes)
	binary.BigEndian.PutUint32(ret[4:8], modes)
	binary.LittleEndian.PutUint32(ret[8:12], d.linkCount)
	binary.BigEndian.PutUint32(ret[12:16], d.linkCount)
	binary.LittleEndian.PutUint32(ret[16:20], d.uid)
	binary.BigEndian.PutUint32(ret[20:24], d.uid)
	binary.LittleEndian.PutUint32(ret[24:28], d.gid)
	binary.BigEndian.PutUint32(ret[28:32], d.gid)
	if d.length == 44 {
		binary.LittleEndian.PutUint32(ret[32:36], d.serial)
		binary.BigEndian.PutUint32(ret[36:40], d.serial)
	}
	return ret
}
func (d rockRidgePosixAttributes) Bytes() []byte {
	ret := make([]byte, 4)
	copy(ret[0:2], rockRidgeSignaturePosixAttributes)
	ret[2] = uint8(d.Length())
	ret[3] = d.Version()
	ret = append(ret, d.Data()...)
	return ret
}
func (d rockRidgePosixAttributes) Continuable() bool {
	return false
}
func (d rockRidgePosixAttributes) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func (r *rockRidgeExtension) parsePosixAttributes(b []byte) (directoryEntrySystemUseExtension, error) {
	targetSize := r.pxLength
	if len(b) != targetSize {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge PX extension must be %d bytes, but received %d", targetSize, len(b))
	}
	size := b[2]
	if size != uint8(targetSize) {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge PX extension must be %d bytes, but byte 2 indicated %d", targetSize, size)
	}
	version := b[3]
	if version != 1 {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge PX extension must be version 1, was %d", version)
	}
	// file mode
	modes := binary.LittleEndian.Uint32(b[4:8])
	var m uint32
	// get Unix permission bits - golang and Rock Ridge use the same ones
	m |= (modes & 0o777)
	// get setuid and setgid
	m |= (modes & uint32(os.ModeSetuid))
	m |= (modes & uint32(os.ModeSetgid))
	// save swapped text mode seems to have no parallel
	var saveSwapText bool
	if modes&0o01000 != 0 {
		saveSwapText = true
	}
	// the rest of the modes do not use the same bits on Rock Ridge and on golang, and are exclusive
	switch {
	case modes&0o140000 == 0o140000:
		m |= uint32(os.ModeSocket)
	case modes&0o120000 == 0o120000:
		m |= uint32(os.ModeSymlink)
	case modes&0o20000 == 0o20000:
		m |= uint32(os.ModeCharDevice | os.ModeDevice)
	case modes&0o60000 == 0o60000:
		m |= uint32(os.ModeDevice)
	case modes&0o40000 == 0o40000:
		m |= uint32(os.ModeDir)
	case modes&0o10000 == 0o10000:
		m |= uint32(os.ModeNamedPipe)
	}

	var serial uint32
	if len(b) == 44 {
		serial = binary.LittleEndian.Uint32(b[36:40])
	}
	return rockRidgePosixAttributes{
		mode:         os.FileMode(m),
		saveSwapText: saveSwapText,
		linkCount:    binary.LittleEndian.Uint32(b[12:16]),
		uid:          binary.LittleEndian.Uint32(b[20:24]),
		gid:          binary.LittleEndian.Uint32(b[28:32]),
		serial:       serial,
		length:       targetSize,
	}, nil
}

// rockRidgePosixDeviceNumber
type rockRidgePosixDeviceNumber struct {
	high uint32
	low  uint32
}

func (d rockRidgePosixDeviceNumber) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(rockRidgePosixDeviceNumber)
	return ok && t == d
}
func (d rockRidgePosixDeviceNumber) Signature() string {
	return rockRidgeSignaturePosixDeviceNumber
}
func (d rockRidgePosixDeviceNumber) Length() int {
	return 20
}
func (d rockRidgePosixDeviceNumber) Version() uint8 {
	return 1
}
func (d rockRidgePosixDeviceNumber) Data() []byte {
	ret := make([]byte, 16)

	binary.LittleEndian.PutUint32(ret[0:4], d.high)
	binary.BigEndian.PutUint32(ret[4:8], d.high)
	binary.LittleEndian.PutUint32(ret[8:12], d.low)
	binary.BigEndian.PutUint32(ret[12:16], d.low)
	return ret
}
func (d rockRidgePosixDeviceNumber) Bytes() []byte {
	ret := make([]byte, 4)
	copy(ret[0:2], rockRidgeSignaturePosixDeviceNumber)
	ret[2] = uint8(d.Length())
	ret[3] = d.Version()
	ret = append(ret, d.Data()...)
	return ret
}
func (d rockRidgePosixDeviceNumber) Continuable() bool {
	return false
}
func (d rockRidgePosixDeviceNumber) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func (r *rockRidgeExtension) parsePosixDeviceNumber(b []byte) (directoryEntrySystemUseExtension, error) {
	targetSize := 20
	if len(b) != targetSize {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge PN extension must be %d bytes, but received %d", targetSize, len(b))
	}
	size := b[2]
	if size != uint8(targetSize) {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge PN extension must be %d bytes, but byte 2 indicated %d", targetSize, size)
	}
	version := b[3]
	if version != 1 {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge PN extension must be version 1, was %d", version)
	}
	return rockRidgePosixDeviceNumber{
		high: binary.LittleEndian.Uint32(b[4:8]),
		low:  binary.LittleEndian.Uint32(b[12:16]),
	}, nil
}

// rockRidgeSymlink
// a symlink can be greater than the 254 max size of a SUSP extension, so it may continue across multiple extension entries
// a rockRidgeSymlink can represent the individual components, or an entire set merged together
// Bytes(), when called, will provide as many consecutive symlink bytes as needed
type rockRidgeSymlink struct {
	continued bool // if this is continuted in another rockRidgeSymlink entry
	name      string
}

func (d rockRidgeSymlink) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(rockRidgeSymlink)
	return ok && t == d
}
func (d rockRidgeSymlink) Signature() string {
	return rockRidgeSignatureSymbolicLink
}
func (d rockRidgeSymlink) Length() int {
	// basic 4 bytes for all SUSP entries, 1 byte for flags, 1 for Component Flags, 1 for Component Len, name
	return 4 + 1 + 2 + len(d.name)
}
func (d rockRidgeSymlink) Version() uint8 {
	return 1
}
func (d rockRidgeSymlink) Data() []byte {
	return []byte{}
}
func (d rockRidgeSymlink) Bytes() []byte {
	// This could be a single entry, or so long that you need multiple concatenated
	// maximum size of a single entry is 254 bytes
	// each SL record requires 4 bytes of header, 1 byte of flags, and 2 bytes of flags and length in component area
	//  so available = 254-(4+1+2) = 247
	headerSize := 4 + 1 + 2
	maxComponentSize := directoryEntryMaxSize - headerSize
	// break the target of the link down into component parts, and then we can calculate the size
	components := splitPath(d.name)
	root := false
	if d.name[0] == "/"[0] {
		root = true
	}
	// go through the components, convert to []byte and add on
	cBytes := make([][]byte, 0)
	if root {
		// component flag, component len, component of path
		cBytes = append(cBytes, []byte{0x08, 0x0})
	}
	for _, e := range components {
		switch e {
		case "..":
			cBytes = append(cBytes, []byte{0x4, 0x0})
		case ".":
			cBytes = append(cBytes, []byte{0x2, 0x0})
		default:
			cBytes = append(cBytes, []byte{0x0, byte(len(e))}, []byte(e))
		}
	}
	// we now have cBytes, which is all of the component parts
	// split into SL entries as needed
	b := make([]byte, 0)
	b2 := make([]byte, 5)
	copy(b2[0:2], rockRidgeSignatureSymbolicLink)
	// we set size and continuing flag when we are done with this entry
	b2[3] = d.Version()
	componentByteCount := 0
	for _, e := range cBytes {
		if len(e)+componentByteCount > maxComponentSize {
			// cannot add it, so close the existing one and start a new record
			b2[2] = uint8(componentByteCount + 5)
			b2[4] = 1
			b = append(b, b2...)

			// new record
			b2 = make([]byte, 5)
			copy(b2[0:2], rockRidgeSignatureSymbolicLink)
			// we set size and continuing flag when we are done with this entry
			b2[3] = d.Version()
			componentByteCount = 0
		}
		b2 = append(b2, e...)
		componentByteCount += len(e)
	}
	if len(b2) > 0 {
		b2[2] = uint8(componentByteCount + 5)
		// this one is not continuing, it is the last
		b2[4] = 0
		b = append(b, b2...)
	}

	return b
}
func (d rockRidgeSymlink) Continuable() bool {
	return d.continued
}
func (d rockRidgeSymlink) Merge(links []directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	for _, e := range links {
		if l, ok := e.(rockRidgeSymlink); ok {
			d.name += l.name
		}
	}
	d.continued = false
	return d
}

func (r *rockRidgeExtension) parseSymlink(b []byte) (directoryEntrySystemUseExtension, error) {
	size := int(b[2])
	if size != len(b) {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge SL extension received %d bytes, but byte 2 indicated %d", len(b), size)
	}
	version := b[3]
	if version != 1 {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge SL extension must be version 1, was %d", version)
	}
	continued := b[4] == 1
	name := ""
	for i := 5; i < len(b); {
		// make it easier to work with
		b2 := b[i:]
		// find out how many bytes we will read
		flags := b2[0]
		size := b2[1]
		switch {
		case flags&0x1 == 0x1:
			name += "."
		case flags&0x2 == 0x2:
			name += ".."
		case flags&0x3 == 0x3:
			name = "/"
		case size > 0:
			name += "/" + string(b2[2:2+size])
		}

		i += 2 + int(size)
	}
	return rockRidgeSymlink{
		continued: continued,
		name:      name,
	}, nil
}

// rockRidgeName
// a name can be greater than the 254 max size of a SUSP extension, so it may continue across multiple extension entries
// a rockRidgeName can represent the individual components, or an entire set merged together
// Bytes(), when called, will provide as many consecutive name bytes as needed
type rockRidgeName struct {
	continued bool // if this is continued in another rockRidgeName entry
	current   bool // refers to current directory, i.e. "."
	parent    bool // refers to parent directory, i.e. ".."
	name      string
}

func (d rockRidgeName) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(rockRidgeName)
	return ok && t == d
}
func (d rockRidgeName) Signature() string {
	return rockRidgeSignatureName
}
func (d rockRidgeName) Length() int {
	// basic 4 bytes for all SUSP entries, 1 byte for flags, name
	return 4 + 1 + len(d.name)
}
func (d rockRidgeName) Version() uint8 {
	return 1
}
func (d rockRidgeName) Data() []byte {
	return []byte{}
}
func (d rockRidgeName) Bytes() []byte {
	// This could be a single entry, or so long that you need multiple concatenated
	// maximum size of a single entry is 254 bytes
	// each SL record requires 4 bytes of header, 1 byte of flags
	//  so available = 254-(4+1) = 249
	headerSize := 4 + 1
	maxComponentSize := directoryEntryMaxSize - headerSize
	// count how many entries we will need
	nameBytes := []byte(d.name)
	count := len(nameBytes) / maxComponentSize
	if len(nameBytes)%maxComponentSize > 0 {
		count++
	}
	b := make([]byte, 0)
	for i := 0; i < count; i++ {
		b2 := make([]byte, 5)
		copy(b2[0:2], rockRidgeSignatureName)
		// we set size and continuing flag when we are done with this entry
		b2[3] = d.Version()
		copyBytes := nameBytes
		continuing := 0
		if len(nameBytes) > maxComponentSize {
			copyBytes = nameBytes[:maxComponentSize]
			continuing = 1
		}
		b2 = append(b2, copyBytes...)
		b2[2] = 5 + uint8(len(copyBytes))
		flags := 0x0 | continuing
		if d.current {
			flags |= 0x2
		}
		if d.parent {
			flags |= 0x4
		}
		b2[4] = byte(flags)

		b = append(b, b2...)
	}
	return b
}
func (d rockRidgeName) Continuable() bool {
	return d.continued
}
func (d rockRidgeName) Merge(names []directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	for _, e := range names {
		if n, ok := e.(rockRidgeName); ok {
			d.name += n.name
		}
	}
	d.continued = false
	return d
}

func (r *rockRidgeExtension) parseName(b []byte) (directoryEntrySystemUseExtension, error) {
	size := int(b[2])
	if size != len(b) {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge NM extension received %d bytes, but byte 2 indicated %d", len(b), size)
	}
	version := b[3]
	if version != 1 {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge NM extension must be version 1, was %d", version)
	}
	continued := b[4]&1 != 0
	current := b[4]&2 != 0
	parent := b[4]&4 != 0
	name := ""
	if size > 5 {
		name = string(b[5:])
	}
	return rockRidgeName{
		continued: continued,
		name:      name,
		current:   current,
		parent:    parent,
	}, nil
}

// rockRidgeTimestamp constants - these are the bitmasks for the flag field
const (
	rockRidgeTimestampCreation   uint8 = 1
	rockRidgeTimestampModify     uint8 = 2
	rockRidgeTimestampAccess     uint8 = 4
	rockRidgeTimestampAttribute  uint8 = 8
	rockRidgeTimestampBackup     uint8 = 16
	rockRidgeTimestampExpiration uint8 = 32
	rockRidgeTimestampEffective  uint8 = 64
	rockRidgeTimestampLongForm   uint8 = 128
)

// rockRidgeTimestamp
type rockRidgeTimestamp struct {
	timestampType uint8
	time          time.Time
}

func (r rockRidgeTimestamp) Equal(o rockRidgeTimestamp) bool {
	// we compare down to the second, not below
	return r.timestampType == o.timestampType && r.time.Unix() == o.time.Unix()
}
func (r rockRidgeTimestamp) Close(o rockRidgeTimestamp) bool {
	// we compare within 5 seconds
	margin := int64(5)
	diff := r.time.Unix() - o.time.Unix()
	return r.timestampType == o.timestampType && diff < margin && diff > -margin
}

type rockRidgeTimestamps struct {
	longForm bool
	stamps   []rockRidgeTimestamp
}
type rockRidgeTimestampByBitOrder []rockRidgeTimestamp

func (s rockRidgeTimestampByBitOrder) Len() int {
	return len(s)
}
func (s rockRidgeTimestampByBitOrder) Swap(i, j int) {
	s[i], s[j] = s[j], s[i]
}
func (s rockRidgeTimestampByBitOrder) Less(i, j int) bool {
	return s[i].timestampType < s[j].timestampType
}

func (d rockRidgeTimestamps) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(rockRidgeTimestamps)
	matched := ok
	if matched && t.longForm == d.longForm && len(d.stamps) == len(t.stamps) {
		for i, e := range d.stamps {
			if !e.Equal(t.stamps[i]) {
				matched = false
				break
			}
		}
	} else {
		matched = false
	}
	return matched
}
func (d rockRidgeTimestamps) Close(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(rockRidgeTimestamps)
	matched := ok
	if matched && t.longForm == d.longForm && len(d.stamps) == len(t.stamps) {
		for i, e := range d.stamps {
			if !e.Close(t.stamps[i]) {
				matched = false
				break
			}
		}
	} else {
		matched = false
	}
	return matched
}
func (d rockRidgeTimestamps) Signature() string {
	return rockRidgeSignatureTimestamps
}
func (d rockRidgeTimestamps) Length() int {
	entryLength := 7
	if d.longForm {
		entryLength = 17
	}
	return 5 + entryLength*len(d.stamps)
}
func (d rockRidgeTimestamps) Version() uint8 {
	return 1
}
func (d rockRidgeTimestamps) Data() []byte {
	return []byte{}
}
func (d rockRidgeTimestamps) Bytes() []byte {
	b := make([]byte, 5)
	copy(b[0:2], rockRidgeSignatureTimestamps)
	b[2] = uint8(d.Length())
	b[3] = d.Version()
	if d.longForm {
		b[4] |= rockRidgeTimestampLongForm
	}

	// now get all of the timestamps
	// these have to be in a specific order
	//  creation, modify, access, attributes, backup, expiration, effective
	// this is the increasing but order they are in anyways
	sort.Sort(rockRidgeTimestampByBitOrder(d.stamps))
	for _, t := range d.stamps {
		var b2 []byte
		b[4] |= t.timestampType
		if d.longForm {
			b2 = timeToDecBytes(t.time)
		} else {
			b2 = timeToBytes(t.time)
		}

		b = append(b, b2...)
	}
	return b
}
func (d rockRidgeTimestamps) Continuable() bool {
	return false
}
func (d rockRidgeTimestamps) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func (r *rockRidgeExtension) parseTimestamps(b []byte) (directoryEntrySystemUseExtension, error) {
	size := b[2]
	if int(size) != len(b) {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge TF extension has %d bytes, but byte 2 indicated %d", len(b), size)
	}
	version := b[3]
	if version != 1 {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge TF extension must be version 1, was %d", version)
	}
	// what timestamps are recorded?
	flags := b[4]
	// go through them one by one in order
	entryLength := 7
	longForm := false
	if flags&rockRidgeTimestampLongForm != 0 {
		entryLength = 17
		longForm = true
	}

	entries := make([]rockRidgeTimestamp, 0)
	tfBytes := b[5:]
	tfTypes := []uint8{rockRidgeTimestampCreation, rockRidgeTimestampModify, rockRidgeTimestampAccess, rockRidgeTimestampAttribute,
		rockRidgeTimestampBackup, rockRidgeTimestampExpiration, rockRidgeTimestampEffective}
	for _, tf := range tfTypes {
		if flags&tf == 0 {
			continue
		}
		timeBytes := tfBytes[:entryLength]
		tfBytes = tfBytes[entryLength:]
		var (
			t   time.Time
			err error
		)
		if longForm {
			t, err = decBytesToTime(timeBytes)
			if err != nil {
				return nil, fmt.Errorf("could not process timestamp %d bytes to long form bytes: %v % x", tf, err, timeBytes)
			}
		} else {
			t = bytesToTime(timeBytes)
		}
		entry := rockRidgeTimestamp{
			time:          t,
			timestampType: tf,
		}
		entries = append(entries, entry)
	}

	return rockRidgeTimestamps{
		stamps:   entries,
		longForm: longForm,
	}, nil
}

// rockRidgeSparseFile
type rockRidgeSparseFile struct {
	length     int
	high       uint32
	low        uint32
	tableDepth uint8
}

func (d rockRidgeSparseFile) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(rockRidgeSparseFile)
	return ok && t == d
}
func (d rockRidgeSparseFile) Signature() string {
	return rockRidgeSignatureSparseFile
}
func (d rockRidgeSparseFile) Length() int {
	return d.length
}
func (d rockRidgeSparseFile) Version() uint8 {
	return 1
}
func (d rockRidgeSparseFile) Data() []byte {
	return []byte{}
}
func (d rockRidgeSparseFile) Bytes() []byte {
	b := make([]byte, d.length)
	copy(b[0:2], rockRidgeSignatureSparseFile)
	b[2] = uint8(d.Length())
	b[3] = d.Version()

	binary.LittleEndian.PutUint32(b[4:8], d.high)
	binary.BigEndian.PutUint32(b[8:12], d.high)
	if d.length == 21 {
		binary.LittleEndian.PutUint32(b[12:16], d.low)
		binary.BigEndian.PutUint32(b[16:20], d.low)
		b[20] = d.tableDepth
	}

	return b
}
func (d rockRidgeSparseFile) Continuable() bool {
	return false
}
func (d rockRidgeSparseFile) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func (r *rockRidgeExtension) parseSparseFile(b []byte) (directoryEntrySystemUseExtension, error) {
	targetSize := r.sfLength
	if len(b) != targetSize {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge SF extension must be %d bytes, but received %d", targetSize, len(b))
	}
	size := b[2]
	if size != uint8(targetSize) {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge SF extension must be %d bytes, but byte 2 indicated %d", targetSize, size)
	}
	version := b[3]
	if version != 1 {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge SF extension must be version 1, was %d", version)
	}
	sf := &rockRidgeSparseFile{
		high:   binary.LittleEndian.Uint32(b[4:8]),
		length: targetSize,
	}
	if targetSize == 21 {
		sf.low = binary.LittleEndian.Uint32(b[12:16])
		sf.tableDepth = b[20]
	}
	return sf, nil
}

// rockRidgeChildDirectory
type rockRidgeChildDirectory struct {
	location uint32
}

func (d rockRidgeChildDirectory) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(rockRidgeChildDirectory)
	return ok && t == d
}
func (d rockRidgeChildDirectory) Signature() string {
	return rockRidgeSignatureChild
}
func (d rockRidgeChildDirectory) Length() int {
	return 12
}
func (d rockRidgeChildDirectory) Version() uint8 {
	return 1
}
func (d rockRidgeChildDirectory) Data() []byte {
	return []byte{}
}
func (d rockRidgeChildDirectory) Bytes() []byte {
	b := make([]byte, 12)
	copy(b[0:2], rockRidgeSignatureChild)
	b[2] = uint8(d.Length())
	b[3] = d.Version()
	binary.LittleEndian.PutUint32(b[4:8], d.location)
	binary.BigEndian.PutUint32(b[8:12], d.location)
	return b
}
func (d rockRidgeChildDirectory) Continuable() bool {
	return false
}
func (d rockRidgeChildDirectory) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func (r *rockRidgeExtension) parseChildDirectory(b []byte) (directoryEntrySystemUseExtension, error) {
	targetSize := 12
	if len(b) != targetSize {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge CL extension must be %d bytes, but received %d", targetSize, len(b))
	}
	size := b[2]
	if size != uint8(targetSize) {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge CL extension must be %d bytes, but byte 2 indicated %d", targetSize, size)
	}
	version := b[3]
	if version != 1 {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge CL extension must be version 1, was %d", version)
	}
	return rockRidgeChildDirectory{
		location: binary.LittleEndian.Uint32(b[4:8]),
	}, nil
}

// rockRidgeParentDirectory
type rockRidgeParentDirectory struct {
	location uint32
}

func (d rockRidgeParentDirectory) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(rockRidgeParentDirectory)
	return ok && t == d
}
func (d rockRidgeParentDirectory) Signature() string {
	return rockRidgeSignatureParent
}
func (d rockRidgeParentDirectory) Length() int {
	return 12
}
func (d rockRidgeParentDirectory) Version() uint8 {
	return 1
}
func (d rockRidgeParentDirectory) Data() []byte {
	return []byte{}
}
func (d rockRidgeParentDirectory) Bytes() []byte {
	b := make([]byte, 12)
	copy(b[0:2], rockRidgeSignatureParent)
	b[2] = uint8(d.Length())
	b[3] = d.Version()
	binary.LittleEndian.PutUint32(b[4:8], d.location)
	binary.BigEndian.PutUint32(b[8:12], d.location)
	return b
}
func (d rockRidgeParentDirectory) Continuable() bool {
	return false
}
func (d rockRidgeParentDirectory) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func (r *rockRidgeExtension) parseParentDirectory(b []byte) (directoryEntrySystemUseExtension, error) {
	targetSize := 12
	if len(b) != targetSize {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge PL extension must be %d bytes, but received %d", targetSize, len(b))
	}
	size := b[2]
	if size != uint8(targetSize) {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge PL extension must be %d bytes, but byte 2 indicated %d", targetSize, size)
	}
	version := b[3]
	if version != 1 {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge PL extension must be version 1, was %d", version)
	}
	return rockRidgeParentDirectory{
		location: binary.LittleEndian.Uint32(b[4:8]),
	}, nil
}

// rockRidgeRelocatedDirectory
type rockRidgeRelocatedDirectory struct {
}

func (d rockRidgeRelocatedDirectory) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(rockRidgeRelocatedDirectory)
	return ok && t == d
}
func (d rockRidgeRelocatedDirectory) Signature() string {
	return rockRidgeSignatureRelocatedDirectory
}
func (d rockRidgeRelocatedDirectory) Length() int {
	return 8
}
func (d rockRidgeRelocatedDirectory) Version() uint8 {
	return 1
}
func (d rockRidgeRelocatedDirectory) Data() []byte {
	return []byte{}
}
func (d rockRidgeRelocatedDirectory) Bytes() []byte {
	b := make([]byte, 8)
	copy(b[0:2], rockRidgeSignatureRelocatedDirectory)
	b[2] = uint8(d.Length())
	b[3] = d.Version()
	return b
}
func (d rockRidgeRelocatedDirectory) Continuable() bool {
	return false
}
func (d rockRidgeRelocatedDirectory) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func (r *rockRidgeExtension) parseRelocatedDirectory(b []byte) (directoryEntrySystemUseExtension, error) {
	targetSize := 4
	if len(b) != targetSize {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge RE extension must be %d bytes, but received %d", targetSize, len(b))
	}
	size := b[2]
	if size != uint8(targetSize) {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge RE extension must be %d bytes, but byte 2 indicated %d", targetSize, size)
	}
	version := b[3]
	if version != 1 {
		//nolint:stylecheck // "Rock Ridge" is a proper noun
		return nil, fmt.Errorf("Rock Ridge RE extension must be version 1, was %d", version)
	}
	return rockRidgeRelocatedDirectory{}, nil
}
