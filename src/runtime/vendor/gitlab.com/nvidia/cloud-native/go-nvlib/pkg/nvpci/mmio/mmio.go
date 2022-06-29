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

package mmio

import (
	"fmt"
	"os"
	"syscall"
	"unsafe"

	"gitlab.com/nvidia/cloud-native/go-nvlib/pkg/nvpci/bytes"
)

// Mmio memory map a region
type Mmio interface {
	bytes.Raw
	bytes.Reader
	bytes.Writer
	Sync() error
	Close() error
	Slice(offset int, size int) Mmio
	LittleEndian() Mmio
	BigEndian() Mmio
}

type mmio struct {
	bytes.Bytes
}

func open(path string, offset int, size int, flags int) (Mmio, error) {
	var mmapFlags int
	switch flags {
	case os.O_RDONLY:
		mmapFlags = syscall.PROT_READ
	case os.O_RDWR:
		mmapFlags = syscall.PROT_READ | syscall.PROT_WRITE
	default:
		return nil, fmt.Errorf("invalid flags: %v", flags)
	}

	file, err := os.OpenFile(path, flags, 0)
	if err != nil {
		return nil, fmt.Errorf("failed to open file: %v", err)
	}
	defer file.Close()

	fi, err := file.Stat()
	if err != nil {
		return nil, fmt.Errorf("failed to get file info: %v", err)
	}

	if size > int(fi.Size()) {
		return nil, fmt.Errorf("requested size larger than file size")
	}

	if size < 0 {
		size = int(fi.Size())
	}

	mmap, err := syscall.Mmap(
		int(file.Fd()),
		int64(offset),
		size,
		mmapFlags,
		syscall.MAP_SHARED)
	if err != nil {
		return nil, fmt.Errorf("failed to mmap file: %v", err)
	}

	return &mmio{bytes.New(&mmap)}, nil
}

// OpenRO open region readonly
func OpenRO(path string, offset int, size int) (Mmio, error) {
	return open(path, offset, size, os.O_RDONLY)
}

// OpenRW open region read write
func OpenRW(path string, offset int, size int) (Mmio, error) {
	return open(path, offset, size, os.O_RDWR)
}

func (m *mmio) Slice(offset int, size int) Mmio {
	return &mmio{m.Bytes.Slice(offset, size)}
}

func (m *mmio) LittleEndian() Mmio {
	return &mmio{m.Bytes.LittleEndian()}
}

func (m *mmio) BigEndian() Mmio {
	return &mmio{m.Bytes.BigEndian()}
}

func (m *mmio) Close() error {
	err := syscall.Munmap(*m.Bytes.Raw())
	if err != nil {
		return fmt.Errorf("failed to munmap file: %v", err)
	}
	return nil
}

func (m *mmio) Sync() error {
	_, _, errno := syscall.Syscall(
		syscall.SYS_MSYNC,
		uintptr(unsafe.Pointer(&(*m.Bytes.Raw())[0])),
		uintptr(m.Len()),
		uintptr(syscall.MS_SYNC|syscall.MS_INVALIDATE))
	if errno != 0 {
		return fmt.Errorf("failed to msync file: %v", errno)
	}
	return nil
}
