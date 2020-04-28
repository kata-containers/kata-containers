// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"fmt"
	"io/ioutil"
	"math/rand"
	"net"
	"os"
	"path/filepath"
	"testing"
	"time"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
	"github.com/opencontainers/runc/libcontainer/utils"
	"github.com/stretchr/testify/assert"
)

var (
	consolePathTest       = "console-test"
	consoleSocketPathTest = "console-socket-test"
)

func TestGetContainerInfoContainerIDEmptyFailure(t *testing.T) {
	assert := assert.New(t)
	status, _, err := getContainerInfo(context.Background(), "")

	assert.Error(err, "This test should fail because containerID is empty")
	assert.Empty(status.ID, "Expected blank fullID, but got %v", status.ID)
}

func TestGetContainerInfo(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	containerID := testContainerID

	containerStatus := vc.ContainerStatus{
		ID: containerID,
		Annotations: map[string]string{
			vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		},
	}

	path, err := createTempContainerIDMapping(containerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return containerStatus, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	status, sandboxID, err := getContainerInfo(context.Background(), testContainerID)
	assert.NoError(err)
	assert.Equal(sandboxID, sandbox.ID())
	assert.Equal(status, containerStatus)
}

func TestValidCreateParamsContainerIDEmptyFailure(t *testing.T) {
	assert := assert.New(t)
	_, err := validCreateParams(context.Background(), "", "")

	assert.Error(err, "This test should fail because containerID is empty")
	assert.False(vcmock.IsMockError(err))
}

func TestGetExistingContainerInfoContainerIDEmptyFailure(t *testing.T) {
	assert := assert.New(t)
	status, _, err := getExistingContainerInfo(context.Background(), "")

	assert.Error(err, "This test should fail because containerID is empty")
	assert.Empty(status.ID, "Expected blank fullID, but got %v", status.ID)
}

func TestValidCreateParamsContainerIDNotUnique(t *testing.T) {
	assert := assert.New(t)

	testSandboxID2 := testSandboxID + "2"

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)
	err = os.MkdirAll(filepath.Join(ctrsMapTreePath, testContainerID, testSandboxID2), 0750)
	assert.NoError(err)

	_, err = validCreateParams(context.Background(), testContainerID, "")

	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestValidCreateParamsInvalidBundle(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundlePath := filepath.Join(tmpdir, "bundle")

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	_, err = validCreateParams(context.Background(), testContainerID, bundlePath)
	// bundle is ENOENT
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestValidCreateParamsBundleIsAFile(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundlePath := filepath.Join(tmpdir, "bundle")
	err = createEmptyFile(bundlePath)
	assert.NoError(err)

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	_, err = validCreateParams(context.Background(), testContainerID, bundlePath)
	// bundle exists as a file, not a directory
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestSetupConsoleExistingConsolePathSuccessful(t *testing.T) {
	assert := assert.New(t)
	console, err := setupConsole(consolePathTest, "")

	assert.NoError(err)
	assert.Equal(console, consolePathTest, "Got %q, Expecting %q", console, consolePathTest)
}

func TestSetupConsoleExistingConsolePathAndConsoleSocketPathSuccessful(t *testing.T) {
	assert := assert.New(t)
	console, err := setupConsole(consolePathTest, consoleSocketPathTest)

	assert.NoError(err)
	assert.Equal(console, consolePathTest, "Got %q, Expecting %q", console, consolePathTest)
}

func TestSetupConsoleEmptyPathsSuccessful(t *testing.T) {
	assert := assert.New(t)

	console, err := setupConsole("", "")
	assert.NoError(err)
	assert.Empty(console, "Console path should be empty, got %q instead", console)
}

func TestSetupConsoleExistingConsoleSocketPath(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "test-socket")
	assert.NoError(err)
	defer os.RemoveAll(dir)

	sockName := filepath.Join(dir, "console.sock")

	l, err := net.Listen("unix", sockName)
	assert.NoError(err)

	console, err := setupConsole("", sockName)
	assert.NoError(err)

	waitCh := make(chan error)
	go func() {
		conn, err1 := l.Accept()
		if err != nil {
			waitCh <- err1
		}

		uConn, ok := conn.(*net.UnixConn)
		if !ok {
			waitCh <- fmt.Errorf("casting to *net.UnixConn failed")
		}

		f, err1 := uConn.File()
		if err != nil {
			waitCh <- err1
		}

		_, err1 = utils.RecvFd(f)
		waitCh <- err1
	}()

	assert.NotEmpty(console, "Console socket path should not be empty")

	err = <-waitCh
	assert.NoError(err)
}

func TestSetupConsoleNotExistingSocketPathFailure(t *testing.T) {
	assert := assert.New(t)

	console, err := setupConsole("", "unknown-sock-path")
	assert.Error(err, "This test should fail because the console socket path does not exist")
	assert.Empty(console, "This test should fail because the console socket path does not exist")
}

func testNoNeedForOutput(t *testing.T, detach bool, tty bool, expected bool) {
	assert := assert.New(t)
	result := noNeedForOutput(detach, tty)

	assert.Equal(result, expected)
}

func TestNoNeedForOutputDetachTrueTtyTrue(t *testing.T) {
	testNoNeedForOutput(t, true, true, true)
}

func TestNoNeedForOutputDetachFalseTtyTrue(t *testing.T) {
	testNoNeedForOutput(t, false, true, false)
}

func TestNoNeedForOutputDetachFalseTtyFalse(t *testing.T) {
	testNoNeedForOutput(t, false, false, false)
}

func TestNoNeedForOutputDetachTrueTtyFalse(t *testing.T) {
	testNoNeedForOutput(t, true, false, false)
}

func TestIsCgroupMounted(t *testing.T) {
	assert := assert.New(t)

	r := rand.New(rand.NewSource(time.Now().Unix()))
	randPath := fmt.Sprintf("/path/to/random/%d", r.Int63())

	assert.False(isCgroupMounted(randPath), "%s does not exist", randPath)

	assert.False(isCgroupMounted(os.TempDir()), "%s is not a cgroup", os.TempDir())

	cgroupsDirPath = ""
	cgroupRootPath, err := getCgroupsDirPath(procMountInfo)
	if err != nil {
		assert.NoError(err)
	}
	memoryCgroupPath := filepath.Join(cgroupRootPath, "memory")
	if _, err := os.Stat(memoryCgroupPath); os.IsNotExist(err) {
		t.Skipf("memory cgroup does not exist: %s", memoryCgroupPath)
	}

	assert.True(isCgroupMounted(memoryCgroupPath), "%s is a cgroup", memoryCgroupPath)
}

func TestGetCgroupsDirPath(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		contents       string
		expectedResult string
		expectError    bool
	}

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		assert.NoError(err)
	}
	defer os.RemoveAll(dir)

	// make sure tested cgroupsDirPath is existed
	testedCgroupDir := filepath.Join(dir, "weirdCgroup")
	err = os.Mkdir(testedCgroupDir, testDirMode)
	assert.NoError(err)

	weirdCgroupPath := filepath.Join(testedCgroupDir, "memory")

	data := []testData{
		{fmt.Sprintf("num1 num2 num3 / %s num6 num7 - cgroup cgroup rw,memory", weirdCgroupPath), testedCgroupDir, false},
		// cgroup mount is not properly formated, if fields post - less than 3
		{fmt.Sprintf("num1 num2 num3 / %s num6 num7 - cgroup cgroup ", weirdCgroupPath), "", true},
		{"a a a a a a a - b c d", "", true},
		{"a \na b \na b c\na b c d", "", true},
		{"", "", true},
	}

	file := filepath.Join(dir, "mountinfo")

	//file does not exist, should error here
	_, err = getCgroupsDirPath(file)
	assert.Error(err)

	for _, d := range data {
		err := ioutil.WriteFile(file, []byte(d.contents), testFileMode)
		assert.NoError(err)

		cgroupsDirPath = ""
		path, err := getCgroupsDirPath(file)
		if d.expectError {
			assert.Error(err, fmt.Sprintf("got %q, test data: %+v", path, d))
		} else {
			assert.NoError(err, fmt.Sprintf("got %q, test data: %+v", path, d))
		}

		assert.Equal(d.expectedResult, path)
	}
}
