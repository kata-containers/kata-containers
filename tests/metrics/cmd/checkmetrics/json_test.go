// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"bytes"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
)

const BadFileContents = `
this is not a valid json file
`

func CreateBadFile(filename string) error {
	return os.WriteFile(filename, []byte(BadFileContents), os.FileMode(0640))
}

const GoodFileContents = `
{
	"env" : {
		"Runtime": "/usr/share/defaults/kata-containers/configuration.toml",
		"RuntimeVersion": "0.1.0",
		"Hypervisor": "/usr/bin/qemu-lite-system-x86_64",
		"HypervisorVersion": "  QEMU emulator version 2.7.0, Copyright (c) 2003-2016 Fabrice Bellard and the QEMU Project developers",
		"Shim": "/usr/local/bin/containerd-shim-kata-v2",
		"ShimVersion": "  kata-shim version 2.4.0-rc0"
	},
	"date" : {
		"ns": 1522162042326099526,
		"Date": "2018-03-27 15:47:22 +0100"
	},
	"Config": [
			{
		"containers": 20,
		"ksm": 0,
		"auto": "",
		"waittime": 5,
		"image": "busybox",
		"command": "sh"
	}
	],
	"Results": [
	{
		"average": {
			"Result": 10.56,
			"Units" : "KB"
		},
		"qemus": {
			"Result": 1.95,
			"Units" : "KB"
		},
		"shims": {
			"Result": 2.40,
			"Units" : "KB"
		},
		"proxys": {
			"Result": 3.21,
			"Units" : "KB"
		}
	},
	{
		"average": {
			"Result": 20.56,
			"Units" : "KB"
		},
		"qemus": {
			"Result": 4.95,
			"Units" : "KB"
		},
		"shims": {
			"Result": 5.40,
			"Units" : "KB"
		},
		"proxys": {
			"Result": 6.21,
			"Units" : "KB"
		}
	},
	{
		"average": {
			"Result": 30.56,
			"Units" : "KB"
		},
		"qemus": {
			"Result": 7.95,
			"Units" : "KB"
		},
		"shims": {
			"Result": 8.40,
			"Units" : "KB"
		},
		"proxys": {
			"Result": 9.21,
			"Units" : "KB"
		}
	}
	]
}
`

func CreateFile(filename string, contents string) error {
	return os.WriteFile(filename, []byte(contents), os.FileMode(0640))
}

func TestLoad(t *testing.T) {
	assert := assert.New(t)

	// Set up and create a json results file
	tmpdir, err := os.MkdirTemp("", "cm-")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	// Check a badly formed JSON file
	badFileName := tmpdir + "badFile.json"
	err = CreateBadFile(badFileName)
	assert.NoError(err)

	// Set up our basic metrics struct
	var m = metrics{
		Name:        "name",
		Description: "desc",
		Type:        "type",
		CheckType:   "json",
		CheckVar:    ".Results | .[] | .average.Result",
		MinVal:      1.9,
		MaxVal:      2.1,
		Gap:         0,
		stats: statistics{
			Results:     []float64{1.0, 2.0, 3.0},
			Iterations:  0,
			Mean:        0.0,
			Min:         0.0,
			Max:         0.0,
			Range:       0.0,
			RangeSpread: 0.0,
			SD:          0.0,
			CoV:         0.0}}

	err = (&jsonRecord{}).load(badFileName, &m)
	assert.Error(err, "Did not error on bad file contents")

	// Check the well formed file
	goodFileName := tmpdir + "goodFile.json"
	err = CreateFile(goodFileName, GoodFileContents)
	assert.NoError(err)

	err = (&jsonRecord{}).load(goodFileName, &m)
	assert.NoError(err, "Error'd on good file contents")

	t.Logf("m now %+v", m)

	// And check some of the values we get from that JSON read
	assert.Equal(3, m.stats.Iterations, "Should be equal")
	assert.Equal(10.56, m.stats.Min, "Should be equal")
	assert.Equal(30.56, m.stats.Max, "Should be equal")

	// Check we default to json type
	m2 := m
	m2.CheckType = ""
	err = (&jsonRecord{}).load(goodFileName, &m)
	assert.NoError(err, "Error'd on no type file contents")

}

func TestReadInts(t *testing.T) {
	assert := assert.New(t)

	good := bytes.NewReader([]byte("1 2 3"))
	bad := bytes.NewReader([]byte("1 2 3.0"))

	_, err := readInts(bad)
	assert.Error(err, "Should fail")

	ints, err := readInts(good)
	assert.NoError(err, "Should fail")
	assert.Equal(1, ints[0], "Should be equal")
	assert.Equal(2, ints[1], "Should be equal")
	assert.Equal(3, ints[2], "Should be equal")
}

func TestReadFloats(t *testing.T) {
	assert := assert.New(t)

	good := bytes.NewReader([]byte("1.0 2.0 3.0"))
	bad := bytes.NewReader([]byte("1.0 2.0 blah"))

	_, err := readFloats(bad)
	assert.Error(err, "Should fail")

	floats, err := readFloats(good)
	assert.NoError(err, "Should fail")
	assert.Equal(1.0, floats[0], "Should be equal")
	assert.Equal(2.0, floats[1], "Should be equal")
	assert.Equal(3.0, floats[2], "Should be equal")
}
