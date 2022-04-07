// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	"github.com/stretchr/testify/assert"
)

func TestFileExists(t *testing.T) {
	dir := t.TempDir()

	file := filepath.Join(dir, "foo")

	assert.False(t, katautils.FileExists(file),
		fmt.Sprintf("File %q should not exist", file))

	err := createEmptyFile(file)
	if err != nil {
		t.Fatal(err)
	}

	assert.True(t, katautils.FileExists(file),
		fmt.Sprintf("File %q should exist", file))
}

func TestGetKernelVersion(t *testing.T) {
	type testData struct {
		contents        string
		expectedVersion string
		expectError     bool
	}

	const validVersion = "1.2.3-4.5.x86_64"
	validContents := fmt.Sprintf("Linux version %s blah blah blah ...", validVersion)

	data := []testData{
		{"", "", true},
		{"invalid contents", "", true},
		{"a b c", "c", false},
		{validContents, validVersion, false},
	}

	tmpdir := t.TempDir()

	subDir := filepath.Join(tmpdir, "subdir")
	err := os.MkdirAll(subDir, testDirMode)
	assert.NoError(t, err)

	_, err = getKernelVersion()
	assert.Error(t, err)

	file := filepath.Join(tmpdir, "proc-version")

	// override
	procVersion = file

	_, err = getKernelVersion()
	// ENOENT
	assert.Error(t, err)
	assert.True(t, os.IsNotExist(err))

	for _, d := range data {
		err := createFile(file, d.contents)
		assert.NoError(t, err)

		version, err := getKernelVersion()
		if d.expectError {
			assert.Error(t, err, fmt.Sprintf("%+v", d))
			continue
		} else {
			assert.NoError(t, err, fmt.Sprintf("%+v", d))
			assert.Equal(t, d.expectedVersion, version)
		}
	}
}

func TestGetDistroDetails(t *testing.T) {
	type testData struct {
		clrContents     string
		nonClrContents  string
		expectedName    string
		expectedVersion string
		expectError     bool
	}

	const unknown = "<<unknown>>"

	tmpdir := t.TempDir()

	testOSRelease := filepath.Join(tmpdir, "os-release")
	testOSReleaseClr := filepath.Join(tmpdir, "os-release-clr")

	const clrExpectedName = "clr"
	const clrExpectedVersion = "1.2.3-4"
	clrContents := fmt.Sprintf(`
HELLO=world
NAME="%s"
FOO=bar
VERSION_ID="%s"
`, clrExpectedName, clrExpectedVersion)

	const nonClrExpectedName = "not-clr"
	const nonClrExpectedVersion = "999"
	nonClrContents := fmt.Sprintf(`
HELLO=world
NAME="%s"
FOO=bar
VERSION_ID="%s"
`, nonClrExpectedName, nonClrExpectedVersion)

	subDir := filepath.Join(tmpdir, "subdir")
	err := os.MkdirAll(subDir, testDirMode)
	assert.NoError(t, err)

	// override
	osRelease = subDir

	_, _, err = getDistroDetails()
	assert.Error(t, err)

	// override
	osRelease = testOSRelease
	osReleaseClr = testOSReleaseClr

	_, _, err = getDistroDetails()
	// ENOENT
	assert.NoError(t, err)

	data := []testData{
		{"", "", unknown, unknown, false},
		{"invalid", "", unknown, unknown, false},
		{clrContents, "", clrExpectedName, clrExpectedVersion, false},
		{"", nonClrContents, nonClrExpectedName, nonClrExpectedVersion, false},
		{clrContents, nonClrContents, nonClrExpectedName, nonClrExpectedVersion, false},
	}

	for _, d := range data {
		err := createFile(osRelease, d.nonClrContents)
		assert.NoError(t, err)

		err = createFile(osReleaseClr, d.clrContents)
		assert.NoError(t, err)

		name, version, err := getDistroDetails()
		if d.expectError {
			assert.Error(t, err, fmt.Sprintf("%+v", d))
			continue
		} else {
			assert.NoError(t, err, fmt.Sprintf("%+v", d))
			assert.Equal(t, d.expectedName, name)
			assert.Equal(t, d.expectedVersion, version)
		}
	}
}

func TestUtilsRunCommand(t *testing.T) {
	output, err := utils.RunCommand([]string{"true"})
	assert.NoError(t, err)
	assert.Equal(t, "", output)
}

func TestUtilsRunCommandCaptureStdout(t *testing.T) {
	output, err := utils.RunCommand([]string{"echo", "hello"})
	assert.NoError(t, err)
	assert.Equal(t, "hello", output)
}

func TestUtilsRunCommandIgnoreStderr(t *testing.T) {
	args := []string{"/bin/sh", "-c", "echo foo >&2;exit 0"}

	output, err := utils.RunCommand(args)
	assert.NoError(t, err)
	assert.Equal(t, "", output)
}

func TestUtilsRunCommandInvalidCmds(t *testing.T) {
	invalidCommands := [][]string{
		{""},
		{"", ""},
		{" "},
		{" ", " "},
		{" ", ""},
		{"\\"},
		{"/"},
		{"/.."},
		{"../"},
		{"/tmp"},
		{"\t"},
		{"\n"},
		{"false"},
	}

	for _, args := range invalidCommands {
		output, err := utils.RunCommand(args)
		assert.Error(t, err)
		assert.Equal(t, "", output)
	}
}
