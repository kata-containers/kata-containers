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
	"unsafe"
)

type native []byte

var _ Bytes = (*native)(nil)

func (b *native) Read8(pos int) uint8 {
	return (*b)[pos]
}

func (b *native) Read16(pos int) uint16 {
	return *(*uint16)(unsafe.Pointer(&((*b)[pos])))
}

func (b *native) Read32(pos int) uint32 {
	return *(*uint32)(unsafe.Pointer(&((*b)[pos])))
}

func (b *native) Read64(pos int) uint64 {
	return *(*uint64)(unsafe.Pointer(&((*b)[pos])))
}

func (b *native) Write8(pos int, value uint8) {
	(*b)[pos] = value
}

func (b *native) Write16(pos int, value uint16) {
	*(*uint16)(unsafe.Pointer(&((*b)[pos]))) = value
}

func (b *native) Write32(pos int, value uint32) {
	*(*uint32)(unsafe.Pointer(&((*b)[pos]))) = value
}

func (b *native) Write64(pos int, value uint64) {
	*(*uint64)(unsafe.Pointer(&((*b)[pos]))) = value
}

func (b *native) Slice(offset int, size int) Bytes {
	nb := (*b)[offset : offset+size]
	return &nb
}

func (b *native) LittleEndian() Bytes {
	return NewLittleEndian((*[]byte)(b))
}

func (b *native) BigEndian() Bytes {
	return NewBigEndian((*[]byte)(b))
}

func (b *native) Raw() *[]byte {
	return (*[]byte)(b)
}

func (b *native) Len() int {
	return len(*b)
}
