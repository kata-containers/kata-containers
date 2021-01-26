// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/json"
	"io/ioutil"
	"os"
	"reflect"
	"testing"

	"github.com/kata-containers/directvolume"
	"github.com/stretchr/testify/assert"
)

func WriteJsonFile(obj interface{}, file string) error {
	maps := make(map[string]interface{})
	t := reflect.TypeOf(obj)
	v := reflect.ValueOf(obj)
	for i := 0; i < v.NumField(); i++ {
		if v.Field(i).String() != "" {
			maps[t.Field(i).Name] = v.Field(i).String()
		}
	}
	rankingsJSON, _ := json.Marshal(maps)
	if err := ioutil.WriteFile(file, rankingsJSON, 0644); err != nil {
		return err
	}
	return nil
}

func TestNoFile(t *testing.T) {
	_, err := getDirectAssignedDiskMountInfo("")
	assert.Error(t, err)
}

func TestNoJson(t *testing.T) {
	file, err := ioutil.TempFile("", "testnojson")
	assert.NoError(t, err)
	defer os.Remove(file.Name())
	defer file.Close()

	_, err = getDirectAssignedDiskMountInfo(file.Name())
	assert.Error(t, err)
}

func TestNotJson(t *testing.T) {
	file, err := ioutil.TempFile("", "testnot.json")
	assert.NoError(t, err)
	defer os.Remove(file.Name())
	defer file.Close()

	_, err = file.WriteString("foobar")
	assert.NoError(t, err)

	_, err = getDirectAssignedDiskMountInfo(file.Name())
	assert.Error(t, err)
}

func TestUnexpectedJson(t *testing.T) {
	file, err := ioutil.TempFile("", "test-weird.json")
	assert.NoError(t, err)
	defer os.Remove(file.Name())
	defer file.Close()

	m := directvolume.DiskMountInfo{
		Device:     "/dev/loop13",
		VolumeType: "blk-filesystem",
		TargetPath: "/configs",
		FsType:     "ext4",
		Options:    "ro",
	}

	_, err = file.WriteString("{\"Device\":\"/dev/loop13\",\"TargetPath\":\"/configs\",\"VolumeType\":\"blk-filesystem\",\"FsType\":\"ext4\",\"Options\":\"ro\", \"spaghetti\":\"overcooked\"}")
	assert.NoError(t, err)

	resDiskInfo, err := getDirectAssignedDiskMountInfo(file.Name())
	assert.NoError(t, err)

	// expect to read back m:
	assert.Equal(t, m, resDiskInfo)
}

func TestJson(t *testing.T) {
	file, err := ioutil.TempFile("", "test.json")
	assert.NoError(t, err)
	defer os.Remove(file.Name())
	defer file.Close()

	m := directvolume.DiskMountInfo{
		Device:     "/dev/xda",
		VolumeType: "blk-filesystem",
		TargetPath: "/certs",
	}

	err = WriteJsonFile(m, file.Name())
	assert.NoError(t, err)

	resDiskInfo, err := getDirectAssignedDiskMountInfo(file.Name())
	assert.NoError(t, err)

	// expect to read back m:
	assert.Equal(t, m, resDiskInfo)
}
