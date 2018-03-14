// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package main

import (
	"fmt"
	"io/ioutil"
	"math/rand"
	"net"
	"os"
	"path/filepath"
	"reflect"
	"syscall"
	"testing"
	"time"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcMock"
	"github.com/opencontainers/runc/libcontainer/utils"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

var (
	consolePathTest       = "console-test"
	consoleSocketPathTest = "console-socket-test"
)

type cgroupTestDataType struct {
	resource  string
	linuxSpec *specs.LinuxResources
}

var cgroupTestData = []cgroupTestDataType{
	{
		"memory",
		&specs.LinuxResources{
			Memory: &specs.LinuxMemory{},
		},
	},
	{
		"cpu",
		&specs.LinuxResources{
			CPU: &specs.LinuxCPU{},
		},
	},
	{
		"pids",
		&specs.LinuxResources{
			Pids: &specs.LinuxPids{},
		},
	},
	{
		"blkio",
		&specs.LinuxResources{
			BlockIO: &specs.LinuxBlockIO{},
		},
	},
}

func TestGetContainerInfoContainerIDEmptyFailure(t *testing.T) {
	assert := assert.New(t)
	status, _, err := getContainerInfo("")

	assert.Error(err, "This test should fail because containerID is empty")
	assert.Empty(status.ID, "Expected blank fullID, but got %v", status.ID)
}

func TestGetContainerInfo(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	containerID := testContainerID

	containerStatus := vc.ContainerStatus{
		ID: containerID,
		Annotations: map[string]string{
			vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		},
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID:               pod.ID(),
				ContainersStatus: []vc.ContainerStatus{containerStatus},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	status, podID, err := getContainerInfo(testContainerID)
	assert.NoError(err)
	assert.Equal(podID, pod.ID())
	assert.Equal(status, containerStatus)
}

func TestGetContainerInfoMismatch(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	containerID := testContainerID + testContainerID

	containerStatus := vc.ContainerStatus{
		ID: containerID,
		Annotations: map[string]string{
			vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		},
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID:               pod.ID(),
				ContainersStatus: []vc.ContainerStatus{containerStatus},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	_, podID, err := getContainerInfo(testContainerID)
	assert.NoError(err)
	assert.Equal(podID, "")
}

func TestValidCreateParamsContainerIDEmptyFailure(t *testing.T) {
	assert := assert.New(t)
	_, err := validCreateParams("", "")

	assert.Error(err, "This test should fail because containerID is empty")
	assert.False(vcMock.IsMockError(err))
}

func TestGetExistingContainerInfoContainerIDEmptyFailure(t *testing.T) {
	assert := assert.New(t)
	status, _, err := getExistingContainerInfo("")

	assert.Error(err, "This test should fail because containerID is empty")
	assert.Empty(status.ID, "Expected blank fullID, but got %v", status.ID)
}

func TestValidCreateParamsContainerIDNotUnique(t *testing.T) {
	assert := assert.New(t)

	containerID := testContainerID + testContainerID

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					// 2 containers with same ID
					{
						ID: containerID,
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
						},
					},
					{
						ID: containerID,
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
						},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	_, err := validCreateParams(testContainerID, "")

	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestValidCreateParamsContainerIDNotUnique2(t *testing.T) {
	assert := assert.New(t)

	containerID := testContainerID + testContainerID

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: containerID,
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
						},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	_, err := validCreateParams(testContainerID, "")
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestValidCreateParamsInvalidBundle(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundlePath := filepath.Join(tmpdir, "bundle")

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	_, err = validCreateParams(testContainerID, bundlePath)
	// bundle is ENOENT
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestValidCreateParamsBundleIsAFile(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundlePath := filepath.Join(tmpdir, "bundle")
	err = createEmptyFile(bundlePath)
	assert.NoError(err)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	_, err = validCreateParams(testContainerID, bundlePath)
	// bundle exists as a file, not a directory
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func testProcessCgroupsPath(t *testing.T, ociSpec oci.CompatOCISpec, expected []string) {
	assert := assert.New(t)
	result, err := processCgroupsPath(ociSpec, true)

	assert.NoError(err)

	if reflect.DeepEqual(result, expected) == false {
		assert.FailNow("DeepEqual failed", "Result path %q should match the expected one %q", result, expected)
	}
}

func TestProcessCgroupsPathEmptyPathSuccessful(t *testing.T) {
	ociSpec := oci.CompatOCISpec{}

	ociSpec.Linux = &specs.Linux{
		CgroupsPath: "",
	}

	testProcessCgroupsPath(t, ociSpec, []string{})
}

func TestProcessCgroupsPathEmptyResources(t *testing.T) {
	ociSpec := oci.CompatOCISpec{}

	ociSpec.Linux = &specs.Linux{
		CgroupsPath: "foo",
	}

	testProcessCgroupsPath(t, ociSpec, []string{})
}

func TestProcessCgroupsPathRelativePathSuccessful(t *testing.T) {
	relativeCgroupsPath := "relative/cgroups/path"
	cgroupsDirPath = "/foo/runtime/base"

	ociSpec := oci.CompatOCISpec{}

	ociSpec.Linux = &specs.Linux{
		CgroupsPath: relativeCgroupsPath,
	}

	for _, d := range cgroupTestData {
		ociSpec.Linux.Resources = d.linuxSpec

		p := filepath.Join(cgroupsDirPath, d.resource, relativeCgroupsPath)

		testProcessCgroupsPath(t, ociSpec, []string{p})
	}
}

func TestProcessCgroupsPathAbsoluteNoCgroupMountSuccessful(t *testing.T) {
	absoluteCgroupsPath := "/absolute/cgroups/path"
	cgroupsDirPath = "/foo/runtime/base"

	ociSpec := oci.CompatOCISpec{}

	ociSpec.Linux = &specs.Linux{
		CgroupsPath: absoluteCgroupsPath,
	}

	for _, d := range cgroupTestData {
		ociSpec.Linux.Resources = d.linuxSpec

		p := filepath.Join(cgroupsDirPath, d.resource, absoluteCgroupsPath)

		testProcessCgroupsPath(t, ociSpec, []string{p})
	}
}

func TestProcessCgroupsPathAbsoluteNoCgroupMountDestinationFailure(t *testing.T) {
	assert := assert.New(t)
	absoluteCgroupsPath := "/absolute/cgroups/path"

	ociSpec := oci.CompatOCISpec{}

	ociSpec.Mounts = []specs.Mount{
		{
			Type: "cgroup",
		},
	}

	ociSpec.Linux = &specs.Linux{
		CgroupsPath: absoluteCgroupsPath,
	}

	for _, d := range cgroupTestData {
		ociSpec.Linux.Resources = d.linuxSpec
		for _, isPod := range []bool{true, false} {
			_, err := processCgroupsPath(ociSpec, isPod)
			assert.Error(err, "This test should fail because no cgroup mount destination provided")
		}
	}
}

func TestProcessCgroupsPathAbsoluteSuccessful(t *testing.T) {
	assert := assert.New(t)

	if os.Geteuid() != 0 {
		t.Skip(testDisabledNeedRoot)
	}

	memoryResource := "memory"
	absoluteCgroupsPath := "/cgroup/mount/destination"

	cgroupMountDest, err := ioutil.TempDir("", "cgroup-memory-")
	assert.NoError(err)
	defer os.RemoveAll(cgroupMountDest)

	resourceMountPath := filepath.Join(cgroupMountDest, memoryResource)
	err = os.MkdirAll(resourceMountPath, cgroupsDirMode)
	assert.NoError(err)

	err = syscall.Mount("go-test", resourceMountPath, "cgroup", 0, memoryResource)
	assert.NoError(err)
	defer syscall.Unmount(resourceMountPath, 0)

	ociSpec := oci.CompatOCISpec{}

	ociSpec.Linux = &specs.Linux{
		Resources: &specs.LinuxResources{
			Memory: &specs.LinuxMemory{},
		},
		CgroupsPath: absoluteCgroupsPath,
	}

	ociSpec.Mounts = []specs.Mount{
		{
			Type:        "cgroup",
			Destination: cgroupMountDest,
		},
	}

	testProcessCgroupsPath(t, ociSpec, []string{filepath.Join(resourceMountPath, absoluteCgroupsPath)})
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

func TestProcessCgroupsPathForResource(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, specConfig)
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	for _, isPod := range []bool{true, false} {
		_, err := processCgroupsPathForResource(spec, "", isPod)
		assert.Error(err)
		assert.False(vcMock.IsMockError(err))
	}
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
