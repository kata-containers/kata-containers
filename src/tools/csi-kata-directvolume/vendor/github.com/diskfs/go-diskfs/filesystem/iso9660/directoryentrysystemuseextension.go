package iso9660

import (
	"bytes"
	"encoding/binary"
	"errors"
	"fmt"
)

const (
	suspExtensionContinuationArea          = "CE"
	suspExtensionPaddingField              = "PD"
	suspExtensionSharingProtocolIndicator  = "SP"
	suspExtensionSharingProtocolTerminator = "ST"
	suspExtensionExtensionsReference       = "ER"
	suspExtensionExtensionsSelector        = "ES"
	suspExtensionCheckBytes                = 0xbeef
)

var (
	// ErrSuspNoHandler error to show gracefully that we do not have a handler for this signature. Opposed to processing error
	ErrSuspNoHandler = errors.New("NoHandler")
	// ErrSuspFilenameUnsupported error to show that this extension does not support searching by path
	ErrSuspFilenameUnsupported = errors.New("FilenameUnsupported")
	// ErrSuspRelocatedDirectoryUnsupported error to indicate that this extension does not support relocated directories
	ErrSuspRelocatedDirectoryUnsupported = errors.New("relocatedDirectoryUnsupported")
)

// suspExtension master for an extension that is registered with an "ER" entry
type suspExtension interface {
	ID() string
	Process(string, []byte) (directoryEntrySystemUseExtension, error)
	GetFilename(*directoryEntry) (string, error)
	Relocated(*directoryEntry) bool
	UsePathtable() bool
	GetDirectoryLocation(*directoryEntry) uint32
	Descriptor() string
	Source() string
	Version() uint8
	GetFileExtensions(string, bool, bool) ([]directoryEntrySystemUseExtension, error)
	GetFinalizeExtensions(*finalizeFileInfo) ([]directoryEntrySystemUseExtension, error)
	Relocatable() bool
	Relocate(map[string]*finalizeFileInfo) ([]*finalizeFileInfo, map[string]*finalizeFileInfo, error)
}

type directoryEntrySystemUseExtension interface {
	Equal(directoryEntrySystemUseExtension) bool
	Signature() string
	Length() int
	Version() uint8
	Data() []byte
	Bytes() []byte
	Continuable() bool                                                         // if this one is continuable to the next one of the same signature
	Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension // merge
}

// directoryEntrySystemUseExtensionRaw raw holder, common to all
type directoryEntrySystemUseExtensionRaw struct {
	signature string
	length    uint8
	version   uint8
	data      []byte
}

func (d directoryEntrySystemUseExtensionRaw) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(directoryEntrySystemUseExtensionRaw)
	return ok && t.signature == d.signature && t.length == d.length && t.version == d.version && bytes.Equal(d.data, t.data)
}
func (d directoryEntrySystemUseExtensionRaw) Signature() string {
	return d.signature
}
func (d directoryEntrySystemUseExtensionRaw) Length() int {
	return int(d.length)
}
func (d directoryEntrySystemUseExtensionRaw) Version() uint8 {
	return d.version
}
func (d directoryEntrySystemUseExtensionRaw) Data() []byte {
	return d.data
}
func (d directoryEntrySystemUseExtensionRaw) Bytes() []byte {
	ret := make([]byte, 4)
	copy(ret[0:2], d.Signature())
	ret[2] = d.length
	ret[3] = d.Version()
	ret = append(ret, d.Data()...)
	return ret
}
func (d directoryEntrySystemUseExtensionRaw) Continuable() bool {
	return false
}
func (d directoryEntrySystemUseExtensionRaw) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func parseSystemUseExtensionRaw(b []byte) directoryEntrySystemUseExtension {
	size := len(b)
	signature := string(b[:2])
	version := b[3]
	data := make([]byte, 0)
	if size > 4 {
		data = b[4:]
	}
	return directoryEntrySystemUseExtensionRaw{
		signature: signature,
		length:    uint8(size),
		version:   version,
		data:      data,
	}
}

// directoryEntrySystemUseExtensionSharingProtocolIndicator single appearance in root entry
type directoryEntrySystemUseExtensionSharingProtocolIndicator struct {
	skipBytes uint8
}

func (d directoryEntrySystemUseExtensionSharingProtocolIndicator) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(directoryEntrySystemUseExtensionSharingProtocolIndicator)
	return ok && t == d
}
func (d directoryEntrySystemUseExtensionSharingProtocolIndicator) Signature() string {
	return suspExtensionSharingProtocolIndicator
}
func (d directoryEntrySystemUseExtensionSharingProtocolIndicator) Length() int {
	return 7
}
func (d directoryEntrySystemUseExtensionSharingProtocolIndicator) Version() uint8 {
	return 1
}
func (d directoryEntrySystemUseExtensionSharingProtocolIndicator) Data() []byte {
	ret := make([]byte, 3)
	binary.BigEndian.PutUint16(ret[0:2], suspExtensionCheckBytes)
	ret[2] = d.skipBytes
	return ret
}
func (d directoryEntrySystemUseExtensionSharingProtocolIndicator) Bytes() []byte {
	ret := make([]byte, 4)
	copy(ret[0:2], suspExtensionSharingProtocolIndicator)
	ret[2] = uint8(d.Length())
	ret[3] = d.Version()
	ret = append(ret, d.Data()...)
	return ret
}
func (d directoryEntrySystemUseExtensionSharingProtocolIndicator) SkipBytes() uint8 {
	return d.skipBytes
}
func (d directoryEntrySystemUseExtensionSharingProtocolIndicator) Continuable() bool {
	return false
}
func (d directoryEntrySystemUseExtensionSharingProtocolIndicator) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func parseSystemUseExtensionSharingProtocolIndicator(b []byte) (directoryEntrySystemUseExtension, error) {
	targetSize := 7
	if len(b) != targetSize {
		return nil, fmt.Errorf("SP extension must be %d bytes, but received %d", targetSize, len(b))
	}
	size := b[2]
	if size != uint8(targetSize) {
		return nil, fmt.Errorf("SP extension must be %d bytes, but byte 2 indicated %d", targetSize, size)
	}
	version := b[3]
	if version != 1 {
		return nil, fmt.Errorf("SP extension must be version 1, was %d", version)
	}
	checkBytes := binary.BigEndian.Uint16(b[4:6])
	if checkBytes != suspExtensionCheckBytes {
		return nil, fmt.Errorf("SP extension must had mismatched check bytes, received % x instead of % x", checkBytes, suspExtensionCheckBytes)
	}
	return directoryEntrySystemUseExtensionSharingProtocolIndicator{
		skipBytes: b[6],
	}, nil
}

// directoryEntrySystemUseExtensionPadding padding
type directoryEntrySystemUseExtensionPadding struct {
	length uint8
}

func (d directoryEntrySystemUseExtensionPadding) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(directoryEntrySystemUseExtensionPadding)
	return ok && t == d
}
func (d directoryEntrySystemUseExtensionPadding) Signature() string {
	return suspExtensionPaddingField
}
func (d directoryEntrySystemUseExtensionPadding) Length() int {
	return int(d.length)
}
func (d directoryEntrySystemUseExtensionPadding) Version() uint8 {
	return 1
}
func (d directoryEntrySystemUseExtensionPadding) Data() []byte {
	ret := make([]byte, d.Length()-4)
	return ret
}
func (d directoryEntrySystemUseExtensionPadding) Bytes() []byte {
	ret := make([]byte, 4)
	copy(ret[0:2], suspExtensionPaddingField)
	ret[2] = d.length
	ret[3] = d.Version()
	ret = append(ret, d.Data()...)
	return ret
}
func (d directoryEntrySystemUseExtensionPadding) Continuable() bool {
	return false
}
func (d directoryEntrySystemUseExtensionPadding) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func parseSystemUseExtensionPadding(b []byte) (directoryEntrySystemUseExtension, error) {
	size := b[2]
	if int(size) != len(b) {
		return nil, fmt.Errorf("PD extension received %d bytes, but byte 2 indicated %d", len(b), size)
	}
	version := b[3]
	if version != 1 {
		return nil, fmt.Errorf("PD extension must be version 1, was %d", version)
	}
	return directoryEntrySystemUseExtensionPadding{
		length: size,
	}, nil
}

// directoryEntrySystemUseTerminator termination
type directoryEntrySystemUseTerminator struct {
}

func (d directoryEntrySystemUseTerminator) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(directoryEntrySystemUseTerminator)
	return ok && t == d
}
func (d directoryEntrySystemUseTerminator) Signature() string {
	return suspExtensionSharingProtocolTerminator
}
func (d directoryEntrySystemUseTerminator) Length() int {
	return 4
}
func (d directoryEntrySystemUseTerminator) Version() uint8 {
	return 1
}
func (d directoryEntrySystemUseTerminator) Data() []byte {
	return []byte{}
}
func (d directoryEntrySystemUseTerminator) Bytes() []byte {
	ret := make([]byte, 4)
	copy(ret[0:2], suspExtensionSharingProtocolTerminator)
	ret[2] = uint8(d.Length())
	ret[3] = d.Version()
	return ret
}
func (d directoryEntrySystemUseTerminator) Continuable() bool {
	return false
}
func (d directoryEntrySystemUseTerminator) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func parseSystemUseExtensionTerminator(b []byte) (directoryEntrySystemUseExtension, error) {
	targetSize := 4
	if len(b) != targetSize {
		return nil, fmt.Errorf("ST extension must be %d bytes, but received %d", targetSize, len(b))
	}
	size := b[2]
	if size != uint8(targetSize) {
		return nil, fmt.Errorf("ST extension must be %d bytes, but byte 2 indicated %d", targetSize, size)
	}
	version := b[3]
	if version != 1 {
		return nil, fmt.Errorf("ST extension must be version 1, was %d", version)
	}
	return directoryEntrySystemUseTerminator{}, nil
}

// directoryEntrySystemUseContinuation termination
type directoryEntrySystemUseContinuation struct {
	location           uint32
	offset             uint32
	continuationLength uint32
}

func (d directoryEntrySystemUseContinuation) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(directoryEntrySystemUseContinuation)
	return ok && t == d
}
func (d directoryEntrySystemUseContinuation) Signature() string {
	return suspExtensionContinuationArea
}
func (d directoryEntrySystemUseContinuation) Length() int {
	return 28
}
func (d directoryEntrySystemUseContinuation) Version() uint8 {
	return 1
}
func (d directoryEntrySystemUseContinuation) Data() []byte {
	b := make([]byte, 24)
	binary.LittleEndian.PutUint32(b[0:4], d.location)
	binary.BigEndian.PutUint32(b[4:8], d.location)
	binary.LittleEndian.PutUint32(b[8:12], d.offset)
	binary.BigEndian.PutUint32(b[12:16], d.offset)
	binary.LittleEndian.PutUint32(b[16:20], d.continuationLength)
	binary.BigEndian.PutUint32(b[20:24], d.continuationLength)
	return b
}
func (d directoryEntrySystemUseContinuation) Bytes() []byte {
	ret := make([]byte, 4)
	copy(ret[0:2], suspExtensionContinuationArea)
	ret[2] = uint8(d.Length())
	ret[3] = d.Version()
	ret = append(ret, d.Data()...)
	return ret
}
func (d directoryEntrySystemUseContinuation) Location() uint32 {
	return d.location
}
func (d directoryEntrySystemUseContinuation) Offset() uint32 {
	return d.offset
}
func (d directoryEntrySystemUseContinuation) ContinuationLength() uint32 {
	return d.continuationLength
}
func (d directoryEntrySystemUseContinuation) Continuable() bool {
	return false
}
func (d directoryEntrySystemUseContinuation) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func parseSystemUseExtensionContinuationArea(b []byte) (directoryEntrySystemUseExtension, error) {
	targetSize := 28
	if len(b) != targetSize {
		return nil, fmt.Errorf("CE extension must be %d bytes, but received %d", targetSize, len(b))
	}
	size := b[2]
	if size != uint8(targetSize) {
		return nil, fmt.Errorf("CE extension must be %d bytes, but byte 2 indicated %d", targetSize, size)
	}
	version := b[3]
	if version != 1 {
		return nil, fmt.Errorf("CE extension must be version 1, was %d", version)
	}
	location := binary.LittleEndian.Uint32(b[4:8])
	offset := binary.LittleEndian.Uint32(b[12:16])
	continuationLength := binary.LittleEndian.Uint32(b[20:24])
	return directoryEntrySystemUseContinuation{
		location:           location,
		offset:             offset,
		continuationLength: continuationLength,
	}, nil
}

// directoryEntrySystemUseExtensionSelector termination
type directoryEntrySystemUseExtensionSelector struct {
	sequence uint8
}

func (d directoryEntrySystemUseExtensionSelector) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(directoryEntrySystemUseExtensionSelector)
	return ok && t == d
}
func (d directoryEntrySystemUseExtensionSelector) Signature() string {
	return suspExtensionExtensionsSelector
}
func (d directoryEntrySystemUseExtensionSelector) Length() int {
	return 5
}
func (d directoryEntrySystemUseExtensionSelector) Version() uint8 {
	return 1
}
func (d directoryEntrySystemUseExtensionSelector) Data() []byte {
	return []byte{d.sequence}
}
func (d directoryEntrySystemUseExtensionSelector) Bytes() []byte {
	ret := make([]byte, 4)
	copy(ret[0:2], suspExtensionExtensionsSelector)
	ret[2] = uint8(d.Length())
	ret[3] = d.Version()
	ret = append(ret, d.Data()...)
	return ret
}
func (d directoryEntrySystemUseExtensionSelector) Sequence() uint8 {
	return d.sequence
}
func (d directoryEntrySystemUseExtensionSelector) Continuable() bool {
	return false
}
func (d directoryEntrySystemUseExtensionSelector) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func parseSystemUseExtensionExtensionsSelector(b []byte) (directoryEntrySystemUseExtension, error) {
	targetSize := 5
	if len(b) != targetSize {
		return nil, fmt.Errorf("ES extension must be %d bytes, but received %d", targetSize, len(b))
	}
	size := b[2]
	if size != uint8(targetSize) {
		return nil, fmt.Errorf("ES extension must be %d bytes, but byte 2 indicated %d", targetSize, size)
	}
	version := b[3]
	if version != 1 {
		return nil, fmt.Errorf("ES extension must be version 1, was %d", version)
	}
	sequence := b[4]
	return directoryEntrySystemUseExtensionSelector{
		sequence: sequence,
	}, nil
}

// directoryEntrySystemUseExtensionReference termination
type directoryEntrySystemUseExtensionReference struct {
	id               string
	descriptor       string
	source           string
	extensionVersion uint8
}

func (d directoryEntrySystemUseExtensionReference) Equal(o directoryEntrySystemUseExtension) bool {
	t, ok := o.(directoryEntrySystemUseExtensionReference)
	return ok && t == d
}
func (d directoryEntrySystemUseExtensionReference) Signature() string {
	return suspExtensionExtensionsReference
}
func (d directoryEntrySystemUseExtensionReference) Length() int {
	return 8 + len(d.id) + len(d.descriptor) + len(d.source)
}
func (d directoryEntrySystemUseExtensionReference) Version() uint8 {
	return 1
}
func (d directoryEntrySystemUseExtensionReference) Data() []byte {
	ret := make([]byte, 4)
	ret[0] = uint8(len(d.id))
	ret[1] = uint8(len(d.descriptor))
	ret[2] = uint8(len(d.source))
	ret[3] = d.extensionVersion
	ret = append(ret, []byte(d.id)...)
	ret = append(ret, []byte(d.descriptor)...)
	ret = append(ret, []byte(d.source)...)
	return ret
}
func (d directoryEntrySystemUseExtensionReference) Bytes() []byte {
	ret := make([]byte, 4)
	copy(ret[0:2], suspExtensionExtensionsReference)
	ret[2] = uint8(d.Length())
	ret[3] = d.Version()
	ret = append(ret, d.Data()...)
	return ret
}
func (d directoryEntrySystemUseExtensionReference) ExtensionVersion() uint8 {
	return d.extensionVersion
}
func (d directoryEntrySystemUseExtensionReference) ExtensionID() string {
	return d.id
}
func (d directoryEntrySystemUseExtensionReference) ExtensionDescriptor() string {
	return d.descriptor
}
func (d directoryEntrySystemUseExtensionReference) ExtensionSource() string {
	return d.source
}
func (d directoryEntrySystemUseExtensionReference) Continuable() bool {
	return false
}
func (d directoryEntrySystemUseExtensionReference) Merge([]directoryEntrySystemUseExtension) directoryEntrySystemUseExtension {
	return nil
}

func parseSystemUseExtensionExtensionsReference(b []byte) (directoryEntrySystemUseExtension, error) {
	size := b[2]
	if len(b) != int(size) {
		return nil, fmt.Errorf("ER extension byte 2 indicated size of %d bytes, but received %d", size, len(b))
	}
	version := b[3]
	if version != 1 {
		return nil, fmt.Errorf("EE extension must be version 1, was %d", version)
	}
	idSize := int(b[4])
	descriptorSize := int(b[5])
	sourceSize := int(b[6])
	extVersion := b[7]
	idStart := 8
	descriptorStart := 8 + idSize
	sourceStart := 8 + idSize + descriptorSize
	id := string(b[idStart : idStart+idSize])
	descriptor := string(b[descriptorStart : descriptorStart+descriptorSize])
	source := string(b[sourceStart : sourceStart+sourceSize])
	return directoryEntrySystemUseExtensionReference{
		id:               id,
		descriptor:       descriptor,
		source:           source,
		extensionVersion: extVersion,
	}, nil
}

var suspExtensionParser = map[string]func([]byte) (directoryEntrySystemUseExtension, error){
	// base extensions
	suspExtensionSharingProtocolIndicator:  parseSystemUseExtensionSharingProtocolIndicator,
	suspExtensionSharingProtocolTerminator: parseSystemUseExtensionTerminator,
	suspExtensionExtensionsSelector:        parseSystemUseExtensionExtensionsSelector,
	suspExtensionExtensionsReference:       parseSystemUseExtensionExtensionsReference,
	suspExtensionPaddingField:              parseSystemUseExtensionPadding,
	suspExtensionContinuationArea:          parseSystemUseExtensionContinuationArea,
}

// parseDirectoryEntryExtensions parse system use extensions area of a directory entry
func parseDirectoryEntryExtensions(b []byte, handlers []suspExtension) ([]directoryEntrySystemUseExtension, error) {
	// and now for extensions in the system use area
	entries := make([]directoryEntrySystemUseExtension, 0)
	lastEntryBySignature := map[string]directoryEntrySystemUseExtension{}
	// minimum size of 4 bytes for any SUSP entry
	for i := 0; i+4 < len(b); {
		// get the indicator
		signature := string(b[i : i+2])
		size := b[i+2]
		suspBytes := b[i : i+int(size)]
		var (
			entry directoryEntrySystemUseExtension
			err   error
		)
		// if we have a parser, use it, else use the raw parser
		if parser, ok := suspExtensionParser[signature]; ok {
			entry, err = parser(suspBytes)
			if err != nil {
				return nil, fmt.Errorf("error parsing %s extension at byte position %d: %v", signature, i, err)
			}
		} else {
			// go through each extension we have and see if it can process
			for _, ext := range handlers {
				entry, err = ext.Process(signature, suspBytes)
				if err != nil && err != ErrSuspNoHandler {
					return nil, fmt.Errorf("SUSP Extension handler %s error processing extension %s: %v", ext.ID(), signature, err)
				}
				if err == nil {
					break
				}
			}
			if entry == nil {
				entry = parseSystemUseExtensionRaw(suspBytes)
			}
		}
		// we now have the entry - see if there was a prior continuable one
		if last, ok := lastEntryBySignature[signature]; ok {
			entry = last.Merge([]directoryEntrySystemUseExtension{entry})
			if entry.Continuable() {
				lastEntryBySignature[signature] = entry
			} else {
				delete(lastEntryBySignature, signature)
			}
		}

		entries = append(entries, entry)
		i += int(size)
	}
	return entries, nil
}
