// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package katatestutils

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"

	semver "github.com/blang/semver/v4"
	"github.com/stretchr/testify/assert"
)

func init() {
	kernelVersion, getKernelErr = testGetKernelVersion()
}

func testGetKernelVersion() (version string, err error) {
	const file = "/proc/version"

	bytes, err := os.ReadFile(file)
	if err != nil {
		return "", err
	}

	line := string(bytes)
	fields := strings.Fields(line)

	const minFields = 3

	count := len(fields)

	if count < minFields {
		return "", fmt.Errorf("expected atleast %d fields in file %q, got %d",
			minFields, file, count)
	}

	version = fixKernelVersion(fields[2])

	return version, nil
}

func TestConstraintNotValidKernelVersion(t *testing.T) {
	assert := assert.New(t)

	assert.NotNil(kernelVersion)

	// Generate new kernel versions for testing purposes based on the
	// current kernel version.
	higherVersion, err := incrementVersion(kernelVersion)
	assert.NoError(err)
	assert.NotEqual(kernelVersion, higherVersion)

	lowerVersion, err := decrementVersion(kernelVersion)
	assert.NoError(err)
	assert.NotEqual(kernelVersion, lowerVersion)

	// Antique kernel version numbers.
	//
	// Note: Not all are actually real kernel releases - we're just trying
	// to do a thorough test.
	lowKernelVersions := []string{
		"0.0.0",
		"0.0.1",
		"1.0.0",
		"1.0.6-1.1.0",
		"2.0.0",
		"2.6.0",
		lowerVersion,
	}

	// Host kernel is expected to be newer than all the low kernel versions
	for _, debug := range []bool{true, false} {
		tc := NewTestConstraint(debug)

		for _, ver := range lowKernelVersions {
			result := tc.NotValid(NeedKernelVersionEquals(ver))
			assert.True(result)

			result = tc.NotValid(NeedKernelVersionLE(ver))
			assert.True(result)

			result = tc.NotValid(NeedKernelVersionLT(ver))
			assert.True(result)

			result = tc.NotValid(NeedKernelVersionGT(ver))
			assert.False(result)

			result = tc.NotValid(NeedKernelVersionGE(ver))
			assert.False(result)

			result = tc.NotValid(NeedKernelVersionNotEquals(ver))
			assert.False(result)
		}
	}

	// Ridiculously high kernel version numbers. The host kernel is
	// expected to never reach these values.
	highKernelVersions := []string{
		higherVersion,
		"999.0.0",
		"999.0.999",
		"999.999.999",
		"1024.0.0",
	}

	for _, debug := range []bool{true, false} {
		tc := NewTestConstraint(debug)

		for _, ver := range highKernelVersions {
			result := tc.NotValid(NeedKernelVersionEquals(ver))
			assert.True(result)

			result = tc.NotValid(NeedKernelVersionGE(ver))
			assert.True(result)

			result = tc.NotValid(NeedKernelVersionGT(ver))
			assert.True(result)

			result = tc.NotValid(NeedKernelVersionLE(ver))
			assert.False(result)

			result = tc.NotValid(NeedKernelVersionLT(ver))
			assert.False(result)

			result = tc.NotValid(NeedKernelVersionNotEquals(ver))
			assert.False(result)
		}
	}
}

func TestConstraintHandleKernelVersion(t *testing.T) {
	assert := assert.New(t)

	ver, err := semver.Make(kernelVersion)
	assert.NoError(err)

	newerMajor, err := semverBumpVersion(ver, true, false, false)
	assert.NoError(err)

	newerMinor, err := semverBumpVersion(ver, false, true, false)
	assert.NoError(err)

	newerPatch, err := semverBumpVersion(ver, false, false, true)
	assert.NoError(err)

	// nolint: govet
	type testData struct {
		version     string
		op          Operator
		result      Result
		expectError bool
	}

	data := []testData{
		{"", eqOperator, Result{}, true},
		{"", geOperator, Result{}, true},
		{"", gtOperator, Result{}, true},
		{"", leOperator, Result{}, true},
		{"", ltOperator, Result{}, true},
		{"", neOperator, Result{}, true},

		{kernelVersion, eqOperator, Result{Success: true}, false},
		{kernelVersion, neOperator, Result{Success: false}, false},

		{newerMajor, eqOperator, Result{Success: false}, false},
		{newerMajor, geOperator, Result{Success: false}, false},
		{newerMajor, gtOperator, Result{Success: false}, false},
		{newerMajor, ltOperator, Result{Success: true}, false},
		{newerMajor, leOperator, Result{Success: true}, false},
		{newerMajor, neOperator, Result{Success: true}, false},

		{newerMinor, eqOperator, Result{Success: false}, false},
		{newerMinor, geOperator, Result{Success: false}, false},
		{newerMinor, gtOperator, Result{Success: false}, false},
		{newerMinor, ltOperator, Result{Success: true}, false},
		{newerMinor, leOperator, Result{Success: true}, false},
		{newerMinor, neOperator, Result{Success: true}, false},

		{newerPatch, eqOperator, Result{Success: false}, false},
		{newerPatch, geOperator, Result{Success: false}, false},
		{newerPatch, gtOperator, Result{Success: false}, false},
		{newerPatch, ltOperator, Result{Success: true}, false},
		{newerPatch, leOperator, Result{Success: true}, false},
		{newerPatch, neOperator, Result{Success: true}, false},
	}

	for _, debug := range []bool{true, false} {
		tc := NewTestConstraint(debug)

		for i, d := range data {
			result, err := tc.handleKernelVersion(d.version, d.op)

			msg := fmt.Sprintf("test[%d]: %+v, result: %+v", i, d, result)

			if d.expectError {
				assert.Error(err, msg)
				continue
			}

			assert.Equal(d.result.Success, result.Success, msg)
		}
	}
}
