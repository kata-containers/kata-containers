package squashfs

import (
	"bytes"
	"compress/zlib"
	"encoding/binary"
	"fmt"
	"io"

	"github.com/klauspost/compress/zstd"
	lz4 "github.com/pierrec/lz4/v4"
	"github.com/ulikunitz/xz"
	"github.com/ulikunitz/xz/lzma"
)

// Compressor defines a compressor. Fulfilled by various implementations in this package
type Compressor interface {
	compress([]byte) ([]byte, error)
	decompress([]byte) ([]byte, error)
	loadOptions([]byte) error
	optionsBytes() []byte
	flavour() compression
}

// CompressorLzma lzma compression
type CompressorLzma struct {
}

func (c *CompressorLzma) compress(in []byte) ([]byte, error) {
	var b bytes.Buffer
	lz, err := lzma.NewWriter(&b)
	if err != nil {
		return nil, fmt.Errorf("error creating lzma compressor: %v", err)
	}
	if _, err := lz.Write(in); err != nil {
		return nil, err
	}
	if err := lz.Close(); err != nil {
		return nil, err
	}
	return b.Bytes(), nil
}
func (c *CompressorLzma) decompress(in []byte) ([]byte, error) {
	b := bytes.NewReader(in)
	lz, err := lzma.NewReader(b)
	if err != nil {
		return nil, fmt.Errorf("error creating lzma decompressor: %v", err)
	}
	p, err := io.ReadAll(lz)
	if err != nil {
		return nil, fmt.Errorf("error decompressing: %v", err)
	}
	return p, nil
}

//nolint:unused,revive // it is important to implement the interface
func (c *CompressorLzma) loadOptions(b []byte) error {
	// lzma has no supported optiosn
	return nil
}
func (c *CompressorLzma) optionsBytes() []byte {
	return []byte{}
}
func (c *CompressorLzma) flavour() compression {
	return compressionLzma
}

type GzipStrategy uint16

// gzip strategy options
const (
	GzipDefault          GzipStrategy = 0x1
	GzipFiltered         GzipStrategy = 0x2
	GzipHuffman          GzipStrategy = 0x4
	GzipRunLengthEncoded GzipStrategy = 0x8
	GzipFixed            GzipStrategy = 0x10
)

// CompressorGzip gzip compression
type CompressorGzip struct {
	CompressionLevel uint32
	WindowSize       uint16
	Strategies       map[GzipStrategy]bool
}

func (c *CompressorGzip) compress(in []byte) ([]byte, error) {
	var b bytes.Buffer
	gz, err := zlib.NewWriterLevel(&b, int(c.CompressionLevel))
	if err != nil {
		return nil, fmt.Errorf("error creating gzip compressor: %v", err)
	}
	if _, err := gz.Write(in); err != nil {
		return nil, err
	}
	if err := gz.Flush(); err != nil {
		return nil, err
	}
	if err := gz.Close(); err != nil {
		return nil, err
	}
	return b.Bytes(), nil
}
func (c *CompressorGzip) decompress(in []byte) ([]byte, error) {
	b := bytes.NewReader(in)
	gz, err := zlib.NewReader(b)
	if err != nil {
		return nil, fmt.Errorf("error creating gzip decompressor: %v", err)
	}
	p, err := io.ReadAll(gz)
	if err != nil {
		return nil, fmt.Errorf("error decompressing: %v", err)
	}
	if err := gz.Close(); err != nil {
		return nil, err
	}
	return p, nil
}

func (c *CompressorGzip) loadOptions(b []byte) error {
	expected := 8
	if len(b) != expected {
		return fmt.Errorf("cannot parse gzip options, received %d bytes expected %d", len(b), expected)
	}
	c.CompressionLevel = binary.LittleEndian.Uint32(b[0:4])
	c.WindowSize = binary.LittleEndian.Uint16(b[4:6])
	strategies := map[GzipStrategy]bool{}
	flags := binary.LittleEndian.Uint16(b[6:8])
	for _, strategy := range []GzipStrategy{GzipDefault, GzipFiltered, GzipHuffman, GzipRunLengthEncoded, GzipFixed} {
		if flags&uint16(strategy) == uint16(strategy) {
			strategies[strategy] = true
		}
	}
	c.Strategies = strategies
	return nil
}
func (c *CompressorGzip) optionsBytes() []byte {
	b := make([]byte, 8)
	binary.LittleEndian.PutUint32(b[0:4], c.CompressionLevel)
	binary.LittleEndian.PutUint16(b[4:6], c.WindowSize)
	var flags uint16
	for _, strategy := range []GzipStrategy{GzipDefault, GzipFiltered, GzipHuffman, GzipRunLengthEncoded, GzipFixed} {
		if c.Strategies[strategy] {
			flags |= uint16(strategy)
		}
	}
	binary.LittleEndian.PutUint16(b[6:8], flags)
	return b
}
func (c *CompressorGzip) flavour() compression {
	return compressionGzip
}

// XzFilter filter for xz compression
type XzFilter uint32

// xz filter options
const (
	XzFilterX86      XzFilter = 0x1
	XzFilterPowerPC  XzFilter = 0x2
	XzFilterIA64     XzFilter = 0x4
	XzFilterArm      XzFilter = 0x8
	XzFilterArmThumb XzFilter = 0x10
	XzFilterSparc    XzFilter = 0x20
)

// CompressorXz xz compression
type CompressorXz struct {
	DictionarySize    uint32
	ExecutableFilters map[XzFilter]bool
}

func (c *CompressorXz) compress(in []byte) ([]byte, error) {
	var b bytes.Buffer
	config := xz.WriterConfig{
		DictCap: int(c.DictionarySize),
	}
	xzWriter, err := config.NewWriter(&b)
	if err != nil {
		return nil, fmt.Errorf("error creating xz compressor: %v", err)
	}
	if _, err := xzWriter.Write(in); err != nil {
		return nil, err
	}
	if err := xzWriter.Close(); err != nil {
		return nil, err
	}
	return b.Bytes(), nil
}
func (c *CompressorXz) decompress(in []byte) ([]byte, error) {
	b := bytes.NewReader(in)
	xzReader, err := xz.NewReader(b)
	if err != nil {
		return nil, fmt.Errorf("error creating xz decompressor: %v", err)
	}
	p, err := io.ReadAll(xzReader)
	if err != nil {
		return nil, fmt.Errorf("error decompressing: %v", err)
	}
	return p, nil
}
func (c *CompressorXz) loadOptions(b []byte) error {
	expected := 8
	if len(b) != expected {
		return fmt.Errorf("cannot parse xz options, received %d bytes expected %d", len(b), expected)
	}
	c.DictionarySize = binary.LittleEndian.Uint32(b[0:4])
	filters := map[XzFilter]bool{}
	flags := binary.LittleEndian.Uint32(b[4:8])
	for _, filter := range []XzFilter{XzFilterX86, XzFilterPowerPC, XzFilterIA64, XzFilterArm, XzFilterArmThumb, XzFilterSparc} {
		if flags&uint32(filter) == uint32(filter) {
			filters[filter] = true
		}
	}
	c.ExecutableFilters = filters
	return nil
}
func (c *CompressorXz) optionsBytes() []byte {
	b := make([]byte, 8)
	binary.LittleEndian.PutUint32(b[0:4], c.DictionarySize)
	var flags uint32
	for _, filter := range []XzFilter{XzFilterX86, XzFilterPowerPC, XzFilterIA64, XzFilterArm, XzFilterArmThumb, XzFilterSparc} {
		if c.ExecutableFilters[filter] {
			flags |= uint32(filter)
		}
	}
	binary.LittleEndian.PutUint32(b[4:8], flags)
	return b
}
func (c *CompressorXz) flavour() compression {
	return compressionXz
}

// lz4 compression
type lz4Flag uint32

const (
	lz4HighCompression lz4Flag = 0x1
)
const (
	lz4version1 uint32 = 1
)

// CompressorLz4 lz4 compression
type CompressorLz4 struct {
	version uint32
	flags   map[lz4Flag]bool
}

func (c *CompressorLz4) compress(in []byte) ([]byte, error) {
	var b bytes.Buffer
	lz := lz4.NewWriter(&b)
	if _, err := lz.Write(in); err != nil {
		return nil, err
	}
	if err := lz.Close(); err != nil {
		return nil, err
	}
	return b.Bytes(), nil
}
func (c *CompressorLz4) decompress(in []byte) ([]byte, error) {
	b := bytes.NewReader(in)
	lz := lz4.NewReader(b)
	p, err := io.ReadAll(lz)
	if err != nil {
		return nil, fmt.Errorf("error decompressing: %v", err)
	}
	return p, nil
}
func (c *CompressorLz4) loadOptions(b []byte) error {
	expected := 8
	if len(b) != expected {
		return fmt.Errorf("cannot parse lz4 options, received %d bytes expected %d", len(b), expected)
	}
	version := binary.LittleEndian.Uint32(b[0:4])
	if version != lz4version1 {
		return fmt.Errorf("compressed with lz4 version %d, only support %d", version, lz4version1)
	}
	c.version = version
	flagMap := map[lz4Flag]bool{}
	flags := binary.LittleEndian.Uint32(b[4:8])
	for _, f := range []lz4Flag{lz4HighCompression} {
		if flags&uint32(f) == uint32(f) {
			flagMap[f] = true
		}
	}
	c.flags = flagMap
	return nil
}
func (c *CompressorLz4) optionsBytes() []byte {
	b := make([]byte, 8)
	binary.LittleEndian.PutUint32(b[0:4], c.version)
	var flags uint32
	for _, f := range []lz4Flag{lz4HighCompression} {
		if c.flags[f] {
			flags |= uint32(f)
		}
	}
	binary.LittleEndian.PutUint32(b[4:8], flags)
	return b
}
func (c *CompressorLz4) flavour() compression {
	return compressionLz4
}

// CompressorZstd zstd compression
type CompressorZstd struct {
	level uint32
}

const (
	zstdMinLevel uint32 = 1
	zstdMaxLevel uint32 = 22
)

func (c *CompressorZstd) loadOptions(b []byte) error {
	expected := 4
	if len(b) != expected {
		return fmt.Errorf("cannot parse zstd options, received %d bytes expected %d", len(b), expected)
	}
	level := binary.LittleEndian.Uint32(b[0:4])
	if level < zstdMinLevel || level > zstdMaxLevel {
		return fmt.Errorf("zstd compression level requested %d, must be at least %d and not more thann %d", level, zstdMinLevel, zstdMaxLevel)
	}
	c.level = level
	return nil
}
func (c *CompressorZstd) optionsBytes() []byte {
	b := make([]byte, 4)
	binary.LittleEndian.PutUint32(b[0:4], c.level)
	return b
}
func (c *CompressorZstd) flavour() compression {
	return compressionZstd
}
func (c *CompressorZstd) compress(in []byte) ([]byte, error) {
	var b bytes.Buffer
	z, err := zstd.NewWriter(&b)
	if err != nil {
		return nil, fmt.Errorf("failed to create zstd compressor: %w", err)
	}
	if _, err := z.Write(in); err != nil {
		return nil, err
	}
	if err := z.Close(); err != nil {
		return nil, err
	}
	return b.Bytes(), nil
}
func (c *CompressorZstd) decompress(in []byte) ([]byte, error) {
	z, err := zstd.NewReader(nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create zstd decompressor: %w", err)
	}
	defer z.Close()
	p, err := z.DecodeAll(in, nil)
	if err != nil {
		return nil, fmt.Errorf("error decompressing zstd: %w", err)
	}
	return p, nil
}

func newCompressor(flavour compression) (Compressor, error) {
	var c Compressor
	switch flavour {
	case compressionNone:
		c = nil
	case compressionGzip:
		c = &CompressorGzip{}
	case compressionLzma:
		c = &CompressorLzma{}
	case compressionLzo:
		return nil, fmt.Errorf("LZO compression not yet supported")
	case compressionXz:
		c = &CompressorXz{}
	case compressionLz4:
		c = &CompressorLz4{}
	case compressionZstd:
		c = &CompressorZstd{}
	default:
		return nil, fmt.Errorf("unknown compression type: %d", flavour)
	}
	return c, nil
}
