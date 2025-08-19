package squashfs

import (
	"encoding/binary"
	"fmt"
	"io"
)

type metadatablock struct {
	compressed bool
	data       []byte
}

const (
	minMetadataBlockSize      = 3
	standardMetadataBlocksize = 8192
)

func getMetadataSize(b []byte) (size uint16, compressed bool, err error) {
	if len(b) < 2 {
		return 0, false, fmt.Errorf("cannot read size of metadata block with %d bytes, must have minimum %d", len(b), 2)
	}
	header := binary.LittleEndian.Uint16(b[:2])
	size = header & 0x7fff
	compressed = header&0x8000 != 0x8000
	return size, compressed, nil
}

func parseMetadata(b []byte, c Compressor) (block *metadatablock, err error) {
	if len(b) < minMetadataBlockSize {
		return nil, fmt.Errorf("metadata block was of len %d, less than minimum %d", len(b), minMetadataBlockSize)
	}
	size, compressed, err := getMetadataSize(b[:2])
	if err != nil {
		return nil, fmt.Errorf("error reading metadata header: %v", err)
	}
	if len(b) < int(2+size) {
		return nil, fmt.Errorf("metadata header said size should be %d but was only %d", size, len(b)-2)
	}
	data := b[2 : 2+size]
	if compressed {
		data, err = c.decompress(data)
		if err != nil {
			return nil, fmt.Errorf("decompress error: %v", err)
		}
	}
	return &metadatablock{
		compressed: compressed,
		data:       data,
	}, nil
}

func (m *metadatablock) toBytes(c Compressor) ([]byte, error) {
	b := make([]byte, 2)
	var (
		header uint16
		data   = m.data
		err    error
	)
	if !m.compressed {
		header |= 0x8000
	} else {
		data, err = c.compress(m.data)
		if err != nil {
			return nil, fmt.Errorf("compression error: %v", err)
		}
	}
	header |= uint16(len(data))
	binary.LittleEndian.PutUint16(b[:2], header)
	b = append(b, data...)
	return b, nil
}

func readMetaBlock(r io.ReaderAt, c Compressor, location int64) (data []byte, size uint16, err error) {
	// read bytes off the reader to determine how big it is and if compressed
	b := make([]byte, 2)
	_, _ = r.ReadAt(b, location)
	size, compressed, err := getMetadataSize(b)
	if err != nil {
		return nil, 0, fmt.Errorf("error getting size and compression for metadata block at %d: %v", location, err)
	}
	b = make([]byte, size)
	read, err := r.ReadAt(b, location+2)
	if err != nil && err != io.EOF {
		return nil, 0, fmt.Errorf("unable to read metadata block of size %d at location %d: %v", size, location, err)
	}
	if read != len(b) {
		return nil, 0, fmt.Errorf("read %d instead of expected %d bytes for metadata block at location %d", read, size, location)
	}
	data = b
	if compressed {
		if c == nil {
			return nil, 0, fmt.Errorf("metadata block at %d compressed, but no compressor provided", location)
		}
		data, err = c.decompress(b)
		if err != nil {
			return nil, 0, fmt.Errorf("decompress error: %v", err)
		}
	}
	return data, size + 2, nil
}

// readMetadata read as many bytes of metadata as required for the given size, with the byteOffset provided as a starting
// point into the first block. Can read multiple blocks if necessary, e.g. if a block is 8192 bytes (standard), and
// requests to read 500 bytes beginning at offset 8000 into the first block.
// it always returns to the end of the block, even if that is greater than the given size. This makes it easy to use more
// data than expected on first read. The consumer is expected to cut it down, if needed
func readMetadata(r io.ReaderAt, c Compressor, firstBlock int64, initialBlockOffset uint32, byteOffset uint16, size int) ([]byte, error) {
	var (
		b           []byte
		blockOffset = int(initialBlockOffset)
	)
	// we know how many blocks, so read them all in
	m, read, err := readMetaBlock(r, c, firstBlock+int64(blockOffset))
	if err != nil {
		return nil, err
	}
	b = append(b, m[byteOffset:]...)
	// do we have any more to read?
	for len(b) < size {
		blockOffset += int(read)
		m, read, err = readMetaBlock(r, c, firstBlock+int64(blockOffset))
		if err != nil {
			return nil, err
		}
		b = append(b, m...)
	}
	return b, nil
}
