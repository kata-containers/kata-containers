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

type swapbo []byte

var _ Bytes = (*swapbo)(nil)

func (b *swapbo) Read8(pos int) uint8 {
	return (*b)[pos]
}

func (b *swapbo) Read16(pos int) uint16 {
	buf := [2]byte{}
	buf[0] = (*b)[pos+1]
	buf[1] = (*b)[pos+0]
	return *(*uint16)(unsafe.Pointer(&buf[0]))
}

func (b *swapbo) Read32(pos int) uint32 {
	buf := [4]byte{}
	buf[0] = (*b)[pos+3]
	buf[1] = (*b)[pos+2]
	buf[2] = (*b)[pos+1]
	buf[3] = (*b)[pos+0]
	return *(*uint32)(unsafe.Pointer(&buf[0]))
}

func (b *swapbo) Read64(pos int) uint64 {
	buf := [8]byte{}
	buf[0] = (*b)[pos+7]
	buf[1] = (*b)[pos+6]
	buf[2] = (*b)[pos+5]
	buf[3] = (*b)[pos+4]
	buf[4] = (*b)[pos+3]
	buf[5] = (*b)[pos+2]
	buf[6] = (*b)[pos+1]
	buf[7] = (*b)[pos+0]
	return *(*uint64)(unsafe.Pointer(&buf[0]))
}

func (b *swapbo) Write8(pos int, value uint8) {
	(*b)[pos] = value
}

func (b *swapbo) Write16(pos int, value uint16) {
	buf := [2]byte{}
	*(*uint16)(unsafe.Pointer(&buf[0])) = value
	(*b)[pos+0] = buf[1]
	(*b)[pos+1] = buf[0]
}

func (b *swapbo) Write32(pos int, value uint32) {
	buf := [4]byte{}
	*(*uint32)(unsafe.Pointer(&buf[0])) = value
	(*b)[pos+0] = buf[3]
	(*b)[pos+1] = buf[2]
	(*b)[pos+2] = buf[1]
	(*b)[pos+3] = buf[0]
}

func (b *swapbo) Write64(pos int, value uint64) {
	buf := [8]byte{}
	*(*uint64)(unsafe.Pointer(&buf[0])) = value
	(*b)[pos+0] = buf[7]
	(*b)[pos+1] = buf[6]
	(*b)[pos+2] = buf[5]
	(*b)[pos+3] = buf[4]
	(*b)[pos+4] = buf[3]
	(*b)[pos+5] = buf[2]
	(*b)[pos+6] = buf[1]
	(*b)[pos+7] = buf[0]
}

func (b *swapbo) Slice(offset int, size int) Bytes {
	nb := (*b)[offset : offset+size]
	return &nb
}

func (b *swapbo) LittleEndian() Bytes {
	return NewLittleEndian((*[]byte)(b))
}

func (b *swapbo) BigEndian() Bytes {
	return NewBigEndian((*[]byte)(b))
}

func (b *swapbo) Raw() *[]byte {
	return (*[]byte)(b)
}

func (b *swapbo) Len() int {
	return len(*b)
}
