// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"errors"
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"path"
	"path/filepath"
	"strings"
	"syscall"
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/stretchr/testify/assert"
)

const (
	testDirMode  = os.FileMode(0750)
	testFileMode = os.FileMode(0640)

	testDisabledNeedRoot    = "Test disabled as requires root user"
	testDisabledNeedNonRoot = "Test disabled as requires non-root user"

	// small docker image used to create root filesystems from
	testDockerImage = "busybox"

	testSandboxID   = "99999999-9999-9999-99999999999999999"
	testContainerID = "1"
	testBundle      = "bundle"
	specConfig      = "config.json"
)

var testDir = ""

func init() {
	var err error

	fmt.Printf("INFO: creating test directory\n")
	testDir, err = ioutil.TempDir("", fmt.Sprintf("%s-", name))
	if err != nil {
		panic(fmt.Sprintf("ERROR: failed to create test directory: %v", err))
	}

	fmt.Printf("INFO: test directory is %v\n", testDir)

	testBundleDir = filepath.Join(testDir, testBundle)
	err = os.MkdirAll(testBundleDir, testDirMode)
	if err != nil {
		panic(fmt.Sprintf("ERROR: failed to create bundle directory %v: %v", testBundleDir, err))
	}

	fmt.Printf("INFO: creating OCI bundle in %v for tests to use\n", testBundleDir)
	err = realMakeOCIBundle(testBundleDir)
	if err != nil {
		panic(fmt.Sprintf("ERROR: failed to create OCI bundle: %v", err))
	}
}

// createOCIConfig creates an OCI configuration (spec) file in
// the bundle directory specified (which must exist).
func createOCIConfig(bundleDir string) error {
	if bundleDir == "" {
		return errors.New("BUG: Need bundle directory")
	}

	if !FileExists(bundleDir) {
		return fmt.Errorf("BUG: Bundle directory %s does not exist", bundleDir)
	}

	var configCmd string

	// Search for a suitable version of runc to use to generate
	// the OCI config file.
	for _, cmd := range []string{"docker-runc", "runc"} {
		fullPath, err := exec.LookPath(cmd)
		if err == nil {
			configCmd = fullPath
			break
		}
	}

	if configCmd == "" {
		return errors.New("Cannot find command to generate OCI config file")
	}

	_, err := RunCommand([]string{configCmd, "spec", "--bundle", bundleDir})
	if err != nil {
		return err
	}

	specFile := filepath.Join(bundleDir, specConfig)
	if !FileExists(specFile) {
		return fmt.Errorf("generated OCI config file does not exist: %v", specFile)
	}

	return nil
}

// realMakeOCIBundle will create an OCI bundle (including the "config.json"
// config file) in the directory specified (which must already exist).
//
// XXX: Note that tests should *NOT* call this function - they should
// XXX: instead call makeOCIBundle().
func realMakeOCIBundle(bundleDir string) error {
	if bundleDir == "" {
		return errors.New("BUG: Need bundle directory")
	}

	if !FileExists(bundleDir) {
		return fmt.Errorf("BUG: Bundle directory %v does not exist", bundleDir)
	}

	err := createOCIConfig(bundleDir)
	if err != nil {
		return err
	}

	// Note the unusual parameter (a directory, not the config
	// file to parse!)
	spec, err := oci.ParseConfigJSON(bundleDir)
	if err != nil {
		return err
	}

	// Determine the rootfs directory name the OCI config refers to
	ociRootPath := spec.Root.Path

	rootfsDir := filepath.Join(bundleDir, ociRootPath)

	if strings.HasPrefix(ociRootPath, "/") {
		return fmt.Errorf("Cannot handle absolute rootfs as bundle must be unique to each test")
	}

	err = createRootfs(rootfsDir)
	if err != nil {
		return err
	}

	return nil
}

// createRootfs creates a minimal root filesystem below the specified
// directory.
func createRootfs(dir string) error {
	err := os.MkdirAll(dir, testDirMode)
	if err != nil {
		return err
	}

	container, err := RunCommand([]string{"docker", "create", testDockerImage})
	if err != nil {
		return err
	}

	cmd1 := exec.Command("docker", "export", container)
	cmd2 := exec.Command("tar", "-C", dir, "-xvf", "-")

	cmd1Stdout, err := cmd1.StdoutPipe()
	if err != nil {
		return err
	}

	cmd2.Stdin = cmd1Stdout

	err = cmd2.Start()
	if err != nil {
		return err
	}

	err = cmd1.Run()
	if err != nil {
		return err
	}

	err = cmd2.Wait()
	if err != nil {
		return err
	}

	// Clean up
	_, err = RunCommand([]string{"docker", "rm", container})
	if err != nil {
		return err
	}

	return nil
}

func createFile(file, contents string) error {
	return ioutil.WriteFile(file, []byte(contents), testFileMode)
}

func createEmptyFile(path string) (err error) {
	return ioutil.WriteFile(path, []byte(""), testFileMode)
}

func TestUtilsResolvePathEmptyPath(t *testing.T) {
	_, err := ResolvePath("")
	assert.Error(t, err)
}

func TestUtilsResolvePathValidPath(t *testing.T) {
	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	target := path.Join(dir, "target")
	linkDir := path.Join(dir, "a/b/c")
	linkFile := path.Join(linkDir, "link")

	err = createEmptyFile(target)
	assert.NoError(t, err)

	absolute, err := filepath.Abs(target)
	assert.NoError(t, err)

	resolvedTarget, err := filepath.EvalSymlinks(absolute)
	assert.NoError(t, err)

	err = os.MkdirAll(linkDir, testDirMode)
	assert.NoError(t, err)

	err = syscall.Symlink(target, linkFile)
	assert.NoError(t, err)

	resolvedLink, err := ResolvePath(linkFile)
	assert.NoError(t, err)

	assert.Equal(t, resolvedTarget, resolvedLink)
}

func TestUtilsResolvePathENOENT(t *testing.T) {
	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}

	target := path.Join(dir, "target")
	linkDir := path.Join(dir, "a/b/c")
	linkFile := path.Join(linkDir, "link")

	err = createEmptyFile(target)
	assert.NoError(t, err)

	err = os.MkdirAll(linkDir, testDirMode)
	assert.NoError(t, err)

	err = syscall.Symlink(target, linkFile)
	assert.NoError(t, err)

	cwd, err := os.Getwd()
	assert.NoError(t, err)
	defer os.Chdir(cwd)

	err = os.Chdir(dir)
	assert.NoError(t, err)

	err = os.RemoveAll(dir)
	assert.NoError(t, err)

	_, err = ResolvePath(filepath.Base(linkFile))
	assert.Error(t, err)
}

func TestFileSize(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir(testDir, "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	file := filepath.Join(dir, "foo")

	// ENOENT
	_, err = fileSize(file)
	assert.Error(err)

	err = createEmptyFile(file)
	assert.NoError(err)

	// zero size
	size, err := fileSize(file)
	assert.NoError(err)
	assert.Equal(size, int64(0))

	msg := "hello"
	msgLen := len(msg)

	err = WriteFile(file, msg, testFileMode)
	assert.NoError(err)

	size, err = fileSize(file)
	assert.NoError(err)
	assert.Equal(size, int64(msgLen))
}

func TestWriteFileErrWriteFail(t *testing.T) {
	assert := assert.New(t)

	err := WriteFile("", "", 0000)
	assert.Error(err)
}

func TestWriteFileErrNoPath(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(dir)

	// attempt to write a file over an existing directory
	err = WriteFile(dir, "", 0000)
	assert.Error(err)
}

func TestGetFileContents(t *testing.T) {
	type testData struct {
		contents string
	}

	data := []testData{
		{""},
		{" "},
		{"\n"},
		{"\n\n"},
		{"\n\n\n"},
		{"foo"},
		{"foo\nbar"},
		{"processor   : 0\nvendor_id   : GenuineIntel\n"},
	}

	dir, err := ioutil.TempDir(testDir, "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	file := filepath.Join(dir, "foo")

	// file doesn't exist
	_, err = GetFileContents(file)
	assert.Error(t, err)

	for _, d := range data {
		// create the file
		err = ioutil.WriteFile(file, []byte(d.contents), testFileMode)
		if err != nil {
			t.Fatal(err)
		}
		defer os.Remove(file)

		contents, err := GetFileContents(file)
		assert.NoError(t, err)
		assert.Equal(t, contents, d.contents)
	}
}

func TestIsEphemeralStorage(t *testing.T) {
	sampleEphePath := "/var/lib/kubelet/pods/366c3a75-4869-11e8-b479-507b9ddd5ce4/volumes/kubernetes.io~empty-dir/cache-volume"
	isEphe := IsEphemeralStorage(sampleEphePath)
	if !isEphe {
		t.Fatalf("Unable to correctly determine volume type")
	}

	sampleEphePath = "/var/lib/kubelet/pods/366c3a75-4869-11e8-b479-507b9ddd5ce4/volumes/cache-volume"
	isEphe = IsEphemeralStorage(sampleEphePath)
	if isEphe {
		t.Fatalf("Unable to correctly determine volume type")
	}
}
