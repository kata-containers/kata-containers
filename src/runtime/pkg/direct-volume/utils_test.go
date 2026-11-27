// Copyright (c) 2022 Databricks Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package volume

import (
	b64 "encoding/base64"
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
	actual := MountInfo{
		VolumeType: "block",
		Device:     "/dev/sda",
		FsType:     "ext4",
		Metadata: map[string]string{
			FSGroupMetadataKey:             "3000",
			FSGroupChangePolicyMetadataKey: string(FSGroupChangeOnRootMismatch),
		},
		Options: []string{"journal_dev", "noload"},
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
	assert.Equal(t, expected.Metadata, actual.Metadata)

	_, err = os.Stat(filepath.Join(kataDirectVolumeRootPath, b64.URLEncoding.EncodeToString([]byte(volumePath))))
	assert.Nil(t, err)
	// Remove the file
	err = Remove(volumePath)
	assert.Nil(t, err)
	_, err = os.Stat(filepath.Join(kataDirectVolumeRootPath, b64.URLEncoding.EncodeToString([]byte(volumePath))))
	assert.True(t, errors.Is(err, os.ErrNotExist))
	_, err = os.Stat(filepath.Join(kataDirectVolumeRootPath))
	assert.Nil(t, err)
}

func TestRecordSandboxID(t *testing.T) {
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

	sandboxID := uuid.Generate().String()
	err = RecordSandboxID(sandboxID, volumePath)
	assert.Nil(t, err)

	id, err := GetSandboxIDForVolume(volumePath)
	assert.Nil(t, err)
	assert.Equal(t, sandboxID, id)
}

func TestRecordSandboxIDNoMountInfoFile(t *testing.T) {
	var err error
	kataDirectVolumeRootPath = t.TempDir()

	var volumePath = "/a/b/c"
	sandboxID := uuid.Generate().String()
	err = RecordSandboxID(sandboxID, volumePath)
	assert.Error(t, err)
	assert.True(t, errors.Is(err, os.ErrNotExist))
}

func TestCheckRawImageFormat(t *testing.T) {
	const runtimeRsOnly = "which is only supported by runtime-rs"
	qcow2Key := func(key string) string {
		return "direct volume /csi/disk.qcow2 sets qcow2 metadata " + key + ", " + runtimeRsOnly
	}
	testCases := []struct {
		name     string
		metadata map[string]string
		// Empty for success, otherwise text the error must contain.
		wantErr string
	}{
		{"no metadata", nil, ""},
		{"empty metadata", map[string]string{}, ""},
		{"explicit raw", map[string]string{ImageFormatMetadataKey: "raw"}, ""},
		{"unrelated short keys", map[string]string{"format": "qcow2", "backing-chain": "[]", "discard": "unmap"}, ""},
		{"qcow2", map[string]string{ImageFormatMetadataKey: "qcow2"}, `has image format "qcow2", ` + runtimeRsOnly},
		{"empty format", map[string]string{ImageFormatMetadataKey: ""}, `has image format "", ` + runtimeRsOnly},
		{"upper-case raw", map[string]string{ImageFormatMetadataKey: "RAW"}, `has image format "RAW", ` + runtimeRsOnly},
		{"backing chain only", map[string]string{BackingChainMetadataKey: "[]"}, qcow2Key(BackingChainMetadataKey)},
		{"raw with backing chain", map[string]string{ImageFormatMetadataKey: "raw", BackingChainMetadataKey: "[]"}, qcow2Key(BackingChainMetadataKey)},
		{"discard only", map[string]string{DiscardMetadataKey: "unmap"}, qcow2Key(DiscardMetadataKey)},
		{"raw with empty discard", map[string]string{ImageFormatMetadataKey: "raw", DiscardMetadataKey: ""}, qcow2Key(DiscardMetadataKey)},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			assert := assert.New(t)

			err := CheckRawImageFormat(&MountInfo{Device: "/csi/disk.qcow2", Metadata: tc.metadata})
			if tc.wantErr == "" {
				assert.NoError(err)
				return
			}
			assert.ErrorContains(err, tc.wantErr)
		})
	}
}
