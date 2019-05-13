// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"io/ioutil"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestFileCopySuccessful(t *testing.T) {
	fileContent := "testContent"

	srcFile, err := ioutil.TempFile("", "test_src_copy")
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(srcFile.Name())
	defer srcFile.Close()

	dstFile, err := ioutil.TempFile("", "test_dst_copy")
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(dstFile.Name())

	dstPath := dstFile.Name()

	if err := dstFile.Close(); err != nil {
		t.Fatal(err)
	}

	if _, err := srcFile.WriteString(fileContent); err != nil {
		t.Fatal(err)
	}

	if err := FileCopy(srcFile.Name(), dstPath); err != nil {
		t.Fatal(err)
	}

	dstContent, err := ioutil.ReadFile(dstPath)
	if err != nil {
		t.Fatal(err)
	}

	if string(dstContent) != fileContent {
		t.Fatalf("Got %q\nExpecting %q", string(dstContent), fileContent)
	}

	srcInfo, err := srcFile.Stat()
	if err != nil {
		t.Fatal(err)
	}

	dstInfo, err := os.Stat(dstPath)
	if err != nil {
		t.Fatal(err)
	}

	if dstInfo.Mode() != srcInfo.Mode() {
		t.Fatalf("Got FileMode %d\nExpecting FileMode %d", dstInfo.Mode(), srcInfo.Mode())
	}

	if dstInfo.IsDir() != srcInfo.IsDir() {
		t.Fatalf("Got IsDir() = %t\nExpecting IsDir() = %t", dstInfo.IsDir(), srcInfo.IsDir())
	}

	if dstInfo.Size() != srcInfo.Size() {
		t.Fatalf("Got Size() = %d\nExpecting Size() = %d", dstInfo.Size(), srcInfo.Size())
	}
}

func TestFileCopySourceEmptyFailure(t *testing.T) {
	if err := FileCopy("", "testDst"); err == nil {
		t.Fatal("This test should fail because source path is empty")
	}
}

func TestFileCopyDestinationEmptyFailure(t *testing.T) {
	if err := FileCopy("testSrc", ""); err == nil {
		t.Fatal("This test should fail because destination path is empty")
	}
}

func TestFileCopySourceNotExistFailure(t *testing.T) {
	srcFile, err := ioutil.TempFile("", "test_src_copy")
	if err != nil {
		t.Fatal(err)
	}

	srcPath := srcFile.Name()

	if err := srcFile.Close(); err != nil {
		t.Fatal(err)
	}

	if err := os.Remove(srcPath); err != nil {
		t.Fatal(err)
	}

	if err := FileCopy(srcPath, "testDest"); err == nil {
		t.Fatal("This test should fail because source file does not exist")
	}
}

func TestGenerateRandomBytes(t *testing.T) {
	bytesNeeded := 8
	randBytes, err := GenerateRandomBytes(bytesNeeded)
	if err != nil {
		t.Fatal(err)
	}

	if len(randBytes) != bytesNeeded {
		t.Fatalf("Failed to generate %d random bytes", bytesNeeded)
	}
}

func TestRevereString(t *testing.T) {
	str := "Teststr"
	reversed := ReverseString(str)

	if reversed != "rtstseT" {
		t.Fatal("Incorrect String Reversal")
	}
}

func TestWriteToFile(t *testing.T) {
	err := WriteToFile("/file-does-not-exist", []byte("test-data"))
	assert.NotNil(t, err)

	tmpFile, err := ioutil.TempFile("", "test_append_file")
	assert.Nil(t, err)

	filename := tmpFile.Name()
	defer os.Remove(filename)

	tmpFile.Close()

	testData := []byte("test-data")
	err = WriteToFile(filename, testData)
	assert.Nil(t, err)

	data, err := ioutil.ReadFile(filename)
	assert.Nil(t, err)

	assert.True(t, reflect.DeepEqual(testData, data))
}

func TestConstraintsToVCPUs(t *testing.T) {
	assert := assert.New(t)

	vcpus := ConstraintsToVCPUs(0, 100)
	assert.Zero(vcpus)

	vcpus = ConstraintsToVCPUs(100, 0)
	assert.Zero(vcpus)

	expectedVCPUs := uint(4)
	vcpus = ConstraintsToVCPUs(4000, 1000)
	assert.Equal(expectedVCPUs, vcpus)

	vcpus = ConstraintsToVCPUs(4000, 1200)
	assert.Equal(expectedVCPUs, vcpus)
}

func TestGetVirtDriveNameInvalidIndex(t *testing.T) {
	_, err := GetVirtDriveName(-1)

	if err == nil {
		t.Fatal(err)
	}
}

func TestGetVirtDriveName(t *testing.T) {
	tests := []struct {
		index         int
		expectedDrive string
	}{
		{0, "vda"},
		{25, "vdz"},
		{27, "vdab"},
		{704, "vdaac"},
		{18277, "vdzzz"},
	}

	for _, test := range tests {
		driveName, err := GetVirtDriveName(test.index)
		if err != nil {
			t.Fatal(err)
		}
		if driveName != test.expectedDrive {
			t.Fatalf("Incorrect drive Name: Got: %s, Expecting :%s", driveName, test.expectedDrive)

		}
	}
}

func TestGetSCSIIdLun(t *testing.T) {
	tests := []struct {
		index          int
		expectedScsiID int
		expectedLun    int
	}{
		{0, 0, 0},
		{1, 0, 1},
		{2, 0, 2},
		{255, 0, 255},
		{256, 1, 0},
		{257, 1, 1},
		{258, 1, 2},
		{512, 2, 0},
		{513, 2, 1},
	}

	for _, test := range tests {
		scsiID, lun, err := GetSCSIIdLun(test.index)
		assert.Nil(t, err)

		if scsiID != test.expectedScsiID && lun != test.expectedLun {
			t.Fatalf("Expecting scsi-id:lun %d:%d,  Got %d:%d", test.expectedScsiID, test.expectedLun, scsiID, lun)
		}
	}

	_, _, err := GetSCSIIdLun(maxSCSIDevices + 1)
	assert.NotNil(t, err)
}

func TestGetSCSIAddress(t *testing.T) {
	tests := []struct {
		index               int
		expectedSCSIAddress string
	}{
		{0, "0:0"},
		{200, "0:200"},
		{255, "0:255"},
		{258, "1:2"},
		{512, "2:0"},
	}

	for _, test := range tests {
		scsiAddr, err := GetSCSIAddress(test.index)
		assert.Nil(t, err)
		assert.Equal(t, scsiAddr, test.expectedSCSIAddress)

	}
}

func TestBuildSocketPath(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		elems    []string
		valid    bool
		expected string
	}

	longPath := strings.Repeat("/a", 106/2)
	longestPath := longPath + "a"
	pathTooLong := filepath.Join(longestPath, "x")

	data := []testData{
		{[]string{""}, false, ""},

		{[]string{"a"}, true, "a"},
		{[]string{"/a"}, true, "/a"},
		{[]string{"a", "b", "c"}, true, "a/b/c"},
		{[]string{"a", "/b", "c"}, true, "a/b/c"},
		{[]string{"/a", "b", "c"}, true, "/a/b/c"},
		{[]string{"/a", "/b", "/c"}, true, "/a/b/c"},

		{[]string{longPath}, true, longPath},
		{[]string{longestPath}, true, longestPath},
		{[]string{pathTooLong}, false, ""},
	}

	for i, d := range data {
		result, err := BuildSocketPath(d.elems...)

		if d.valid {
			assert.NoErrorf(err, "test %d, data %+v", i, d)
		} else {
			assert.Errorf(err, "test %d, data %+v", i, d)
		}

		assert.NotNil(result)
		assert.Equal(d.expected, result)
	}
}

func TestSupportsVsocks(t *testing.T) {
	assert := assert.New(t)

	orgVHostVSockDevicePath := VHostVSockDevicePath
	defer func() {
		VHostVSockDevicePath = orgVHostVSockDevicePath
	}()

	VHostVSockDevicePath = "/abc/xyz/123"
	assert.False(SupportsVsocks())

	vHostVSockFile, err := ioutil.TempFile("", "vhost-vsock")
	assert.NoError(err)
	defer os.Remove(vHostVSockFile.Name())
	defer vHostVSockFile.Close()
	VHostVSockDevicePath = vHostVSockFile.Name()

	assert.True(SupportsVsocks())
}

func TestValidCgroupPath(t *testing.T) {
	assert := assert.New(t)

	assert.Equal(DefaultCgroupPath, ValidCgroupPath("../../../"))
	assert.Equal(filepath.Join(DefaultCgroupPath, "foo"), ValidCgroupPath("../../../foo"))
	assert.Equal("/hi", ValidCgroupPath("/../hi"))
	assert.Equal("/hi/foo", ValidCgroupPath("/../hi/foo"))
	assert.Equal(DefaultCgroupPath, ValidCgroupPath(""))
	assert.Equal(DefaultCgroupPath, ValidCgroupPath(""))
	assert.Equal(DefaultCgroupPath, ValidCgroupPath("../"))
	assert.Equal(DefaultCgroupPath, ValidCgroupPath("."))
	assert.Equal(DefaultCgroupPath, ValidCgroupPath("./../"))
	assert.Equal(filepath.Join(DefaultCgroupPath, "o / g"), ValidCgroupPath("o / m /../ g"))
}
