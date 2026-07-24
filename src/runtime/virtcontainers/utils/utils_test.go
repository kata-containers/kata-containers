// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"fmt"
	"math"
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

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/cpuset"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
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

// TestIsDockerContainer validates hook-detection logic in isolation.
// End-to-end Docker→containerd→kata integration is covered by
// external tests (see tests/integration/kubernetes/).
func TestIsDockerContainer(t *testing.T) {
	assert := assert.New(t)

	// nil spec
	assert.False(IsDockerContainer(nil))

	// nil hooks
	assert.False(IsDockerContainer(&specs.Spec{}))

	// Unrelated prestart hook
	ociSpec := &specs.Spec{
		Hooks: &specs.Hooks{
			Prestart: []specs.Hook{ //nolint:all
				{Args: []string{"haha"}},
			},
		},
	}
	assert.False(IsDockerContainer(ociSpec))

	// Prestart hook with libnetwork (Docker < 26)
	ociSpec.Hooks.Prestart = append(ociSpec.Hooks.Prestart, specs.Hook{ //nolint:all
		Args: []string{"libnetwork-xxx"},
	})
	assert.True(IsDockerContainer(ociSpec))

	// CreateRuntime hook with libnetwork (Docker >= 26)
	ociSpec2 := &specs.Spec{
		Hooks: &specs.Hooks{
			CreateRuntime: []specs.Hook{
				{Args: []string{"/usr/bin/docker-proxy", "libnetwork-setkey", "abc123", "ctrl"}},
			},
		},
	}
	assert.True(IsDockerContainer(ociSpec2))

	// CreateRuntime hook without libnetwork
	ociSpec3 := &specs.Spec{
		Hooks: &specs.Hooks{
			CreateRuntime: []specs.Hook{
				{Args: []string{"/some/other/hook"}},
			},
		},
	}
	assert.False(IsDockerContainer(ociSpec3))
}

// TestDockerNetnsPath validates netns path discovery from OCI hook args.
// This does not test the actual namespace opening or endpoint scanning;
// see integration tests for full-path coverage.
func TestDockerNetnsPath(t *testing.T) {
	assert := assert.New(t)

	// Valid 64-char hex sandbox IDs for test cases.
	validID := strings.Repeat("ab", 32)        // 64 hex chars
	validID2 := strings.Repeat("cd", 32)       // another 64 hex chars
	invalidShortID := "abc123"                 // too short
	invalidUpperID := strings.Repeat("AB", 32) // uppercase rejected

	// nil spec
	assert.Equal("", DockerNetnsPath(nil))

	// nil hooks
	assert.Equal("", DockerNetnsPath(&specs.Spec{}))

	// Hook without libnetwork-setkey
	spec := &specs.Spec{
		Hooks: &specs.Hooks{
			Prestart: []specs.Hook{ //nolint:all
				{Args: []string{"/some/binary", "unrelated"}},
			},
		},
	}
	assert.Equal("", DockerNetnsPath(spec))

	// Prestart hook with libnetwork-setkey but sandbox ID too short (rejected by regex)
	spec = &specs.Spec{
		Hooks: &specs.Hooks{
			Prestart: []specs.Hook{ //nolint:all
				{Args: []string{"/usr/bin/proxy", "libnetwork-setkey", invalidShortID, "ctrl"}},
			},
		},
	}
	assert.Equal("", DockerNetnsPath(spec))

	// Prestart hook with libnetwork-setkey but uppercase hex (rejected by regex)
	spec = &specs.Spec{
		Hooks: &specs.Hooks{
			Prestart: []specs.Hook{ //nolint:all
				{Args: []string{"/usr/bin/proxy", "libnetwork-setkey", invalidUpperID, "ctrl"}},
			},
		},
	}
	assert.Equal("", DockerNetnsPath(spec))

	// Prestart hook with valid sandbox ID but netns file doesn't exist on disk
	spec = &specs.Spec{
		Hooks: &specs.Hooks{
			Prestart: []specs.Hook{ //nolint:all
				{Args: []string{"/usr/bin/proxy", "libnetwork-setkey", validID, "ctrl"}},
			},
		},
	}
	assert.Equal("", DockerNetnsPath(spec))

	// Prestart hook with libnetwork-setkey and existing netns file — success path
	tmpDir := t.TempDir()
	fakeNsDir := filepath.Join(tmpDir, "netns")
	err := os.MkdirAll(fakeNsDir, 0755)
	assert.NoError(err)
	fakeNsFile := filepath.Join(fakeNsDir, validID)
	err = os.WriteFile(fakeNsFile, []byte{}, 0644)
	assert.NoError(err)

	// Temporarily override dockerNetnsPrefixes so DockerNetnsPath can find
	// the netns file we created under the temp directory.
	origPrefixes := dockerNetnsPrefixes
	dockerNetnsPrefixes = []string{fakeNsDir + "/"}
	defer func() { dockerNetnsPrefixes = origPrefixes }()

	spec = &specs.Spec{
		Hooks: &specs.Hooks{
			Prestart: []specs.Hook{ //nolint:all
				{Args: []string{"/usr/bin/proxy", "libnetwork-setkey", validID, "ctrl"}},
			},
		},
	}
	assert.Equal(fakeNsFile, DockerNetnsPath(spec))

	// Sandbox ID that is a directory rather than a regular file — must be rejected
	dirID := validID2
	err = os.MkdirAll(filepath.Join(fakeNsDir, dirID), 0755)
	assert.NoError(err)
	spec = &specs.Spec{
		Hooks: &specs.Hooks{
			Prestart: []specs.Hook{ //nolint:all
				{Args: []string{"/usr/bin/proxy", "libnetwork-setkey", dirID, "ctrl"}},
			},
		},
	}
	assert.Equal("", DockerNetnsPath(spec))

	// CreateRuntime hook with valid sandbox ID — file doesn't exist
	validID3 := strings.Repeat("ef", 32)
	spec = &specs.Spec{
		Hooks: &specs.Hooks{
			CreateRuntime: []specs.Hook{
				{Args: []string{"/usr/bin/proxy", "libnetwork-setkey", validID3, "ctrl"}},
			},
		},
	}
	assert.Equal("", DockerNetnsPath(spec))

	// Hook with libnetwork-setkey as last arg (no sandbox ID follows) — no panic
	spec = &specs.Spec{
		Hooks: &specs.Hooks{
			Prestart: []specs.Hook{ //nolint:all
				{Args: []string{"libnetwork-setkey"}},
			},
		},
	}
	assert.Equal("", DockerNetnsPath(spec))

	// Empty args slice
	spec = &specs.Spec{
		Hooks: &specs.Hooks{
			Prestart: []specs.Hook{ //nolint:all
				{Args: []string{}},
			},
		},
	}
	assert.Equal("", DockerNetnsPath(spec))
}

func TestDistributeVCPUsProportionallySymmetric(t *testing.T) {
	assert := assert.New(t)
	nodes := []types.GuestNUMANode{
		{HostCPUs: "0-3"},
		{HostCPUs: "4-7"},
	}
	dist, err := DistributeVCPUsProportionally(nodes, 8)
	assert.NoError(err)
	assert.Equal([]uint32{4, 4}, dist)
}

func TestDistributeVCPUsProportionallyAsymmetric(t *testing.T) {
	assert := assert.New(t)
	nodes := []types.GuestNUMANode{
		{HostCPUs: "0-7"},
		{HostCPUs: "8-9"},
	}
	dist, err := DistributeVCPUsProportionally(nodes, 10)
	assert.NoError(err)
	assert.Equal([]uint32{8, 2}, dist)
}

func TestDistributeVCPUsProportionallyMinOnePerNode(t *testing.T) {
	assert := assert.New(t)
	nodes := []types.GuestNUMANode{
		{HostCPUs: "0-99"},
		{HostCPUs: "100"},
	}
	dist, err := DistributeVCPUsProportionally(nodes, 2)
	assert.NoError(err)
	assert.Equal(uint32(1), dist[0])
	assert.Equal(uint32(1), dist[1])
}

func TestDistributeVCPUsProportionallyThreeNodes(t *testing.T) {
	assert := assert.New(t)
	nodes := []types.GuestNUMANode{
		{HostCPUs: "0-5"},
		{HostCPUs: "6-8"},
		{HostCPUs: "9"},
	}
	// 6+3+1=10 host CPUs, 10 vCPUs: proportional = 6, 3, 1
	dist, err := DistributeVCPUsProportionally(nodes, 10)
	assert.NoError(err)
	assert.Equal([]uint32{6, 3, 1}, dist)
}

func TestDistributeVCPUsProportionallyTooFewVCPUs(t *testing.T) {
	assert := assert.New(t)
	nodes := []types.GuestNUMANode{
		{HostCPUs: "0"},
		{HostCPUs: "1"},
		{HostCPUs: "2"},
	}
	_, err := DistributeVCPUsProportionally(nodes, 2)
	assert.Error(err)
	assert.Contains(err.Error(), "must be >= NUMA node count")
}

func TestFilterCPUBearingNUMANodes(t *testing.T) {
	assert := assert.New(t)

	// GH200-like topology: one CPU node plus several CPU-less memory nodes.
	nodes := []types.GuestNUMANode{
		{HostNodes: "0", HostCPUs: "0-71"},
		{HostNodes: "1", HostCPUs: ""},
		{HostNodes: "2", HostCPUs: ""},
	}
	filtered := FilterCPUBearingNUMANodes(nodes)
	assert.Equal([]types.GuestNUMANode{{HostNodes: "0", HostCPUs: "0-71"}}, filtered)

	// All CPU-bearing nodes survive unchanged.
	nodes = []types.GuestNUMANode{
		{HostNodes: "0", HostCPUs: "0-3"},
		{HostNodes: "1", HostCPUs: "4-7"},
	}
	assert.Equal(nodes, FilterCPUBearingNUMANodes(nodes))

	// All CPU-less collapses to an empty (non-nil) slice.
	filtered = FilterCPUBearingNUMANodes([]types.GuestNUMANode{{HostCPUs: ""}})
	assert.Empty(filtered)
}

func TestFilterNUMANodesByCPUSet(t *testing.T) {
	assert := assert.New(t)

	nodes := []types.GuestNUMANode{
		{HostNodes: "0", HostCPUs: "0-55,112-167"},
		{HostNodes: "1", HostCPUs: "56-111,168-223"},
	}

	// Sandbox cpuset only from node 0 -> should return 1 node
	sandboxCPUs, _ := cpuset.Parse("1-40,113-152")
	filtered := FilterNUMANodesByCPUSet(nodes, sandboxCPUs)
	assert.Len(filtered, 1)
	assert.Equal("0", filtered[0].HostNodes)

	// Sandbox cpuset from both nodes -> should return 2 nodes
	sandboxCPUs, _ = cpuset.Parse("1-40,56-80")
	filtered = FilterNUMANodesByCPUSet(nodes, sandboxCPUs)
	assert.Len(filtered, 2)

	// Sandbox cpuset only from node 1 -> should return 1 node
	sandboxCPUs, _ = cpuset.Parse("60-70,170-180")
	filtered = FilterNUMANodesByCPUSet(nodes, sandboxCPUs)
	assert.Len(filtered, 1)
	assert.Equal("1", filtered[0].HostNodes)

	// Empty cpuset -> no filtering, return all
	emptyCPUs := cpuset.NewCPUSet()
	filtered = FilterNUMANodesByCPUSet(nodes, emptyCPUs)
	assert.Len(filtered, 2)

	// Single-node host (1 NUMA node) -> returns 1 regardless
	singleNode := []types.GuestNUMANode{
		{HostNodes: "0", HostCPUs: "0-7"},
	}
	sandboxCPUs, _ = cpuset.Parse("0-3")
	filtered = FilterNUMANodesByCPUSet(singleNode, sandboxCPUs)
	assert.Len(filtered, 1)
	assert.Equal("0", filtered[0].HostNodes)
}

func TestTmpfsMaxInodes(t *testing.T) {
	tcs := []struct {
		name     string
		size     uint32
		expected uint64
	}{
		{
			name:     "empty",
			size:     0,
			expected: 0,
		},
		{
			name:     "1 MB",
			size:     1,
			expected: 128,
		},
		{
			name:     "uint32 overflow",
			size:     math.MaxInt32,
			expected: 274877906816,
		},
	}

	for _, tc := range tcs {
		t.Run(tc.name, func(t *testing.T) {
			actual := TmpfsMaxInodes(tc.size)
			assert.Equal(t, tc.expected, actual)
		})
	}
}
