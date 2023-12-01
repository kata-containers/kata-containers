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

// createLinkAndCategorise will create a link and categorise it. If
// createLinkManually is set, the link will be created "manually" (without the
// constructor) and categorise() called. If not set, the constructor will be
// used.
func createLinkAndCategorise(assert *assert.Assertions, createLinkManually bool) {
	dir, err := os.MkdirTemp("", "")
	assert.NoError(err)

	cwd, err := os.Getwd()
	assert.NoError(err)
	defer os.Chdir(cwd)

	err = os.Chdir(dir)
	assert.NoError(err)
	defer os.RemoveAll(dir)

	readmeDirName := "dir-with-readme"
	emptyDirName := "empty"
	makeDirs(assert, dir, readmeDirName, emptyDirName)

	readmeDirPath := filepath.Join(readmeDirName, readmeName)

	topLevelReadmeName := "top-level.md"
	topLevelReadmeLink := filepath.Join("/", topLevelReadmeName)

	topLevelReadmePath := filepath.Join(dir, topLevelReadmeName)

	type testData struct {
		linkAddress string

		expectedPath string

		expectedType LinkType
		expectError  bool

		// Set if expectedPath should be checked
		checkPath bool
	}

	docRoot = dir

	data := []testData{
		{"", "", -1, true, false},
		{"a", "", -1, true, false},
		{"a.b", "", -1, true, false},
		{"a#b", "", -1, true, false},

		{"htt://foo", "", -1, true, false},
		{"HTTP://foo", "", -1, true, false},
		{"moohttp://foo", "", -1, true, false},
		{"mailto", "", -1, true, false},
		{"http", "", -1, true, false},
		{"https", "", -1, true, false},

		{"http://foo", "", urlLink, false, false},
		{"https://foo/", "", urlLink, false, false},
		{"https://foo/bar", "", urlLink, false, false},
		{"mailto:me", "", mailLink, false, false},

		{".", "", externalFile, false, false},
		{"/", "", externalFile, false, false},
		{emptyDirName, "", externalFile, false, false},

		{readmeDirName, readmeDirPath, externalLink, false, true},
		{"foo.md", "foo.md", externalLink, false, true},
		{"foo.md#bar", "foo.md", externalLink, false, true},
		{topLevelReadmeLink, topLevelReadmePath, externalLink, false, true},
	}

	logger := logrus.WithField("test", "true")
	description := ""

	for i, d := range data {
		var link Link
		var err error

		doc := newDoc("foo", logger)

		if createLinkManually {
			link = Link{
				Doc:         doc,
				Address:     d.linkAddress,
				Description: description,
			}

			err = link.categorise()
		} else {
			link, err = newLink(doc, d.linkAddress, description)
		}

		msg := fmt.Sprintf("test[%d] manual-link: %v: %+v, link: %+v\n", i, createLinkManually, d, link)

		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.NoError(err, msg)

		assert.Equal(link.Doc, doc)
		assert.Equal(link.Address, d.linkAddress)
		assert.Equal(link.Description, description)
		assert.Equal(link.Type, d.expectedType)

		if d.checkPath {
			assert.Equal(d.expectedPath, link.ResolvedPath)
		}
	}
}

func TestNewLink(t *testing.T) {
	assert := assert.New(t)

	createLinkAndCategorise(assert, false)
}

func TestLinkCategorise(t *testing.T) {
	assert := assert.New(t)

	createLinkAndCategorise(assert, true)
}

func TestLinkHandleImplicitREADME(t *testing.T) {
	assert := assert.New(t)

	dir, err := os.MkdirTemp("", "")
	assert.NoError(err)
	defer os.RemoveAll(dir)

	cwd, err := os.Getwd()
	assert.NoError(err)
	defer os.Chdir(cwd)

	err = os.Chdir(dir)
	assert.NoError(err)
	defer os.RemoveAll(dir)

	readmeDirName := "dir-with-readme"
	emptyDirName := "empty"
	makeDirs(assert, dir, readmeDirName, emptyDirName)

	readmePath := filepath.Join(readmeDirName, readmeName)

	emptyFileName := "empty-file"

	err = createFile(emptyFileName, "")
	assert.NoError(err)

	type testData struct {
		linkAddr     string
		expectedPath string
		expectedType LinkType
		isREADME     bool
		expectError  bool
	}

	data := []testData{
		{"", "", unknownLink, false, true},
		{"foo", "", unknownLink, false, true},
		{emptyFileName, "", unknownLink, false, false},
		{emptyDirName, "", unknownLink, false, false},
		{readmeDirName, readmePath, externalLink, true, false},
	}

	logger := logrus.WithField("test", "true")

	for i, d := range data {
		doc := newDoc("foo", logger)

		link := Link{
			Doc:     doc,
			Address: d.linkAddr,
		}

		msg := fmt.Sprintf("test[%d]: %+v\n", i, d)

		isREADME, err := link.handleImplicitREADME()

		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.NoError(err, msg)
		assert.Equal(isREADME, d.isREADME)
		assert.Equal(isREADME, d.isREADME)
		assert.Equal(link.Address, d.linkAddr)
		assert.Equal(link.Type, d.expectedType)
		assert.Equal(link.ResolvedPath, d.expectedPath)
	}
}
