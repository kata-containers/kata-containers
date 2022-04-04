// Copyright (c) 2022 Databricks Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package volume

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/uuid"
	"github.com/stretchr/testify/assert"
)

func TestAdd(t *testing.T) {
	var err error
	kataDirectVolumeRootPath = t.TempDir()
	var volumePath = "/a/b/c"
	var basePath = "a"
	actual := MountInfo{
		VolumeType: "block",
		Device:     "/dev/sda",
		FsType:     "ext4",
		Options:    []string{"journal_dev", "noload"},
	}
	buf, err := json.Marshal(actual)
	assert.Nil(t, err)

	// Add the mount info
	assert.Nil(t, Add(volumePath, string(buf)))

	// Validate the mount info
	expected, err := VolumeMountInfo(volumePath)
	assert.Nil(t, err)
	assert.Equal(t, expected.Device, actual.Device)
	assert.Equal(t, expected.FsType, actual.FsType)
	assert.Equal(t, expected.Options, actual.Options)

	// Remove the file
	err = Remove(volumePath)
	assert.Nil(t, err)
	_, err = os.Stat(filepath.Join(kataDirectVolumeRootPath, basePath))
	assert.True(t, errors.Is(err, os.ErrNotExist))

	// Test invalid mount info json
	assert.Error(t, Add(volumePath, "{invalid json}"))
}

func TestRecordSandboxId(t *testing.T) {
	var err error
	kataDirectVolumeRootPath = t.TempDir()

	var volumePath = "/a/b/c"
	mntInfo := MountInfo{
		VolumeType: "block",
		Device:     "/dev/sda",
		FsType:     "ext4",
		Options:    []string{"journal_dev", "noload"},
	}
	buf, err := json.Marshal(mntInfo)
	assert.Nil(t, err)

	// Add the mount info
	assert.Nil(t, Add(volumePath, string(buf)))

	sandboxId := uuid.Generate().String()
	err = RecordSandboxId(sandboxId, volumePath)
	assert.Nil(t, err)

	id, err := GetSandboxIdForVolume(volumePath)
	assert.Nil(t, err)
	assert.Equal(t, sandboxId, id)
}

func TestRecordSandboxIdNoMountInfoFile(t *testing.T) {
	var err error
	kataDirectVolumeRootPath = t.TempDir()

	var volumePath = "/a/b/c"
	sandboxId := uuid.Generate().String()
	err = RecordSandboxId(sandboxId, volumePath)
	assert.Error(t, err)
	assert.True(t, errors.Is(err, os.ErrNotExist))
}
