// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"fmt"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	"syscall"
	"testing"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/stretchr/testify/assert"
)

const (
	testDirMode  = os.FileMode(0750)
	testFileMode = os.FileMode(0640)

	testSandboxID   = "99999999-9999-9999-99999999999999999"
	testContainerID = "1"
)

var (
	testDir = ""
)

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

func TestVerifyContainerID(t *testing.T) {
	assert := assert.New(t)

	for i, d := range ktu.ContainerIDTestData {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		err := VerifyContainerID(d.ID)
		if d.Valid {
			assert.NoError(err, msg)
		} else {
			assert.Error(err, msg)
		}
	}
}
