// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"reflect"
	"testing"

	"github.com/stretchr/testify/assert"
)

const (
	testNetmonPath  = "/foo/bar/netmon"
	testRuntimePath = "/foo/bar/runtime"
)

func TestNetmonLogger(t *testing.T) {
	got := netmonLogger()
	expected := virtLog.WithField("subsystem", "netmon")
	assert.True(t, reflect.DeepEqual(expected, got),
		"Got %+v\nExpected %+v", got, expected)
}

func TestPrepareNetMonParams(t *testing.T) {
	// Empty netmon path
	params := netmonParams{}
	got, err := prepareNetMonParams(params)
	assert.NotNil(t, err)
	assert.Equal(t, got, []string{})

	// Empty runtime path
	params.netmonPath = testNetmonPath
	got, err = prepareNetMonParams(params)
	assert.NotNil(t, err)
	assert.Equal(t, got, []string{})

	// Empty sandbox ID
	params.runtime = testRuntimePath
	got, err = prepareNetMonParams(params)
	assert.NotNil(t, err)
	assert.Equal(t, got, []string{})

	// Successful case
	params.sandboxID = testSandboxID
	got, err = prepareNetMonParams(params)
	assert.Nil(t, err)
	expected := []string{testNetmonPath,
		"-r", testRuntimePath,
		"-s", testSandboxID}
	assert.True(t, reflect.DeepEqual(expected, got),
		"Got %+v\nExpected %+v", got, expected)
}

func TestStopNetmon(t *testing.T) {
	pid := -1
	err := stopNetmon(pid)
	assert.Nil(t, err)
}
