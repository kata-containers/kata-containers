//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
)

const (
	testFileMode = os.FileMode(0640)
	testDirMode  = os.FileMode(0750)
	readmeName   = "README.md"
)

func createFile(file, contents string) error {
	return os.WriteFile(file, []byte(contents), testFileMode)
}

// makeDirs creates two directories below the specified base directory: one is
// an empty director named emptyDirName and the other is named readmeDirName
// and contains a markdown file called "README.md".
func makeDirs(assert *assert.Assertions, baseDir string, readmeDirName, emptyDirName string) {
	readmeDir := filepath.Join(baseDir, readmeDirName)
	err := os.MkdirAll(readmeDir, testDirMode)
	assert.NoError(err)

	readme := filepath.Join(readmeDir, "README.md")

	err = createFile(readme, "# hello")
	assert.NoError(err)

	emptyDir := filepath.Join(baseDir, emptyDirName)
	err = os.MkdirAll(emptyDir, testDirMode)
	assert.NoError(err)
}

func TestDocAddHeading(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		heading     Heading
		expectError bool
	}

	data := []testData{
		{Heading{"", "", "", -1}, true},
		{Heading{"Foo", "", "", -1}, true},
		{Heading{"Foo", "", "", 0}, true},
		{Heading{"Foo", "", "", 1}, true},
		{Heading{"Foo", "", "foo", -1}, true},
		{Heading{"Foo", "", "foo", 0}, true},

		{Heading{"Foo", "", "foo", 1}, false},
		{Heading{"`Foo`", "`Foo`", "foo", 1}, false},
	}

	logger := logrus.WithField("test", "true")

	for i, d := range data {
		doc := newDoc("foo", logger)

		assert.Empty(doc.Headings)

		msg := fmt.Sprintf("test[%d]: %+v\n", i, d)

		err := doc.addHeading(d.heading)
		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.NoError(err, msg)
		assert.NotEmpty(doc.Headings, msg)

		name := d.heading.Name

		result, ok := doc.Headings[name]
		assert.True(ok, msg)

		assert.Equal(d.heading, result, msg)
	}
}

func TestDocAddLink(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		link        Link
		expectError bool
	}

	data := []testData{
		{Link{nil, "", "", "", -1}, true},
		{Link{nil, "foo", "", "", unknownLink}, true},

		{Link{nil, "foo", "", "", internalLink}, false},
		{Link{nil, "http://google.com", "", "", urlLink}, false},
		{Link{nil, "https://google.com", "", "", urlLink}, false},
		{Link{nil, "mailto:me@somewhere.com", "", "", mailLink}, false},
	}

	logger := logrus.WithField("test", "true")

	for i, d := range data {
		doc := newDoc("foo", logger)

		assert.Empty(doc.Links)

		msg := fmt.Sprintf("test[%d]: %+v\n", i, d)

		err := doc.addLink(d.link)
		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.NoError(err, msg)
		assert.NotEmpty(doc.Links, msg)
		addr := d.link.Address

		result := doc.Links[addr][0]
		assert.Equal(result, d.link)
	}
}

func TestDocLinkAddrToPath(t *testing.T) {
	assert := assert.New(t)

	dir, err := os.MkdirTemp("", "")
	assert.NoError(err)

	cwd, err := os.Getwd()
	assert.NoError(err)
	defer os.Chdir(cwd)

	err = os.Chdir(dir)
	assert.NoError(err)
	defer os.RemoveAll(dir)

	savedDocRoot := docRoot
	docRoot = dir

	defer func() {
		docRoot = savedDocRoot

	}()

	mdFile := "bar.md"
	mdPath := filepath.Join("/", mdFile)
	actualMDPath := filepath.Join(dir, mdFile)

	type testData struct {
		linkAddr     string
		expectedPath string
		expectError  bool
	}

	data := []testData{
		{"", "", true},
		{"bar", "bar", false},
		{"bar.md", "bar.md", false},
		{mdPath, actualMDPath, false},
	}

	logger := logrus.WithField("test", "true")
	doc := newDoc("foo", logger)

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v\n", i, d)

		result, err := doc.linkAddrToPath(d.linkAddr)

		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.NoError(err, msg)
		assert.Equal(d.expectedPath, result)
	}
}
