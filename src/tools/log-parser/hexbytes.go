//
// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"io"
	"os"
	"strings"
)

// HexByteReader is an I/O Reader type.
type HexByteReader struct {
	file string
	f    *os.File
	data []byte

	// total length of "data"
	len int

	// how much of "data" has been sent back to the caller
	offset int
}

// NewHexByteReader returns a new hex byte reader that escapes all
// hex-encoded characters.
func NewHexByteReader(file string) *HexByteReader {
	var f *os.File

	// treat dash as an alias for standard input
	if file == stdinFile {
		f = os.Stdin
	}

	return &HexByteReader{
		file: file,
		f:    f,
	}
}

// Read is a Reader that converts "\x" to "\\x"
func (r *HexByteReader) Read(p []byte) (n int, err error) {
	size := len(p)

	if r.data == nil {
		if r.f == nil {
			r.f, err = os.Open(r.file)
			if err != nil {
				return 0, err
			}
		}

		// read the entire file
		bytes, err := io.ReadAll(r.f)
		if err != nil {
			return 0, err
		}

		// although logfmt is happy to parse an empty file, this is
		// surprising to users, so make it an error.
		if len(bytes) == 0 {
			return 0, errors.New("file is empty")
		}

		// perform the conversion
		s := string(bytes)
		result := strings.Replace(s, `\x`, `\\x`, -1)

		// store the data
		r.data = []byte(result)
		r.len = len(r.data)
		r.offset = 0
	}

	// calculate how much data is left to copy
	remaining := r.len - r.offset

	if remaining == 0 {
		return 0, io.EOF
	}

	// see how much data can be copied on this call
	limit := size

	if remaining < limit {
		limit = remaining
	}

	for i := 0; i < limit; i++ {
		// index into the stored data
		src := r.offset

		// copy
		p[i] = r.data[src]

		// update
		r.offset++
	}

	return limit, nil
}
