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
	"errors"
	"flag"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcMock"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

const (
	testPID                     = 100
	testConsole                 = "/dev/pts/999"
	testContainerTypeAnnotation = "io.kubernetes.cri-o.ContainerType"
	testSandboxIDAnnotation     = "io.kubernetes.cri-o.SandboxID"
	testContainerTypePod        = "sandbox"
	testContainerTypeContainer  = "container"
)

var testStrPID = fmt.Sprintf("%d", testPID)

func mockCPUSetContent(contents map[string]string) error {
	for filePath, data := range contents {
		if err := writeFile(filePath, data, testFileMode); err != nil {
			return err
		}
	}

	return nil
}

func testCreateCgroupsFilesSuccessful(t *testing.T, cgroupsDirPath string, cgroupsPathList []string, pid int) {
	if err := createCgroupsFiles("foo", cgroupsDirPath, cgroupsPathList, pid); err != nil {
		t.Fatalf("This test should succeed (cgroupsPath %q, pid %d): %s", cgroupsPathList, pid, err)
	}
}

// return the value of the *last* param with the specified key
func findLastParam(key string, params []vc.Param) (string, error) {
	if key == "" {
		return "", errors.New("ERROR: need non-nil key")
	}

	l := len(params)
	if l == 0 {
		return "", errors.New("ERROR: no params")
	}

	for i := l - 1; i >= 0; i-- {
		p := params[i]

		if key == p.Key {
			return p.Value, nil
		}
	}

	return "", fmt.Errorf("no param called %q found", name)
}

func TestCgroupsFilesEmptyCgroupsPathSuccessful(t *testing.T) {
	testCreateCgroupsFilesSuccessful(t, "", []string{}, testPID)
}

func TestCreateCgroupsFilesFailToWriteFile(t *testing.T) {
	if os.Geteuid() == 0 {
		// The os.FileMode(0000) trick doesn't work for root.
		t.Skip(testDisabledNeedNonRoot)
	}

	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	// create the file as a directory to force an error
	file := filepath.Join(tmpdir, "cgroups-file")
	err = os.MkdirAll(file, os.FileMode(0000))
	assert.NoError(err)

	files := []string{file}

	err = createCgroupsFiles("foo", "cgroups-file", files, testPID)
	assert.Error(err)
}

func TestCgroupsFilesNonEmptyCgroupsPathSuccessful(t *testing.T) {
	cgroupsPath, err := ioutil.TempDir(testDir, "cgroups-path-")
	if err != nil {
		t.Fatalf("Could not create temporary cgroups directory: %s", err)
	}

	testCreateCgroupsFilesSuccessful(t, "cgroups-path-", []string{cgroupsPath}, testPID)

	defer os.RemoveAll(cgroupsPath)

	tasksPath := filepath.Join(cgroupsPath, cgroupsTasksFile)
	procsPath := filepath.Join(cgroupsPath, cgroupsProcsFile)

	for _, path := range []string{tasksPath, procsPath} {
		if _, err := os.Stat(path); err != nil {
			t.Fatalf("Path %q should have been created: %s", path, err)
		}

		fileBytes, err := ioutil.ReadFile(path)
		if err != nil {
			t.Fatalf("Could not read %q previously created: %s", path, err)
		}

		if string(fileBytes) != testStrPID {
			t.Fatalf("PID %s read from %q different from expected PID %s", string(fileBytes), path, testStrPID)
		}
	}
}

func TestCreatePIDFileSuccessful(t *testing.T) {
	pidDirPath, err := ioutil.TempDir(testDir, "pid-path-")
	if err != nil {
		t.Fatalf("Could not create temporary PID directory: %s", err)
	}

	pidFilePath := filepath.Join(pidDirPath, "pid-file-path")
	if err := createPIDFile(pidFilePath, testPID); err != nil {
		t.Fatal(err)
	}

	fileBytes, err := ioutil.ReadFile(pidFilePath)
	if err != nil {
		os.RemoveAll(pidFilePath)
		t.Fatalf("Could not read %q: %s", pidFilePath, err)
	}

	if string(fileBytes) != testStrPID {
		os.RemoveAll(pidFilePath)
		t.Fatalf("PID %s read from %q different from expected PID %s", string(fileBytes), pidFilePath, testStrPID)
	}

	os.RemoveAll(pidFilePath)
}

func TestCreatePIDFileEmptyPathSuccessful(t *testing.T) {
	file := ""
	if err := createPIDFile(file, testPID); err != nil {
		t.Fatalf("This test should not fail (pidFilePath %q, pid %d)", file, testPID)
	}
}

func TestCreatePIDFileUnableToRemove(t *testing.T) {
	if os.Geteuid() == 0 {
		// The os.FileMode(0000) trick doesn't work for root.
		t.Skip(testDisabledNeedNonRoot)
	}

	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	subdir := filepath.Join(tmpdir, "dir")
	file := filepath.Join(subdir, "pidfile")

	// stop non-root user from removing the directory later
	err = os.MkdirAll(subdir, os.FileMode(0000))
	assert.NoError(err)

	err = createPIDFile(file, testPID)
	assert.Error(err)

	// let it be deleted
	os.Chmod(subdir, testDirMode)
}

func TestCreatePIDFileUnableToCreate(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	subdir := filepath.Join(tmpdir, "dir")
	file := filepath.Join(subdir, "pidfile")

	err = createPIDFile(file, testPID)

	// subdir doesn't exist
	assert.Error(err)
	os.Chmod(subdir, testDirMode)
}

func TestCreateCLIFunctionNoRuntimeConfig(t *testing.T) {
	assert := assert.New(t)

	app := cli.NewApp()
	ctx := cli.NewContext(app, nil, nil)
	app.Name = "foo"
	ctx.App.Metadata = map[string]interface{}{
		"foo": "bar",
	}

	fn, ok := createCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err := fn(ctx)

	// no runtime config in the Metadata
	assert.Error(err)
}

func TestCreateCLIFunctionSetupConsoleFail(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	subdir := filepath.Join(tmpdir, "dir")

	// does not exist
	consoleSocketPath := filepath.Join(subdir, "console")

	set := flag.NewFlagSet("", 0)

	set.String("console-socket", consoleSocketPath, "")

	app := cli.NewApp()
	ctx := cli.NewContext(app, set, nil)
	app.Name = "foo"

	ctx.App.Metadata = map[string]interface{}{
		"runtimeConfig": runtimeConfig,
	}

	fn, ok := createCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)

	// failed to setup console
	assert.Error(err)
}

func TestCreateCLIFunctionCreateFail(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	set := flag.NewFlagSet("", 0)

	set.String("console-socket", "", "")

	app := cli.NewApp()
	ctx := cli.NewContext(app, set, nil)
	app.Name = "foo"

	ctx.App.Metadata = map[string]interface{}{
		"runtimeConfig": runtimeConfig,
	}

	fn, ok := createCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)

	// create() failed
	assert.Error(err)
}

func TestCreateInvalidArgs(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
		MockContainers: []*vcMock.Container{
			{MockID: testContainerID},
			{MockID: testContainerID},
			{MockID: testContainerID},
		},
	}

	testingImpl.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		return pod, nil
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.CreatePodFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	pidFilePath := filepath.Join(tmpdir, "pidfile.txt")

	type testData struct {
		containerID   string
		bundlePath    string
		console       string
		pidFilePath   string
		detach        bool
		runtimeConfig oci.RuntimeConfig
	}

	data := []testData{
		{"", "", "", "", false, oci.RuntimeConfig{}},
		{"", "", "", "", true, oci.RuntimeConfig{}},
		{"foo", "", "", "", true, oci.RuntimeConfig{}},
		{testContainerID, bundlePath, testConsole, pidFilePath, false, runtimeConfig},
		{testContainerID, bundlePath, testConsole, pidFilePath, true, runtimeConfig},
	}

	for i, d := range data {
		err := create(d.containerID, d.bundlePath, d.console, d.pidFilePath, d.detach, d.runtimeConfig)
		assert.Errorf(err, "test %d (%+v)", i, d)
	}
}

func TestCreateInvalidConfigJSON(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	pidFilePath := filepath.Join(tmpdir, "pidfile.txt")

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	f, err := os.OpenFile(ociConfigFile, os.O_APPEND|os.O_WRONLY, testFileMode)
	assert.NoError(err)

	// invalidate the JSON
	_, err = f.WriteString("{")
	assert.NoError(err)
	f.Close()

	for detach := range []bool{true, false} {
		err := create(testContainerID, bundlePath, testConsole, pidFilePath, true, runtimeConfig)
		assert.Errorf(err, "%+v", detach)
		assert.False(vcMock.IsMockError(err))
	}
}

func TestCreateInvalidContainerType(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	pidFilePath := filepath.Join(tmpdir, "pidfile.txt")

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Force an invalid container type
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = "I-am-not-a-valid-container-type"

	// rewrite the file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for detach := range []bool{true, false} {
		err := create(testContainerID, bundlePath, testConsole, pidFilePath, true, runtimeConfig)
		assert.Errorf(err, "%+v", detach)
		assert.False(vcMock.IsMockError(err))
	}
}

func TestCreateContainerInvalid(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	pidFilePath := filepath.Join(tmpdir, "pidfile.txt")

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)

	assert.NoError(err)

	// Force createContainer() to be called.
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeContainer

	// rewrite the file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for detach := range []bool{true, false} {
		err := create(testContainerID, bundlePath, testConsole, pidFilePath, true, runtimeConfig)
		assert.Errorf(err, "%+v", detach)
		assert.False(vcMock.IsMockError(err))
	}
}

func TestCreateProcessCgroupsPathSuccessful(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
		MockContainers: []*vcMock.Container{
			{MockID: testContainerID},
		},
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	testingImpl.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		return pod, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
		testingImpl.CreatePodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	pidFilePath := filepath.Join(tmpdir, "pidfile.txt")

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Force pod-type container
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypePod

	// Set a limit to ensure processCgroupsPath() considers the
	// cgroup part of the spec
	limit := int64(1024 * 1024)
	spec.Linux.Resources.Memory = &specs.LinuxMemory{
		Limit: &limit,
	}

	// Set an absolute path
	spec.Linux.CgroupsPath = "/this/is/a/cgroup/path"

	var mounts []specs.Mount
	foundMount := false

	// Replace the standard cgroup destination with a temporary one.
	for _, mount := range spec.Mounts {
		if mount.Type == "cgroup" {
			foundMount = true
			cgroupDir, err := ioutil.TempDir("", "cgroup")
			assert.NoError(err)

			defer os.RemoveAll(cgroupDir)
			mount.Destination = cgroupDir
		}

		mounts = append(mounts, mount)
	}

	assert.True(foundMount)

	// Replace mounts with the newly created one.
	spec.Mounts = mounts

	// Rewrite the file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for _, detach := range []bool{true, false} {
		err := create(testContainerID, bundlePath, testConsole, pidFilePath, detach, runtimeConfig)
		assert.NoError(err, "detached: %+v", detach)
	}
}

func TestCreateCreateCgroupsFilesFail(t *testing.T) {
	if os.Geteuid() == 0 {
		// The os.FileMode(0000) trick doesn't work for root.
		t.Skip(testDisabledNeedNonRoot)
	}

	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
		MockContainers: []*vcMock.Container{
			{MockID: testContainerID},
		},
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	testingImpl.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		return pod, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
		testingImpl.CreatePodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	pidFilePath := filepath.Join(tmpdir, "pidfile.txt")

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Force pod-type container
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypePod

	// Set a limit to ensure processCgroupsPath() considers the
	// cgroup part of the spec
	limit := int64(1024 * 1024)
	spec.Linux.Resources.Memory = &specs.LinuxMemory{
		Limit: &limit,
	}

	// Override
	cgroupsDirPath = filepath.Join(tmpdir, "cgroups")
	err = os.MkdirAll(cgroupsDirPath, testDirMode)
	assert.NoError(err)

	// Set a relative path
	spec.Linux.CgroupsPath = "./a/relative/path"

	dir := filepath.Join(cgroupsDirPath, "memory")

	// Stop directory from being created
	err = os.MkdirAll(dir, os.FileMode(0000))
	assert.NoError(err)

	// Rewrite the file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for detach := range []bool{true, false} {
		err := create(testContainerID, bundlePath, testConsole, pidFilePath, true, runtimeConfig)
		assert.Errorf(err, "%+v", detach)
		assert.False(vcMock.IsMockError(err))
	}
}

func TestCreateCreateCreatePidFileFail(t *testing.T) {
	if os.Geteuid() == 0 {
		// The os.FileMode(0000) trick doesn't work for root.
		t.Skip(testDisabledNeedNonRoot)
	}

	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
		MockContainers: []*vcMock.Container{
			{MockID: testContainerID},
		},
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	testingImpl.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		return pod, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
		testingImpl.CreatePodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	pidDir := filepath.Join(tmpdir, "pid")
	pidFilePath := filepath.Join(pidDir, "pidfile.txt")

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Force pod-type container
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypePod

	// Set a limit to ensure processCgroupsPath() considers the
	// cgroup part of the spec
	limit := int64(1024 * 1024)
	spec.Linux.Resources.Memory = &specs.LinuxMemory{
		Limit: &limit,
	}

	// Rewrite the file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	// stop the pidfile from being created
	err = os.MkdirAll(pidDir, os.FileMode(0000))
	assert.NoError(err)

	for detach := range []bool{true, false} {
		err := create(testContainerID, bundlePath, testConsole, pidFilePath, true, runtimeConfig)
		assert.Errorf(err, "%+v", detach)
		assert.False(vcMock.IsMockError(err))
	}
}

func TestCreate(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
		MockContainers: []*vcMock.Container{
			{MockID: testContainerID},
		},
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	testingImpl.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		return pod, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
		testingImpl.CreatePodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	pidFilePath := filepath.Join(tmpdir, "pidfile.txt")

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Force pod-type container
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypePod

	// Set a limit to ensure processCgroupsPath() considers the
	// cgroup part of the spec
	limit := int64(1024 * 1024)
	spec.Linux.Resources.Memory = &specs.LinuxMemory{
		Limit: &limit,
	}

	// Rewrite the file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for detach := range []bool{true, false} {
		err := create(testContainerID, bundlePath, testConsole, pidFilePath, true, runtimeConfig)
		assert.NoError(err, "%+v", detach)
	}
}

func TestCreateInvalidKernelParams(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	pidFilePath := filepath.Join(tmpdir, "pidfile.txt")

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Force createPod() to be called.
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypePod

	// rewrite the file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	savedFunc := getKernelParamsFunc
	defer func() {
		getKernelParamsFunc = savedFunc
	}()

	getKernelParamsFunc = func(containerID string) []vc.Param {
		return []vc.Param{
			{
				Key:   "",
				Value: "",
			},
		}
	}

	for detach := range []bool{true, false} {
		err := create(testContainerID, bundlePath, testConsole, pidFilePath, true, runtimeConfig)
		assert.Errorf(err, "%+v", detach)
		assert.False(vcMock.IsMockError(err))
	}
}

func TestCreateCreatePodPodConfigFail(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	quota := int64(0)
	limit := int64(0)

	spec.Linux.Resources.Memory = &specs.LinuxMemory{
		Limit: &limit,
	}

	spec.Linux.Resources.CPU = &specs.LinuxCPU{
		// specify an invalid value
		Quota: &quota,
	}

	_, err = createPod(spec, runtimeConfig, testContainerID, bundlePath, testConsole, true)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestCreateCreatePodFail(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	_, err = createPod(spec, runtimeConfig, testContainerID, bundlePath, testConsole, true)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))
}

func TestCreateCreateContainerContainerConfigFail(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Set invalid container type
	containerType := "你好，世界"
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = containerType

	// rewrite file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for _, disableOutput := range []bool{true, false} {
		_, err = createContainer(spec, testContainerID, bundlePath, testConsole, disableOutput)
		assert.Error(err)
		assert.False(vcMock.IsMockError(err))
		assert.True(strings.Contains(err.Error(), containerType))
	}
}

func TestCreateCreateContainerFail(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// set expected container type and podID
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeContainer
	spec.Annotations[testSandboxIDAnnotation] = testPodID

	// rewrite file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for _, disableOutput := range []bool{true, false} {
		_, err = createContainer(spec, testContainerID, bundlePath, testConsole, disableOutput)
		assert.Error(err)
		assert.True(vcMock.IsMockError(err))
	}
}

func TestCreateCreateContainer(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	testingImpl.CreateContainerFunc = func(podID string, containerConfig vc.ContainerConfig) (vc.VCPod, vc.VCContainer, error) {
		return &vcMock.Pod{}, &vcMock.Container{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
		testingImpl.CreateContainerFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(fileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// set expected container type and podID
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeContainer
	spec.Annotations[testSandboxIDAnnotation] = testPodID

	// rewrite file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for _, disableOutput := range []bool{true, false} {
		_, err = createContainer(spec, testContainerID, bundlePath, testConsole, disableOutput)
		assert.NoError(err)
	}
}

func TestCopyParentCPUSetFail(t *testing.T) {
	assert := assert.New(t)

	cgroupsPath, err := ioutil.TempDir(testDir, "cgroups-path-")
	assert.NoError(err)
	defer os.RemoveAll(cgroupsPath)

	err = copyParentCPUSet(cgroupsPath, testDir)
	assert.Error(err)
}

func TestCopyParentCPUSetSuccessful(t *testing.T) {
	assert := assert.New(t)

	cgroupsPath, err := ioutil.TempDir(testDir, "cgroups-path-")
	assert.NoError(err)
	defer os.RemoveAll(cgroupsPath)

	cgroupsSrcPath := filepath.Join(cgroupsPath, "src")
	err = os.Mkdir(cgroupsSrcPath, testDirMode)
	assert.NoError(err)

	err = mockCPUSetContent(map[string]string{
		filepath.Join(cgroupsSrcPath, "cpuset.cpus"): "0-1",
		filepath.Join(cgroupsSrcPath, "cpuset.mems"): "0-1",
	})
	assert.NoError(err)

	cgroupsDstPath := filepath.Join(cgroupsPath, "dst")
	err = os.Mkdir(cgroupsDstPath, testDirMode)
	assert.NoError(err)

	fd, err := os.Create(filepath.Join(cgroupsDstPath, "cpuset.cpus"))
	assert.NoError(err)
	fd.Close()

	fd, err = os.Create(filepath.Join(cgroupsDstPath, "cpuset.mems"))
	assert.NoError(err)
	fd.Close()

	err = copyParentCPUSet(cgroupsDstPath, cgroupsSrcPath)
	assert.NoError(err)

	currentCpus, currentMems, err := getCPUSet(cgroupsDstPath)
	assert.NoError(err)

	assert.False(isEmptyString(currentCpus))
	assert.False(isEmptyString(currentMems))
}

func TestSetKernelParams(t *testing.T) {
	assert := assert.New(t)

	config := oci.RuntimeConfig{}

	assert.Empty(config.HypervisorConfig.KernelParams)

	err := setKernelParams(testContainerID, &config)
	assert.NoError(err)

	assert.NotEmpty(config.HypervisorConfig.KernelParams)
}

func TestSetKernelParamsUserOptionTakesPriority(t *testing.T) {
	assert := assert.New(t)

	initName := "init"
	initValue := "/sbin/myinit"

	ipName := "ip"
	ipValue := "127.0.0.1"

	params := []vc.Param{
		{Key: initName, Value: initValue},
		{Key: ipName, Value: ipValue},
	}

	hypervisorConfig := vc.HypervisorConfig{
		KernelParams: params,
	}

	// Config containing user-specified kernel parameters
	config := oci.RuntimeConfig{
		HypervisorConfig: hypervisorConfig,
	}

	assert.NotEmpty(config.HypervisorConfig.KernelParams)

	err := setKernelParams(testContainerID, &config)
	assert.NoError(err)

	kernelParams := config.HypervisorConfig.KernelParams

	init, err := findLastParam(initName, kernelParams)
	assert.NoError(err)
	assert.Equal(initValue, init)

	ip, err := findLastParam(ipName, kernelParams)
	assert.NoError(err)
	assert.Equal(ipValue, ip)

}
