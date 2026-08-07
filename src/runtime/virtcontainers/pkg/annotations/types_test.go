// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package annotations

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestParseEmptyDirs(t *testing.T) {
	assert := assert.New(t)

	eds := &EmptyDirs{
		EmptyDirs: []*EmptyDir{
			&EmptyDir{
				Name: "vol-1",
			},
		},
	}

	// struct to string, Marshal object
	str, err := eds.String()
	assert.NoError(err)
	assert.Equal(`{"EmptyDirs":[{"name":"vol-1"}]}`, str)

	ed := eds.EmptyDirs[0]

	// test if a memory backended emptydir
	assert.False(ed.IsMemoryBackended())

	ed.Medium = EmptyDirMediumMemory
	assert.True(ed.IsMemoryBackended())

	ed.SizeLimit = "2Gi"

	// struct to string, Marshal object
	str, err = eds.String()
	assert.NoError(err)
	assert.Equal(`{"EmptyDirs":[{"name":"vol-1","medium":"Memory","size_limit":"2Gi"}]}`, str)

	// string to object, Unmarshal
	str = `{"EmptyDirs":[{"name":"vol-2","medium":"Memory","size_limit":"1Gi"}]}`
	eds, err = ParseEmptyDirs(str)
	assert.NoError(err)

	ed = eds.EmptyDirs[0]
	assert.Equal("vol-2", ed.Name)
	assert.Equal("1Gi", ed.SizeLimit)
	assert.Equal(EmptyDirMediumMemory, ed.Medium)
}
