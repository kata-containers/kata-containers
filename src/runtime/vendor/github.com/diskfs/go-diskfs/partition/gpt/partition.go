package gpt

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"io"
	"reflect"
	"strings"
	"unicode/utf16"

	"github.com/diskfs/go-diskfs/backend"
	uuid "github.com/google/uuid"
)

// PartitionEntrySize fixed size of a GPT partition entry
const PartitionEntrySize = 128

var zeroUUIDBytes = make([]byte, 16)

// Partition represents the structure of a single partition on the disk
type Partition struct {
	Start              uint64 // start sector for the partition
	End                uint64 // end sector for the partition
	Size               uint64 // size of the partition in bytes
	Type               Type   // parttype for the partition
	Name               string // name for the partition
	GUID               string // partition GUID, can be left blank to auto-generate
	Attributes         uint64 // Attributes flags
	logicalSectorSize  int
	physicalSectorSize int
}

func reverseSlice(s interface{}) {
	size := reflect.ValueOf(s).Len()
	swap := reflect.Swapper(s)
	for i, j := 0, size-1; i < j; i, j = i+1, j-1 {
		swap(i, j)
	}
}

// toBytes return the 128 bytes for this partition
func (p *Partition) toBytes() ([]byte, error) {
	b := make([]byte, PartitionEntrySize)

	// if the Type is Unused, just return all zeroes
	if p.Type == Unused {
		return b, nil
	}

	// partition type GUID is first 16 bytes
	typeGUID, err := uuid.Parse(string(p.Type))
	if err != nil {
		return nil, fmt.Errorf("unable to parse partition type GUID: %v", err)
	}
	copy(b[0:16], bytesToUUIDBytes(typeGUID[0:16]))

	// partition identifier GUID is next 16 bytes
	idGUID, err := uuid.Parse(p.GUID)
	if err != nil {
		return nil, fmt.Errorf("unable to parse partition identifier GUID: %v", err)
	}
	copy(b[16:32], bytesToUUIDBytes(idGUID[0:16]))

	// next is first LBA and last LBA, uint64 = 8 bytes each
	binary.LittleEndian.PutUint64(b[32:40], p.Start)
	binary.LittleEndian.PutUint64(b[40:48], p.End)
	binary.LittleEndian.PutUint64(b[48:56], p.Attributes)

	// now the partition name - it is UTF16LE encoded, max 36 code units for 72 bytes
	r := make([]rune, 0, len(p.Name))
	// first convert to runes
	for _, s := range p.Name {
		r = append(r, s)
	}
	if len(r) > 36 {
		return nil, fmt.Errorf("cannot use %s as partition name, has %d Unicode code units, maximum size is 36", p.Name, len(r))
	}
	// next convert the runes to uint16
	nameb := utf16.Encode(r)
	// and then convert to little-endian bytes
	for i, u := range nameb {
		pos := 56 + i*2
		binary.LittleEndian.PutUint16(b[pos:pos+2], u)
	}

	return b, nil
}

// FromBytes create a partition entry from bytes
func partitionFromBytes(b []byte, logicalSectorSize, physicalSectorSize int) (*Partition, error) {
	if len(b) != PartitionEntrySize {
		return nil, fmt.Errorf("data for partition was %d bytes instead of expected %d", len(b), PartitionEntrySize)
	}
	// is it all zeroes?
	if bytes.Equal(b[0:16], zeroUUIDBytes) {
		return nil, nil
	}
	typeGUID, err := uuid.FromBytes(bytesToUUIDBytes(b[0:16]))
	if err != nil {
		return nil, fmt.Errorf("unable to read partition type GUID: %v", err)
	}
	typeString := typeGUID.String()
	uid, err := uuid.FromBytes(bytesToUUIDBytes(b[16:32]))
	if err != nil {
		return nil, fmt.Errorf("unable to read partition identifier GUID: %v", err)
	}
	firstLBA := binary.LittleEndian.Uint64(b[32:40])
	lastLBA := binary.LittleEndian.Uint64(b[40:48])
	attribs := binary.LittleEndian.Uint64(b[48:56])

	// get the partition name
	nameb := b[56:]
	u := make([]uint16, 0, 72)
	for i := 0; i < len(nameb); i += 2 {
		// strip any 0s off of the end
		entry := binary.LittleEndian.Uint16(nameb[i : i+2])
		if entry == 0 {
			break
		}
		u = append(u, entry)
	}
	r := utf16.Decode(u)
	name := string(r)

	return &Partition{
		Start:              firstLBA,
		End:                lastLBA,
		Name:               name,
		GUID:               strings.ToUpper(uid.String()),
		Attributes:         attribs,
		Type:               Type(strings.ToUpper(typeString)),
		logicalSectorSize:  logicalSectorSize,
		physicalSectorSize: physicalSectorSize,
	}, nil
}

func (p *Partition) GetSize() int64 {
	// size already is in Bytes
	return int64(p.Size)
}

func (p *Partition) GetStart() int64 {
	_, lss := p.sectorSizes()
	return int64(p.Start) * int64(lss)
}

// WriteContents fills the partition with the contents provided
// reads from beginning of reader to exactly size of partition in bytes
func (p *Partition) WriteContents(f backend.WritableFile, contents io.Reader) (uint64, error) {
	pss, lss := p.sectorSizes()
	total := uint64(0)
	// validate start/end/size
	calculatedSize := (p.End - p.Start + 1) * uint64(lss)
	switch {
	case p.Size > 0 && p.Size == calculatedSize:
		// all is good
	case p.Size == 0 && p.End >= p.Start:
		// Size was not set
		p.Size = calculatedSize
	case p.Size > 0 && p.Size%uint64(lss) == 0 && p.End == 0:
		// End was not set
		p.End = p.Start + p.Size/uint64(lss) - 1
	default:
		return total, fmt.Errorf("cannot reconcile partition size %d with start %d / end %d", p.Size, p.Start, p.End)
	}

	// chunks of physical sector size for efficient writing
	b := make([]byte, pss)
	// we start at the correct byte location
	start := p.Start * uint64(lss)
	// loop in physical sector sizes
	for {
		read, err := contents.Read(b)
		if err != nil && err != io.EOF {
			return total, fmt.Errorf("could not read contents to pass to partition: %v", err)
		}
		tmpTotal := uint64(read) + total
		if tmpTotal > p.Size {
			return total, fmt.Errorf("requested to write at least %d bytes to partition but maximum size is %d", tmpTotal, p.Size)
		}
		if read > 0 {
			var written int
			written, err = f.WriteAt(b[:read], int64(start+total))
			if err != nil {
				return total, fmt.Errorf("error writing to file: %v", err)
			}
			total += uint64(written)
		}
		// increment our total
		// is this the end of the data?
		if err == io.EOF {
			break
		}
	}
	// did the total written equal the size of the partition?
	if total != p.Size {
		return total, fmt.Errorf("write %d bytes to partition but actual size is %d", total, p.Size)
	}
	return total, nil
}

// ReadContents reads the contents of the partition into a writer
// streams the entire partition to the writer
func (p *Partition) ReadContents(f backend.File, out io.Writer) (int64, error) {
	pss, _ := p.sectorSizes()
	total := int64(0)
	// chunks of physical sector size for efficient writing
	b := make([]byte, pss)
	// we start at the correct byte location
	start := p.GetStart()
	size := p.GetSize()

	// loop in physical sector sizes
	for {
		read, err := f.ReadAt(b, start+total)
		if err != nil && err != io.EOF {
			return total, fmt.Errorf("error reading from file: %v", err)
		}
		if read > 0 {
			_, _ = out.Write(b[:read])
		}
		// increment our total
		total += int64(read)
		// is this the end of the data?
		if err == io.EOF || total >= size {
			break
		}
	}
	return total, nil
}

// initEntry adjust the Start/End/Size entries and ensure it has a GUID
func (p *Partition) initEntry(blocksize, starting uint64) error {
	part := p
	if part.Type == Unused {
		return nil
	}
	var guid uuid.UUID

	if part.GUID == "" {
		guid, _ = uuid.NewRandom()
	} else {
		var err error
		guid, err = uuid.Parse(part.GUID)
		if err != nil {
			return fmt.Errorf("invalid UUID: %s", part.GUID)
		}
	}
	part.GUID = strings.ToUpper(guid.String())

	// check size matches sectors
	// valid possibilities:
	// 1- size=0, start>=0, end>start - valid - begin at start, go until end
	// 2- size>0, start>=0, end=0 - valid - begin at start for size bytes
	// 3- size>0, start=0, end=0 - valid - begin at end of previous partition, go for size bytes
	// anything else is an error
	size, start, end := part.Size, part.Start, part.End
	calculatedSize := (end - start + 1) * blocksize
	switch {
	case end >= start && size == calculatedSize:
	case size == 0 && end >= start:
		// provided specific start and end, so calculate size
		part.Size = calculatedSize
	case size > 0 && size%blocksize == 0 && start > 0 && end == 0:
		// provided specific start and size, so calculate end
		part.End = start + size/blocksize - 1
	case size > 0 && size%blocksize == 0 && start == 0 && end == 0:
		// we start right after the end of the previous
		start = starting
		end = start + size/blocksize - 1
		part.Start = start
		part.End = end
	default:
		return fmt.Errorf("invalid partition entry, size %d bytes does not match start sector %d and end sector %d", size, start, end)
	}
	return nil
}

func (p *Partition) sectorSizes() (physical, logical int) {
	physical, logical = p.physicalSectorSize, p.logicalSectorSize
	if physical == 0 {
		physical = physicalSectorSize
	}
	if logical == 0 {
		logical = logicalSectorSize
	}
	return physical, logical
}

func (p *Partition) Equal(o *Partition) bool {
	return p != nil && o != nil && *p == *o
}

// UUID returns the partitions UUID
func (p *Partition) UUID() string {
	return p.GUID
}

// Expand increases the size of the partition by a number of sectors
func (p *Partition) Expand(sectors uint64) {
	p.End += sectors
	p.Size += sectors * uint64(p.logicalSectorSize)
}
