// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package config

import (
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func createPFNFile(assert *assert.Assertions, dir string) string {
	pfnPath := filepath.Join(dir, "pfn")
	file, err := os.Create(pfnPath)
	assert.NoError(err)
	defer file.Close()

	l, err := file.WriteAt([]byte(pfnSignature), pfnSignatureOffset)
	assert.NoError(err)
	assert.Equal(len(pfnSignature), l)

	return pfnPath
}

func TestHasPFNSignature(t *testing.T) {
	assert := assert.New(t)

	b := hasPFNSignature("/abc/xyz/123/sw")
	assert.False(b)

	f, err := ioutil.TempFile("", "pfn")
	assert.NoError(err)
	f.Close()
	defer os.Remove(f.Name())

	b = hasPFNSignature(f.Name())
	assert.False(b)

	pfnFile := createPFNFile(assert, os.TempDir())
	defer os.Remove(pfnFile)

	b = hasPFNSignature(pfnFile)
	assert.True(b)
}
