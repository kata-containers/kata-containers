// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
)

const badFileContents = `
this is not a valid toml file
`

func createBadFile(filename string) error {
	return os.WriteFile(filename, []byte(badFileContents), os.FileMode(0640))
}

const goodFileContents = `
# This file contains baseline expectations
# for checked results by checkmetrics tool.
[[metric]]
# The name of the metrics test, must match
# that of the generated CSV file
name = "boot-times"
type = "json"
description = "measure container lifecycle timings"
# Min and Max values to set a 'range' that
# the median of the CSV Results data must fall
# within (inclusive)
checkvar = ".Results | .[] | .\"to-workload\".Result"
checktype = "mean"
minval = 1.3
maxval = 1.5

# ... repeat this for each metric ...
`

func createGoodFile(filename string) error {
	return os.WriteFile(filename, []byte(goodFileContents), os.FileMode(0640))
}

func TestNewBasefile(t *testing.T) {

	assert := assert.New(t)

	tmpdir, err := os.MkdirTemp("", "cm-")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	// Should fail to load a nil filename
	_, err = newBasefile("")
	assert.NotNil(err, "Did not error on empty filename")

	// Should fail to load a file that does not exist
	_, err = newBasefile("/some/file/that/does/not/exist")
	assert.NotNil(err, "Did not error on non-existent file")

	// Check a badly formed toml file
	badFileName := tmpdir + "badFile.toml"
	err = createBadFile(badFileName)
	assert.NoError(err)
	_, err = newBasefile(badFileName)
	assert.NotNil(err, "Did not error on bad file contents")

	// Check a well formed toml file
	goodFileName := tmpdir + "goodFile.toml"
	err = createGoodFile(goodFileName)
	assert.NoError(err)
	bf, err := newBasefile(goodFileName)
	assert.Nil(err, "Error'd on good file contents")

	// Now check we did load what we expected from the toml
	t.Logf("Entry.Name: %v", bf.Metric[0].Name)
	m := bf.Metric[0]

	assert.Equal("boot-times", m.Name, "data loaded should match")
	assert.Equal("measure container lifecycle timings", m.Description, "data loaded should match")
	assert.Equal("json", m.Type, "data loaded should match")
	assert.Equal("mean", m.CheckType, "data loaded should match")
	assert.Equal(".Results | .[] | .\"to-workload\".Result", m.CheckVar, "data loaded should match")
	assert.Equal(1.3, m.MinVal, "data loaded should match")
	assert.Equal(1.5, m.MaxVal, "data loaded should match")
	// Gap has not been calculated yet...
	assert.Equal(0.0, m.Gap, "data loaded should match")
}
