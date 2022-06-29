/*
 * Copyright (c) 2021, NVIDIA CORPORATION.  All rights reserved.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package bytes

import (
	"encoding/binary"
	"unsafe"
)

// Raw returns just the bytes without any assumptions about layout
type Raw interface {
	Raw() *[]byte
}

// Reader used to read various data sizes in the byte array
type Reader interface {
	Read8(pos int) uint8
	Read16(pos int) uint16
	Read32(pos int) uint32
	Read64(pos int) uint64
	Len() int
}

// Writer used to write various sizes of data in the byte array
type Writer interface {
	Write8(pos int, value uint8)
	Write16(pos int, value uint16)
	Write32(pos int, value uint32)
	Write64(pos int, value uint64)
	Len() int
}

// Bytes object for manipulating arbitrary byte arrays
type Bytes interface {
	Raw
	Reader
	Writer
	Slice(offset int, size int) Bytes
	LittleEndian() Bytes
	BigEndian() Bytes
}

var nativeByteOrder binary.ByteOrder

func init() {
	buf := [2]byte{}
	*(*uint16)(unsafe.Pointer(&buf[0])) = uint16(0x00FF)

	switch buf {
	case [2]byte{0xFF, 0x00}:
		nativeByteOrder = binary.LittleEndian
	case [2]byte{0x00, 0xFF}:
		nativeByteOrder = binary.BigEndian
	default:
		panic("Unable to infer byte order")
	}
}

// New raw bytearray
func New(data *[]byte) Bytes {
	return (*native)(data)
}

// NewLittleEndian little endian ordering of bytes
func NewLittleEndian(data *[]byte) Bytes {
	if nativeByteOrder == binary.LittleEndian {
		return (*native)(data)
	}

	return (*swapbo)(data)
}

// NewBigEndian big endian ordering of bytes
func NewBigEndian(data *[]byte) Bytes {
	if nativeByteOrder == binary.BigEndian {
		return (*native)(data)
	}

	return (*swapbo)(data)
}
