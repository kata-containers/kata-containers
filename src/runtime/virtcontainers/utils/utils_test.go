// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"fmt"
	"os"
	"os/exec"
	"path"
	"path/filepath"
	"reflect"
	"strings"
	"syscall"
	"testing"

	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
)

const waitLocalProcessTimeoutSecs = 3

func TestFileCopySuccessful(t *testing.T) {
	assert := assert.New(t)
	fileContent := "testContent"

	srcFile, err := os.CreateTemp("", "test_src_copy")
	assert.NoError(err)
	defer os.Remove(srcFile.Name())
	defer srcFile.Close()

	dstFile, err := os.CreateTemp("", "test_dst_copy")
	assert.NoError(err)
	defer os.Remove(dstFile.Name())

	dstPath := dstFile.Name()

	assert.NoError(dstFile.Close())

	_, err = srcFile.WriteString(fileContent)
	assert.NoError(err)

	err = FileCopy(srcFile.Name(), dstPath)
	assert.NoError(err)

	dstContent, err := os.ReadFile(dstPath)
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
	srcFile, err := os.CreateTemp("", "test_src_copy")
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
	reversed := reverseString(str)
	assert.Equal(reversed, "rtstseT")
}

func TestCleanupFds(t *testing.T) {
	assert := assert.New(t)

	tmpFile, err := os.CreateTemp("", "testFds1")
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

	tmpFile, err := os.CreateTemp("", "test_append_file")
	assert.NoError(err)

	filename := tmpFile.Name()
	defer os.Remove(filename)

	tmpFile.Close()

	testData := []byte("test-data")
	err = WriteToFile(filename, testData)
	assert.NoError(err)

	data, err := os.ReadFile(filename)
	assert.NoError(err)

	assert.True(reflect.DeepEqual(testData, data))
}

func TestCalculateCPUsF(t *testing.T) {
	assert := assert.New(t)

	n := CalculateCPUsF(1, 1)
	expected := float32(1)
	assert.Equal(n, expected)

	n = CalculateCPUsF(1, 0)
	expected = float32(0)
	assert.Equal(n, expected)

	n = CalculateCPUsF(-1, 1)
	expected = float32(0)
	assert.Equal(n, expected)

	n = CalculateCPUsF(500, 1000)
	expected = float32(0.5)
	assert.Equal(n, expected)
}

func TestGetVirtDriveNameInvalidIndex(t *testing.T) {
	assert := assert.New(t)
	_, err := GetVirtDriveName(-1)
	assert.Error(err)
}

func TestGetVirtDriveName(t *testing.T) {
	assert := assert.New(t)
	// nolint: govet
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
	// nolint: govet
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

	// nolint: govet
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
			assert.NoError(err, msg)
		} else {
			assert.Error(err, msg)
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

	vHostVSockFile, err := os.CreateTemp("", "vhost-vsock")
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

func TestWaitLocalProcess(t *testing.T) {
	cfg := []struct {
		command string
		args    []string
		timeout uint
		signal  syscall.Signal
	}{
		{
			"true",
			[]string{},
			waitLocalProcessTimeoutSecs,
			syscall.SIGKILL,
		},
		{
			"sleep",
			[]string{"999"},
			waitLocalProcessTimeoutSecs,
			syscall.SIGKILL,
		},
		{
			"sleep",
			[]string{"999"},
			1,
			syscall.SIGKILL,
		},
	}

	logger := logrus.WithField("foo", "bar")

	for _, opts := range cfg {
		assert := assert.New(t)

		cmd := exec.Command(opts.command, opts.args...)
		err := cmd.Start()
		assert.NoError(err)

		pid := cmd.Process.Pid

		err = WaitLocalProcess(pid, opts.timeout, opts.signal, logger)
		assert.NoError(err)

		_ = cmd.Wait()
	}
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

func TestMkdirAllWithInheritedOwnerSuccessful(t *testing.T) {
	if os.Getuid() != 0 {
		t.Skip("Test disabled as requires root user")
	}
	assert := assert.New(t)
	tmpDir1 := t.TempDir()
	tmpDir2 := t.TempDir()

	testCases := []struct {
		before    func(rootDir string, uid, gid int)
		rootDir   string
		targetDir string
		uid       int
		gid       int
	}{
		{
			before: func(rootDir string, uid, gid int) {
				err := syscall.Chown(rootDir, uid, gid)
				assert.NoError(err)
			},
			rootDir:   tmpDir1,
			targetDir: path.Join(tmpDir1, "foo", "bar"),
			uid:       1234,
			gid:       5678,
		},
		{
			before: func(rootDir string, uid, gid int) {
				// remove the tmpDir2 so the MkdirAllWithInheritedOwner() call creates them from /tmp
				err := os.RemoveAll(tmpDir2)
				assert.NoError(err)
			},
			rootDir:   tmpDir2,
			targetDir: path.Join(tmpDir2, "foo", "bar"),
			uid:       0,
			gid:       0,
		},
	}

	for _, tc := range testCases {
		if tc.before != nil {
			tc.before(tc.rootDir, tc.uid, tc.gid)
		}

		err := MkdirAllWithInheritedOwner(tc.targetDir, 0700)
		assert.NoError(err)
		// tmpDir1: /tmp/TestMkdirAllWithInheritedOwnerSuccessful/001
		// tmpDir2: /tmp/TestMkdirAllWithInheritedOwnerSuccessful/002
		// remove the first two parent "/tmp/TestMkdirAllWithInheritedOwnerSuccessful" from the assertion as it's owned by root
		for _, p := range getAllParentPaths(tc.targetDir)[2:] {
			info, err := os.Stat(p)
			assert.NoError(err)
			assert.True(info.IsDir())
			stat, ok := info.Sys().(*syscall.Stat_t)
			assert.True(ok)
			assert.Equal(tc.uid, int(stat.Uid))
			assert.Equal(tc.gid, int(stat.Gid))
		}
	}
}

func TestChownToParent(t *testing.T) {
	if os.Getuid() != 0 {
		t.Skip("Test disabled as requires root user")
	}
	assert := assert.New(t)
	rootDir := t.TempDir()
	uid := 1234
	gid := 5678
	err := syscall.Chown(rootDir, uid, gid)
	assert.NoError(err)

	targetDir := path.Join(rootDir, "foo")

	err = os.MkdirAll(targetDir, 0700)
	assert.NoError(err)

	err = ChownToParent(targetDir)
	assert.NoError(err)

	info, err := os.Stat(targetDir)
	assert.NoError(err)
	stat, ok := info.Sys().(*syscall.Stat_t)
	assert.True(ok)
	assert.Equal(uid, int(stat.Uid))
	assert.Equal(gid, int(stat.Gid))
}

func TestGetAllParentPaths(t *testing.T) {
	assert := assert.New(t)

	testCases := []struct {
		targetPath string
		parents    []string
	}{
		{
			targetPath: "/",
			parents:    []string{},
		},
		{
			targetPath: ".",
			parents:    []string{},
		},
		{
			targetPath: "foo",
			parents:    []string{"foo"},
		},
		{
			targetPath: "/tmp/bar",
			parents:    []string{"/tmp", "/tmp/bar"},
		},
	}

	for _, tc := range testCases {
		assert.Equal(tc.parents, getAllParentPaths(tc.targetPath))
	}
}

func TestRevertBytes(t *testing.T) {
	assert := assert.New(t)

	//10MB
	testNum := uint64(10000000)
	expectedNum := uint64(10485760)

	num := RevertBytes(testNum)
	assert.Equal(expectedNum, num)
}

func TestIsDockerContainer(t *testing.T) {
	assert := assert.New(t)

	ociSpec := &specs.Spec{
		Hooks: &specs.Hooks{
			Prestart: []specs.Hook{
				{
					Args: []string{
						"haha",
					},
				},
			},
		},
	}
	assert.False(IsDockerContainer(ociSpec))

	ociSpec.Hooks.Prestart = append(ociSpec.Hooks.Prestart, specs.Hook{
		Args: []string{"libnetwork-xxx"},
	})
	assert.True(IsDockerContainer(ociSpec))
}
