// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"syscall"
	"testing"

	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
)

const waitLocalProcessTimeoutSecs = 3

func TestFileCopySuccessful(t *testing.T) {
	assert := assert.New(t)
	fileContent := "testContent"

	srcFile, err := ioutil.TempFile("", "test_src_copy")
	assert.NoError(err)
	defer os.Remove(srcFile.Name())
	defer srcFile.Close()

	dstFile, err := ioutil.TempFile("", "test_dst_copy")
	assert.NoError(err)
	defer os.Remove(dstFile.Name())

	dstPath := dstFile.Name()

	assert.NoError(dstFile.Close())

	_, err = srcFile.WriteString(fileContent)
	assert.NoError(err)

	err = FileCopy(srcFile.Name(), dstPath)
	assert.NoError(err)

	dstContent, err := ioutil.ReadFile(dstPath)
	assert.NoError(err)
	assert.Equal(string(dstContent), fileContent)

	srcInfo, err := srcFile.Stat()
	assert.NoError(err)

	dstInfo, err := os.Stat(dstPath)
	assert.NoError(err)

	assert.Equal(dstInfo.Mode(), srcInfo.Mode())
	assert.Equal(dstInfo.IsDir(), srcInfo.IsDir())
	assert.Equal(dstInfo.Size(), srcInfo.Size())
}

func TestFileCopySourceEmptyFailure(t *testing.T) {
	assert := assert.New(t)
	err := FileCopy("", "testDst")
	assert.Error(err)
}

func TestFileCopyDestinationEmptyFailure(t *testing.T) {
	assert := assert.New(t)
	err := FileCopy("testSrc", "")
	assert.Error(err)
}

func TestFileCopySourceNotExistFailure(t *testing.T) {
	assert := assert.New(t)
	srcFile, err := ioutil.TempFile("", "test_src_copy")
	assert.NoError(err)

	srcPath := srcFile.Name()
	assert.NoError(srcFile.Close())
	assert.NoError(os.Remove(srcPath))

	err = FileCopy(srcPath, "testDest")
	assert.Error(err)
}

func TestGenerateRandomBytes(t *testing.T) {
	assert := assert.New(t)
	bytesNeeded := 8
	randBytes, err := GenerateRandomBytes(bytesNeeded)
	assert.NoError(err)
	assert.Equal(len(randBytes), bytesNeeded)
}

func TestRevereString(t *testing.T) {
	assert := assert.New(t)
	str := "Teststr"
	reversed := ReverseString(str)
	assert.Equal(reversed, "rtstseT")
}

func TestCleanupFds(t *testing.T) {
	assert := assert.New(t)

	tmpFile, err := ioutil.TempFile("", "testFds1")
	assert.NoError(err)
	filename := tmpFile.Name()
	defer os.Remove(filename)

	numFds := 1
	fds := make([]*os.File, numFds)
	assert.NotNil(fds)
	assert.Equal(len(fds), 1)

	fds[0] = tmpFile

	CleanupFds(fds, 0)
	CleanupFds(fds, 1)

	err = tmpFile.Close()
	assert.Error(err)
}

func TestWriteToFile(t *testing.T) {
	assert := assert.New(t)

	err := WriteToFile("/file-does-not-exist", []byte("test-data"))
	assert.NotNil(err)

	tmpFile, err := ioutil.TempFile("", "test_append_file")
	assert.NoError(err)

	filename := tmpFile.Name()
	defer os.Remove(filename)

	tmpFile.Close()

	testData := []byte("test-data")
	err = WriteToFile(filename, testData)
	assert.NoError(err)

	data, err := ioutil.ReadFile(filename)
	assert.NoError(err)

	assert.True(reflect.DeepEqual(testData, data))
}

func TestCalculateMilliCPUs(t *testing.T) {
	assert := assert.New(t)

	n := CalculateMilliCPUs(1, 1)
	expected := uint32(1000)
	assert.Equal(n, expected)

	n = CalculateMilliCPUs(1, 0)
	expected = uint32(0)
	assert.Equal(n, expected)

	n = CalculateMilliCPUs(-1, 1)
	assert.Equal(n, expected)
}

func TestCaluclateVCpusFromMilliCpus(t *testing.T) {
	assert := assert.New(t)

	n := CalculateVCpusFromMilliCpus(1)
	expected := uint32(1)
	assert.Equal(n, expected)
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
	assert := assert.New(t)
	_, err := GetVirtDriveName(-1)
	assert.Error(err)
}

func TestGetVirtDriveName(t *testing.T) {
	assert := assert.New(t)
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

	for i, test := range tests {
		msg := fmt.Sprintf("test[%d]: %+v", i, test)
		driveName, err := GetVirtDriveName(test.index)
		assert.NoError(err, msg)
		assert.Equal(driveName, test.expectedDrive, msg)
	}
}

func TestGetSCSIIdLun(t *testing.T) {
	assert := assert.New(t)

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

	for i, test := range tests {
		msg := fmt.Sprintf("test[%d]: %+v", i, test)
		scsiID, lun, err := GetSCSIIdLun(test.index)
		assert.NoError(err, msg)
		assert.Equal(scsiID, test.expectedScsiID, msg)
		assert.Equal(lun, test.expectedLun, msg)
	}

	_, _, err := GetSCSIIdLun(-1)
	assert.Error(err)
	_, _, err = GetSCSIIdLun(maxSCSIDevices + 1)
	assert.Error(err)
}

func TestGetSCSIAddress(t *testing.T) {
	assert := assert.New(t)
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

	for i, test := range tests {
		msg := fmt.Sprintf("test[%d]: %+v", i, test)
		scsiAddr, err := GetSCSIAddress(test.index)
		assert.NoError(err, msg)
		assert.Equal(scsiAddr, test.expectedSCSIAddress, msg)
	}

	_, err := GetSCSIAddress(-1)
	assert.Error(err)
}

func TestMakeNameID(t *testing.T) {
	assert := assert.New(t)

	nameID := MakeNameID("testType", "testID", 14)
	expected := "testType-testI"
	assert.Equal(expected, nameID)
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
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		if d.valid {
			assert.NoErrorf(err, "test %d, data %+v", i, d, msg)
		} else {
			assert.Errorf(err, "test %d, data %+v", i, d, msg)
		}

		assert.NotNil(result, msg)
		assert.Equal(d.expected, result, msg)
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

func TestAlignMem(t *testing.T) {
	assert := assert.New(t)

	memSize := MemUnit(1024) * MiB
	blockSize := MemUnit(512) * MiB
	resultMem := memSize.AlignMem(blockSize)
	expected := memSize
	assert.Equal(expected, resultMem)

	memSize = MemUnit(512) * MiB
	blockSize = MemUnit(1024) * MiB
	resultMem = memSize.AlignMem(blockSize)
	expected = blockSize
	assert.Equal(expected, resultMem)

	memSize = MemUnit(1024) * MiB
	blockSize = MemUnit(50) * MiB
	resultMem = memSize.AlignMem(blockSize)
	expected = memSize + (blockSize - (memSize % blockSize))
	assert.Equal(expected, resultMem)
}

func TestToMiB(t *testing.T) {
	assert := assert.New(t)

	memSize := MemUnit(1) * GiB
	result := memSize.ToMiB()
	expected := uint64(1024)
	assert.Equal(expected, result)
}

func TestToBytes(t *testing.T) {
	assert := assert.New(t)

	memSize := MemUnit(1) * GiB
	result := memSize.ToBytes()
	expected := uint64(1073741824)
	assert.Equal(expected, result)
}

func TestWaitLocalProcessInvalidSignal(t *testing.T) {
	assert := assert.New(t)

	const invalidSignal = syscall.Signal(999)

	cmd := exec.Command("sleep", "999")
	err := cmd.Start()
	assert.NoError(err)

	pid := cmd.Process.Pid

	logger := logrus.WithField("foo", "bar")

	err = WaitLocalProcess(pid, waitLocalProcessTimeoutSecs, invalidSignal, logger)
	assert.Error(err)

	err = syscall.Kill(pid, syscall.SIGTERM)
	assert.NoError(err)

	err = cmd.Wait()

	// This will error because we killed the process without the knowledge
	// of exec.Command.
	assert.Error(err)
}

func TestWaitLocalProcessInvalidPid(t *testing.T) {
	assert := assert.New(t)

	invalidPids := []int{-999, -173, -3, -2, -1, 0}

	logger := logrus.WithField("foo", "bar")

	for i, pid := range invalidPids {
		msg := fmt.Sprintf("test[%d]: %v", i, pid)

		err := WaitLocalProcess(pid, waitLocalProcessTimeoutSecs, syscall.Signal(0), logger)
		assert.Error(err, msg)
	}
}

func TestWaitLocalProcessBrief(t *testing.T) {
	assert := assert.New(t)

	cmd := exec.Command("true")
	err := cmd.Start()
	assert.NoError(err)

	pid := cmd.Process.Pid

	logger := logrus.WithField("foo", "bar")

	err = WaitLocalProcess(pid, waitLocalProcessTimeoutSecs, syscall.SIGKILL, logger)
	assert.NoError(err)

	_ = cmd.Wait()
}

func TestWaitLocalProcessLongRunningPreKill(t *testing.T) {
	assert := assert.New(t)

	cmd := exec.Command("sleep", "999")
	err := cmd.Start()
	assert.NoError(err)

	pid := cmd.Process.Pid

	logger := logrus.WithField("foo", "bar")

	err = WaitLocalProcess(pid, waitLocalProcessTimeoutSecs, syscall.SIGKILL, logger)
	assert.NoError(err)

	_ = cmd.Wait()
}

func TestWaitLocalProcessLongRunning(t *testing.T) {
	assert := assert.New(t)

	cmd := exec.Command("sleep", "999")
	err := cmd.Start()
	assert.NoError(err)

	pid := cmd.Process.Pid

	logger := logrus.WithField("foo", "bar")

	// Don't wait for long as the process isn't actually trying to stop,
	// so it will have to timeout and then be killed.
	const timeoutSecs = 1

	err = WaitLocalProcess(pid, timeoutSecs, syscall.Signal(0), logger)
	assert.NoError(err)

	_ = cmd.Wait()
}
