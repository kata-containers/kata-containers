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

	"gitlab.com/nvidia/cloud-native/go-nvlib/pkg/nvpci/bytes"
)

type mockMmio struct {
	mmio
	source *[]byte
	offset int
	rw     bool
}

func mockOpen(source *[]byte, offset int, size int, rw bool) (Mmio, error) {
	if size < 0 {
		size = len(*source) - offset
	}
	if (offset + size) > len(*source) {
		return nil, fmt.Errorf("offset+size out of range")
	}

	data := append([]byte{}, (*source)[offset:offset+size]...)

	m := &mockMmio{}
	m.Bytes = bytes.New(&data).LittleEndian()
	m.source = source
	m.offset = offset
	m.rw = rw

	return m, nil
}

// MockOpenRO open read only
func MockOpenRO(source *[]byte, offset int, size int) (Mmio, error) {
	return mockOpen(source, offset, size, false)
}

// MockOpenRW open read write
func MockOpenRW(source *[]byte, offset int, size int) (Mmio, error) {
	return mockOpen(source, offset, size, true)
}

func (m *mockMmio) Close() error {
	m = &mockMmio{}
	return nil
}

func (m *mockMmio) Sync() error {
	if !m.rw {
		return fmt.Errorf("opened read-only")
	}
	for i := range *m.Bytes.Raw() {
		(*m.source)[m.offset+i] = (*m.Bytes.Raw())[i]
	}
	return nil
}
