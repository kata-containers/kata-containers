// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"flag"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"regexp"
	"testing"

	"github.com/kata-containers/runtime/pkg/katautils"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

const (
	testPID                     = 100
	testConsole                 = "/dev/pts/999"
	testContainerTypeAnnotation = "io.kubernetes.cri-o.ContainerType"
	testContainerTypeSandbox    = "sandbox"
	testContainerTypeContainer  = "container"
)

var (
	testStrPID      = fmt.Sprintf("%d", testPID)
	ctrsMapTreePath = "/var/run/kata-containers/containers-mapping"
)

func TestCreatePIDFileSuccessful(t *testing.T) {
	pidDirPath, err := ioutil.TempDir(testDir, "pid-path-")
	if err != nil {
		t.Fatalf("Could not create temporary PID directory: %s", err)
	}

	pidFilePath := filepath.Join(pidDirPath, "pid-file-path")
	if err := createPIDFile(context.Background(), pidFilePath, testPID); err != nil {
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
	if err := createPIDFile(context.Background(), file, testPID); err != nil {
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

	err = createPIDFile(context.Background(), file, testPID)
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

	err = createPIDFile(context.Background(), file, testPID)

	// subdir doesn't exist
	assert.Error(err)
	os.Chmod(subdir, testDirMode)
}

func TestCreateCLIFunctionNoRuntimeConfig(t *testing.T) {
	assert := assert.New(t)

	ctx := createCLIContext(nil)
	ctx.App.Name = "foo"
	ctx.App.Metadata["foo"] = "bar"

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

	ctx := createCLIContext(set)
	ctx.App.Name = "foo"

	ctx.App.Metadata["runtimeConfig"] = runtimeConfig

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

	ctx := createCLIContext(set)
	ctx.App.Name = "foo"

	ctx.App.Metadata["runtimeConfig"] = runtimeConfig

	fn, ok := createCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)

	// create() failed
	assert.Error(err)
}

func TestCreateInvalidArgs(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
		MockContainers: []*vcmock.Container{
			{MockID: testContainerID},
			{MockID: testContainerID},
			{MockID: testContainerID},
		},
	}

	testingImpl.CreateSandboxFunc = func(ctx context.Context, sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path
	katautils.SetCtrsMapTreePath(ctrsMapTreePath)

	defer func() {
		testingImpl.CreateSandboxFunc = nil
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
		systemdCgroup bool
		runtimeConfig oci.RuntimeConfig
	}

	data := []testData{
		{"", "", "", "", false, false, oci.RuntimeConfig{}},
		{"", "", "", "", true, true, oci.RuntimeConfig{}},
		{"foo", "", "", "", true, false, oci.RuntimeConfig{}},
		{testContainerID, bundlePath, testConsole, pidFilePath, false, false, runtimeConfig},
		{testContainerID, bundlePath, testConsole, pidFilePath, true, true, runtimeConfig},
	}

	for i, d := range data {
		err := create(context.Background(), d.containerID, d.bundlePath, d.console, d.pidFilePath, d.detach, d.systemdCgroup, d.runtimeConfig)
		assert.Errorf(err, "test %d (%+v)", i, d)
	}
}

func TestCreateInvalidConfigJSON(t *testing.T) {
	assert := assert.New(t)

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path
	katautils.SetCtrsMapTreePath(ctrsMapTreePath)

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
	assert.True(katautils.FileExists(ociConfigFile))

	f, err := os.OpenFile(ociConfigFile, os.O_APPEND|os.O_WRONLY, testFileMode)
	assert.NoError(err)

	// invalidate the JSON
	_, err = f.WriteString("{")
	assert.NoError(err)
	f.Close()

	for detach := range []bool{true, false} {
		err := create(context.Background(), testContainerID, bundlePath, testConsole, pidFilePath, true, true, runtimeConfig)
		assert.Errorf(err, "%+v", detach)
		assert.False(vcmock.IsMockError(err))
		os.RemoveAll(path)
	}
}

func TestCreateInvalidContainerType(t *testing.T) {
	assert := assert.New(t)

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path
	katautils.SetCtrsMapTreePath(ctrsMapTreePath)

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
	assert.True(katautils.FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Force an invalid container type
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = "I-am-not-a-valid-container-type"

	// rewrite the file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for detach := range []bool{true, false} {
		err := create(context.Background(), testContainerID, bundlePath, testConsole, pidFilePath, true, true, runtimeConfig)
		assert.Errorf(err, "%+v", detach)
		assert.False(vcmock.IsMockError(err))
		os.RemoveAll(path)
	}
}

func TestCreateContainerInvalid(t *testing.T) {
	assert := assert.New(t)

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path
	katautils.SetCtrsMapTreePath(ctrsMapTreePath)

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
	assert.True(katautils.FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)

	assert.NoError(err)

	// Force createContainer() to be called.
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeContainer

	// rewrite the file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for detach := range []bool{true, false} {
		err := create(context.Background(), testContainerID, bundlePath, testConsole, pidFilePath, true, true, runtimeConfig)
		assert.Errorf(err, "%+v", detach)
		assert.False(vcmock.IsMockError(err))
		os.RemoveAll(path)
	}
}

func TestCreateProcessCgroupsPathSuccessful(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledNeedNonRoot)
	}

	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
		MockContainers: []*vcmock.Container{
			{MockID: testContainerID},
		},
	}

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path
	katautils.SetCtrsMapTreePath(ctrsMapTreePath)

	testingImpl.CreateSandboxFunc = func(ctx context.Context, sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.CreateSandboxFunc = nil
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
	assert.True(katautils.FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Force sandbox-type container
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeSandbox

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

	err = create(context.Background(), testContainerID, "", testConsole, pidFilePath, false, true, runtimeConfig)
	assert.Error(err, "bundle path not set")

	re := regexp.MustCompile("config.json.*no such file or directory")
	matches := re.FindAllStringSubmatch(err.Error(), -1)
	assert.NotEmpty(matches)

	for _, detach := range []bool{true, false} {
		err := create(context.Background(), testContainerID, bundlePath, testConsole, pidFilePath, detach, true, runtimeConfig)
		assert.NoError(err, "detached: %+v", detach)
		os.RemoveAll(path)
	}
}

func TestCreateCreateCgroupsFilesFail(t *testing.T) {
	if os.Geteuid() == 0 {
		// The os.FileMode(0000) trick doesn't work for root.
		t.Skip(testDisabledNeedNonRoot)
	}

	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
		MockContainers: []*vcmock.Container{
			{MockID: testContainerID},
		},
	}

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path
	katautils.SetCtrsMapTreePath(ctrsMapTreePath)

	testingImpl.CreateSandboxFunc = func(ctx context.Context, sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.CreateSandboxFunc = nil
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
	assert.True(katautils.FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Force sandbox-type container
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeSandbox

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
		err := create(context.Background(), testContainerID, bundlePath, testConsole, pidFilePath, true, true, runtimeConfig)
		assert.Errorf(err, "%+v", detach)
		assert.False(vcmock.IsMockError(err))
		os.RemoveAll(path)
	}
}

func TestCreateCreateCreatePidFileFail(t *testing.T) {
	if os.Geteuid() == 0 {
		// The os.FileMode(0000) trick doesn't work for root.
		t.Skip(testDisabledNeedNonRoot)
	}

	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
		MockContainers: []*vcmock.Container{
			{MockID: testContainerID},
		},
	}

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path
	katautils.SetCtrsMapTreePath(ctrsMapTreePath)

	testingImpl.CreateSandboxFunc = func(ctx context.Context, sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.CreateSandboxFunc = nil
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
	assert.True(katautils.FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Force sandbox-type container
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeSandbox

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
		err := create(context.Background(), testContainerID, bundlePath, testConsole, pidFilePath, true, true, runtimeConfig)
		assert.Errorf(err, "%+v", detach)
		assert.False(vcmock.IsMockError(err))
		os.RemoveAll(path)
	}
}

func TestCreate(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledNeedNonRoot)
	}

	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
		MockContainers: []*vcmock.Container{
			{MockID: testContainerID},
		},
	}

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path
	katautils.SetCtrsMapTreePath(ctrsMapTreePath)

	testingImpl.CreateSandboxFunc = func(ctx context.Context, sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.CreateSandboxFunc = nil
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
	assert.True(katautils.FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Force sandbox-type container
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeSandbox

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
		err := create(context.Background(), testContainerID, bundlePath, testConsole, pidFilePath, true, true, runtimeConfig)
		assert.NoError(err, "%+v", detach)
		os.RemoveAll(path)
	}
}
