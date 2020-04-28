// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package katatestutils

import (
	"errors"
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"testing"

	"github.com/blang/semver"
	"github.com/stretchr/testify/assert"
)

const (
	testFileMode    = os.FileMode(0640)
	invalidOperator = 1234

	skipUnknownDistroName = "skipping test as cannot determine distro name"
)

type testDataUID struct {
	uid int
	op  Operator
	c   Constraints
}

type testDataDistro struct {
	distro string
	op     Operator
	c      Constraints
}

var distros = []string{
	"centos",
	"clear-linux-os",
	"debian",
	"fedora",
	"opensuse",
	"rhel",
	"sles",
	"ubuntu",
}

var thisUID = os.Getuid()
var rootUID = 0

// name and version of current distro and kernel version of system tests are
// running on
var distroName string
var distroVersion string
var kernelVersion string

// error saved when attempting to determine distro name+version and kernel
// version.
var getDistroErr error
var getKernelErr error

// true if running as root
var root = thisUID == rootUID

var uidEqualsRootData = testDataUID{
	uid: rootUID,
	op:  eqOperator,
	c: Constraints{
		Operator: eqOperator,
		UID:      rootUID,
		UIDSet:   true,
	},
}

var uidNotEqualsRootData = testDataUID{
	uid: rootUID,
	op:  neOperator,
	c: Constraints{
		Operator: neOperator,
		UID:      rootUID,
		UIDSet:   true,
	},
}

var distroEqualsCurrentData testDataDistro
var distroNotEqualsCurrentData testDataDistro

func init() {
	distroName, distroVersion, getDistroErr = testGetDistro()
	kernelVersion, getKernelErr = testGetKernelVersion()

	distroEqualsCurrentData = testDataDistro{
		distro: distroName,
		op:     eqOperator,
		c: Constraints{
			DistroName: distroName,
			Operator:   eqOperator,
		},
	}

	distroNotEqualsCurrentData = testDataDistro{
		distro: distroName,
		op:     neOperator,
		c: Constraints{
			DistroName: distroName,
			Operator:   neOperator,
		},
	}
}

func fileExists(path string) bool {
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return false
	}

	return true
}

// getAnotherDistro returns a distro name not equal to the one specified.
func getAnotherDistro(distro string) string {
	for _, d := range distros {
		if d != distro {
			return d
		}
	}

	panic(fmt.Sprintf("failed to find a distro different to %s", distro))
}

func checkUIDConstraints(assert *assert.Assertions, a, b Constraints, desc string) {
	msg := fmt.Sprintf("%s: a: %+v, b: %+v", desc, a, b)

	assert.Equal(a.UID, b.UID, msg)
	assert.Equal(a.Operator, b.Operator, msg)
	assert.Equal(a.UIDSet, b.UIDSet, msg)
}

func checkDistroConstraints(assert *assert.Assertions, a, b Constraints, desc string) {
	msg := fmt.Sprintf("%s: a: %+v, b: %+v", desc, a, b)

	assert.Equal(a.DistroName, b.DistroName, msg)
	assert.Equal(a.Operator, b.Operator, msg)
}

func checkKernelConstraint(assert *assert.Assertions, f Constraint, version string, op Operator, msg string) {
	c := Constraints{}

	f(&c)

	assert.Equal(c.KernelVersion, version, msg)
	assert.Equal(c.Operator, op, msg)
}

// runCommand runs a command and returns its output
func runCommand(args ...string) ([]string, error) {
	cmd := exec.Command(args[0], args[1:]...)
	bytes, err := cmd.Output()
	if err != nil {
		return []string{}, err
	}

	output := strings.Split(string(bytes), "\n")

	return output, nil
}

// semverBumpVersion takes an existing semantic version and increments one or
// more parts of it, returning the new version number as a string.
func semverBumpVersion(ver semver.Version, bumpMajor, bumpMinor, bumpPatch bool) (string, error) {
	if bumpMajor {
		err := ver.IncrementMajor()
		if err != nil {
			return "", err
		}
	}

	if bumpMinor {
		err := ver.IncrementMinor()
		if err != nil {
			return "", err
		}
	}

	if bumpPatch {
		err := ver.IncrementPatch()
		if err != nil {
			return "", err
		}
	}

	return ver.String(), nil
}

// changeVersion modifies the specified version and returns the
// string representation. If decrement is true the returned version is smaller
// than the specified version, else it is larger.
func changeVersion(version string, decrement bool) (string, error) {
	operand := int64(1)

	if decrement {
		operand = -1
	}

	// Is it an integer?
	intResult, err := strconv.ParseUint(version, 10, 0)
	if err == nil {
		if intResult == 0 && decrement {
			return "", fmt.Errorf("cannot decrement integer version with value zero")
		}

		return fmt.Sprintf("%d", uint64(int64(intResult)+operand)), nil
	}

	// Is it a float?
	floatResult, err := strconv.ParseFloat(version, 32)
	if err == nil {
		if int(floatResult) == 0 && decrement {
			return "", fmt.Errorf("cannot decrement integer part of floating point version with value zero: %v", version)
		}

		return fmt.Sprintf("%f", floatResult+float64(operand)), nil
	}

	// Not an int nor a float, so it must be a semantic version
	ver, err := semver.Make(version)
	if err != nil {
		// but if not, bail as we've run out of options
		return "", err
	}

	if decrement {
		// the semver package only provides increment operations, so
		// handle decrement ourselves.
		major := ver.Major

		if major == 0 {
			return "", fmt.Errorf("cannot decrement semver with zero major version: %+v", version)
		}

		major--

		ver.Major = major
	} else {
		err = ver.IncrementMajor()
		if err != nil {
			return "", err
		}
	}

	return ver.String(), nil
}

func incrementVersion(version string) (string, error) {
	return changeVersion(version, false)
}

func decrementVersion(version string) (string, error) {
	return changeVersion(version, true)
}

// testGetDistro is an alternative implementation of getDistroDetails() used
// for testing.
func testGetDistro() (name, version string, err error) {
	files := []string{"/etc/os-release", "/usr/lib/os-release"}

	for _, file := range files {
		if !fileExists(file) {
			continue
		}

		output, err := runCommand("grep", "^ID=", file)
		if err != nil {
			return "", "", err
		}

		line := output[0]
		fields := strings.Split(line, "=")
		if name == "" {
			name = strings.Trim(fields[1], `"`)
			name = strings.ToLower(name)
		}

		output, err = runCommand("grep", "^VERSION_ID=", file)
		if err != nil {
			return "", "", err
		}

		line = output[0]
		fields = strings.Split(line, "=")
		if version == "" {
			version = strings.Trim(fields[1], `"`)
			version = strings.ToLower(version)
		}
	}

	if name != "" && version != "" {
		return name, version, nil
	}

	if name == "" {
		return "", "", errUnknownDistroName
	}

	if version == "" {
		return "", "", errUnknownDistroVersion
	}

	return "", "", errors.New("BUG: something bad happened")
}

func testGetKernelVersion() (version string, err error) {
	const file = "/proc/version"

	bytes, err := ioutil.ReadFile(file)
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

func TestOperatorString(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		op    Operator
		value string
	}

	data := []testData{
		{eqOperator, "=="},
		{neOperator, "!="},
	}

	for i, d := range data {
		value := d.op.String()

		assert.Equal(value, d.value, "test[%d]: %+v", i, d)
	}
}

func TestNewTestConstraint(t *testing.T) {
	if getDistroErr != nil {
		t.Skipf("skipping as unable to determine distro name/version: %v",
			getDistroErr)
	}

	if getKernelErr != nil {
		t.Skipf("skipping as unable to determine kernel version: %v",
			getKernelErr)
	}

	assert := assert.New(t)

	for i, debug := range []bool{true, false} {
		c := NewTestConstraint(debug)

		msg := fmt.Sprintf("test[%d]: debug: %v, constraint: %+v", i, debug, c)

		assert.Equal(debug, c.Debug, msg)

		assert.Equal(distroName, c.DistroName, msg)
		assert.Equal(distroVersion, c.DistroVersion, msg)
		assert.Equal(kernelVersion, c.KernelVersion, msg)
		assert.Equal(thisUID, c.ActualEUID)

		toCheck := []string{
			distroName,
			distroVersion,
			kernelVersion,
			c.DistroName,
			c.DistroVersion,
			c.KernelVersion,
		}

		for _, str := range toCheck {
			assert.NotNil(str, msg)
		}
	}
}

func TestGetFileContents(t *testing.T) {
	assert := assert.New(t)

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
	}

	dir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(dir)

	file := filepath.Join(dir, "foo")

	// file doesn't exist
	_, err = getFileContents(file)
	assert.Error(err)

	for _, d := range data {
		// create the file
		err = ioutil.WriteFile(file, []byte(d.contents), testFileMode)
		assert.NoError(err)
		defer os.Remove(file)

		contents, err := getFileContents(file)
		assert.NoError(err)
		assert.Equal(contents, d.contents)
	}
}

func TestGetDistroDetails(t *testing.T) {
	assert := assert.New(t)

	if getDistroErr == errUnknownDistroName {
		t.Skip(skipUnknownDistroName)
	}

	assert.NoError(getDistroErr)
	assert.NotNil(distroName)
	assert.NotNil(distroVersion)

	name, version, err := getDistroDetails()
	assert.NoError(err)
	assert.NotNil(name)
	assert.NotNil(version)

	assert.Equal(name, distroName)
	assert.Equal(version, distroVersion)
}

func TestGetKernelVersion(t *testing.T) {
	assert := assert.New(t)

	assert.NoError(getKernelErr)
	assert.NotNil(kernelVersion)

	version, err := getKernelVersion()
	assert.NoError(err)
	assert.NotNil(version)

	assert.Equal(version, kernelVersion)
}

func TestConstraintHandleDistroName(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		distro      string
		op          Operator
		result      Result
		expectError bool
	}

	distroName, _, err := testGetDistro()
	if err != nil && err == errUnknownDistroName {
		t.Skip(skipUnknownDistroName)
	}

	// Look for the first distro that is not the same as the distro this
	// test is currently running on.
	differentDistro := getAnotherDistro(distroName)

	data := []testData{
		{"", eqOperator, Result{}, true},
		{"", neOperator, Result{}, true},
		{"", invalidOperator, Result{}, true},
		{distroName, invalidOperator, Result{}, true},
		{distroName, invalidOperator, Result{}, true},

		{
			distroName,
			eqOperator,
			Result{
				Description: distroName,
				Success:     true,
			},
			false,
		},
		{
			distroName,
			neOperator,
			Result{
				Description: distroName,
				Success:     false,
			},
			false,
		},
		{
			differentDistro,
			eqOperator,
			Result{
				Description: differentDistro,
				Success:     false,
			},
			false,
		},

		{
			differentDistro,
			neOperator,
			Result{
				Description: differentDistro,
				Success:     true,
			},
			false,
		},
	}

	for _, debug := range []bool{true, false} {
		tc := NewTestConstraint(debug)

		for i, d := range data {
			result, err := tc.handleDistroName(d.distro, d.op)

			msg := fmt.Sprintf("test[%d]: %+v, result: %+v", i, d, result)

			if d.expectError {
				assert.Error(err, msg)
				continue

			}

			assert.NoError(err, msg)
			assert.Equal(result.Success, d.result.Success, msg)
			assert.NotNil(result.Description, msg)
		}
	}
}

func TestConstraintHandleDistroVersion(t *testing.T) {
	assert := assert.New(t)

	assert.NotNil(distroVersion)

	// Generate a new distro version for testing purposes. Since we don't
	// know the format of this particular distros versioning scheme, we
	// need to calculate it.
	higherVersion, err := incrementVersion(distroVersion)
	assert.NoError(err)
	assert.NotEqual(distroVersion, higherVersion)

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

		{distroVersion, eqOperator, Result{Success: true}, false},
		{higherVersion, eqOperator, Result{Success: false}, false},

		{distroVersion, gtOperator, Result{Success: false}, false},
		{higherVersion, gtOperator, Result{Success: false}, false},

		{distroVersion, geOperator, Result{Success: true}, false},
		{higherVersion, geOperator, Result{Success: false}, false},

		{distroVersion, ltOperator, Result{Success: false}, false},
		{higherVersion, ltOperator, Result{Success: true}, false},

		{distroVersion, leOperator, Result{Success: true}, false},
		{higherVersion, leOperator, Result{Success: true}, false},

		{distroVersion, neOperator, Result{Success: false}, false},
		{higherVersion, neOperator, Result{Success: true}, false},
	}

	for _, debug := range []bool{true, false} {
		tc := NewTestConstraint(debug)

		for i, d := range data {
			result, err := tc.handleDistroVersion(d.version, d.op)

			msg := fmt.Sprintf("test[%d]: %+v, result: %+v", i, d, result)

			if d.expectError {
				assert.Error(err, msg)
				continue
			}

			assert.Equal(d.result.Success, result.Success, msg)
		}
	}
}

func TestConstraintHandleVersionType(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		versionName    string
		currentVersion string
		op             Operator
		newVersion     string
		result         Result
		expectError    bool
	}

	data := []testData{
		//----------

		{"", "", eqOperator, "", Result{}, true},

		{"name", "foo", eqOperator, "", Result{}, true},
		{"name", "", eqOperator, "foo", Result{}, true},
		{"name", "1", eqOperator, "", Result{}, true},
		{"name", "", eqOperator, "1", Result{}, true},

		{"name", "1", eqOperator, "1", Result{Success: true}, false},
		{"name", "1", eqOperator, "2", Result{Success: false}, false},
		{"name", "2", eqOperator, "1", Result{Success: false}, false},

		{"name", "3.141", eqOperator, "3.141", Result{Success: true}, false},
		{"name", "4.141", eqOperator, "3.141", Result{Success: false}, false},
		{"name", "3.141", eqOperator, "4.141", Result{Success: false}, false},

		{"name", "3.1.4-1", eqOperator, "3.1.4-1", Result{Success: true}, false},
		{"name", "3.1.4-1", eqOperator, "4.1.4-1", Result{Success: false}, false},
		{"name", "4.1.4-1", eqOperator, "3.1.4-1", Result{Success: false}, false},

		//----------

		{"", "", ltOperator, "", Result{}, true},

		{"name", "foo", ltOperator, "", Result{}, true},
		{"name", "", ltOperator, "foo", Result{}, true},
		{"name", "1", ltOperator, "", Result{}, true},
		{"name", "", ltOperator, "1", Result{}, true},

		{"name", "1", ltOperator, "2", Result{Success: true}, false},
		{"name", "2", ltOperator, "1", Result{Success: false}, false},
		{"name", "1", ltOperator, "1", Result{Success: false}, false},

		{"name", "1.3", ltOperator, "2.3", Result{Success: true}, false},
		{"name", "2.3", ltOperator, "1.3", Result{Success: false}, false},
		{"name", "1.3", ltOperator, "1.3", Result{Success: false}, false},

		{"name", "3.1.4", ltOperator, "3.1.5", Result{Success: true}, false},
		{"name", "3.1.5", ltOperator, "3.1.4", Result{Success: false}, false},
		{"name", "3.1.4", ltOperator, "3.1.4", Result{Success: false}, false},

		//----------

		{"", "", leOperator, "", Result{}, true},

		{"name", "foo", leOperator, "", Result{}, true},
		{"name", "", leOperator, "foo", Result{}, true},
		{"name", "1", leOperator, "", Result{}, true},
		{"name", "", leOperator, "1", Result{}, true},

		{"name", "1", leOperator, "2", Result{Success: true}, false},
		{"name", "2", leOperator, "1", Result{Success: false}, false},
		{"name", "1", leOperator, "1", Result{Success: true}, false},

		{"name", "1.3", leOperator, "2.3", Result{Success: true}, false},
		{"name", "2.3", leOperator, "1.3", Result{Success: false}, false},
		{"name", "1.3", leOperator, "1.3", Result{Success: true}, false},

		{"name", "3.1.4", leOperator, "3.1.5", Result{Success: true}, false},
		{"name", "3.1.5", leOperator, "3.1.4", Result{Success: false}, false},
		{"name", "3.1.4", leOperator, "3.1.4", Result{Success: true}, false},

		//----------

		{"", "", gtOperator, "", Result{}, true},

		{"name", "foo", gtOperator, "", Result{}, true},
		{"name", "", gtOperator, "foo", Result{}, true},
		{"name", "1", gtOperator, "", Result{}, true},
		{"name", "", gtOperator, "1", Result{}, true},

		{"name", "1", gtOperator, "2", Result{Success: false}, false},
		{"name", "2", gtOperator, "1", Result{Success: true}, false},
		{"name", "1", gtOperator, "1", Result{Success: false}, false},

		{"name", "1.3", gtOperator, "2.3", Result{Success: false}, false},
		{"name", "2.3", gtOperator, "1.3", Result{Success: true}, false},
		{"name", "1.3", gtOperator, "1.3", Result{Success: false}, false},

		{"name", "3.1.4", gtOperator, "3.1.5", Result{Success: false}, false},
		{"name", "3.1.5", gtOperator, "3.1.4", Result{Success: true}, false},
		{"name", "3.1.4", gtOperator, "3.1.4", Result{Success: false}, false},

		//----------

		{"", "", geOperator, "", Result{}, true},

		{"name", "foo", geOperator, "", Result{}, true},
		{"name", "", geOperator, "foo", Result{}, true},
		{"name", "1", geOperator, "", Result{}, true},
		{"name", "", geOperator, "1", Result{}, true},

		{"name", "1", geOperator, "2", Result{Success: false}, false},
		{"name", "2", geOperator, "1", Result{Success: true}, false},
		{"name", "1", geOperator, "1", Result{Success: true}, false},

		{"name", "1.3", geOperator, "2.3", Result{Success: false}, false},
		{"name", "2.3", geOperator, "1.3", Result{Success: true}, false},
		{"name", "1.3", geOperator, "1.3", Result{Success: true}, false},

		{"name", "3.1.4", geOperator, "3.1.5", Result{Success: false}, false},
		{"name", "3.1.5", geOperator, "3.1.4", Result{Success: true}, false},
		{"name", "3.1.4", geOperator, "3.1.4", Result{Success: true}, false},

		//----------

		{"", "", neOperator, "", Result{}, true},

		{"name", "foo", neOperator, "", Result{}, true},
		{"name", "", neOperator, "foo", Result{}, true},
		{"name", "1", neOperator, "", Result{}, true},
		{"name", "", neOperator, "1", Result{}, true},

		{"name", "1", neOperator, "2", Result{Success: true}, false},
		{"name", "2", neOperator, "1", Result{Success: true}, false},
		{"name", "1", neOperator, "1", Result{Success: false}, false},

		{"name", "1.3", neOperator, "2.3", Result{Success: true}, false},
		{"name", "2.3", neOperator, "1.3", Result{Success: true}, false},
		{"name", "1.3", neOperator, "1.3", Result{Success: false}, false},

		{"name", "3.1.4", neOperator, "3.1.5", Result{Success: true}, false},
		{"name", "3.1.5", neOperator, "3.1.4", Result{Success: true}, false},
		{"name", "3.1.4", neOperator, "3.1.4", Result{Success: false}, false},

		//----------
	}

	for i, d := range data {
		result, err := handleVersionType(d.versionName, d.currentVersion, d.op, d.newVersion)

		msg := fmt.Sprintf("test[%d]: %+v, result: %+v", i, d, result)

		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.Equal(d.result.Success, result.Success, msg)
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

func TestConstraintHandleUID(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		uid         int
		op          Operator
		result      Result
		expectError bool
	}

	data := []testData{
		{-1, eqOperator, Result{}, true},
		{-1, neOperator, Result{}, true},
		{-2, eqOperator, Result{}, true},
		{-2, neOperator, Result{}, true},
		{rootUID, invalidOperator, Result{}, true},
		{thisUID, invalidOperator, Result{}, true},

		{rootUID, eqOperator, Result{Success: root}, false},
		{rootUID, neOperator, Result{Success: !root}, false},

		{thisUID, eqOperator, Result{Success: true}, false},
		{thisUID, neOperator, Result{Success: false}, false},
	}

	for _, debug := range []bool{true, false} {
		tc := NewTestConstraint(debug)

		for i, d := range data {
			result, err := tc.handleUID(d.uid, d.op)

			msg := fmt.Sprintf("test[%d]: %+v, result: %+v", i, d, result)

			if d.expectError {
				assert.Error(err, msg)
				continue
			}

			assert.NoError(err, msg)
			assert.Equal(result.Success, d.result.Success, msg)
			assert.NotNil(result.Description, msg)
		}
	}
}

func TestConstraintHandleResults(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		result Result
		err    error
	}

	data := []testData{
		{Result{}, errors.New("foo")},

		{Result{Success: true}, nil},
		{Result{Success: false}, nil},
	}

	for _, debug := range []bool{true, false} {
		tc := NewTestConstraint(debug)

		for i, d := range data {
			tc.Passed = nil
			tc.Failed = nil

			msg := fmt.Sprintf("test[%d]: %+v", i, d)

			if d.err != nil {
				assert.Panics(func() {
					tc.handleResults(d.result, d.err)
				}, msg)
				continue
			}

			tc.handleResults(d.result, d.err)

			passedLen := len(tc.Passed)
			failedLen := len(tc.Failed)

			var expectedPassedLen int
			var expectedFailedLen int

			if d.result.Success {
				expectedPassedLen = 1
				expectedFailedLen = 0
			} else {
				expectedPassedLen = 0
				expectedFailedLen = 1
			}

			assert.Equal(passedLen, expectedPassedLen, msg)
			assert.Equal(failedLen, expectedFailedLen, msg)
		}
	}
}

func TestNeedUID(t *testing.T) {
	assert := assert.New(t)

	data := []testDataUID{
		uidEqualsRootData,
		uidNotEqualsRootData,
		{thisUID, eqOperator, Constraints{
			Operator: eqOperator,
			UID:      thisUID,
			UIDSet:   true},
		},
	}

	for i, d := range data {
		c := Constraints{}

		f := NeedUID(d.uid, d.op)
		f(&c)

		desc := fmt.Sprintf("test[%d]: %+v", i, d)
		checkUIDConstraints(assert, c, d.c, desc)
	}
}

func TestNeedRoot(t *testing.T) {
	assert := assert.New(t)

	c := Constraints{}

	f := NeedRoot()
	f(&c)

	checkUIDConstraints(assert, c, uidEqualsRootData.c, "TestNeedRoot")
}

func TestNeedNonRoot(t *testing.T) {
	assert := assert.New(t)

	c := Constraints{}

	f := NeedNonRoot()
	f(&c)

	checkUIDConstraints(assert, c, uidNotEqualsRootData.c, "TestNeedNonRoot")
}

func TestNeedDistroWithOp(t *testing.T) {
	assert := assert.New(t)

	if getDistroErr == errUnknownDistroName {
		t.Skip(skipUnknownDistroName)
	}

	data := []testDataDistro{
		distroEqualsCurrentData,
		distroNotEqualsCurrentData,

		// check name provided is lower-cased
		{
			strings.ToUpper(distroName),
			eqOperator,
			Constraints{
				DistroName: distroName,
				Operator:   eqOperator,
			},
		},
	}

	for i, d := range data {

		c := Constraints{}

		f := NeedDistroWithOp(d.distro, d.op)
		f(&c)

		desc := fmt.Sprintf("test[%d]: %+v, constraints: %+v", i, d, c)
		checkDistroConstraints(assert, d.c, c, desc)
	}
}

func TestNeedDistroEquals(t *testing.T) {
	assert := assert.New(t)

	c := Constraints{}

	f := NeedDistroEquals(distroName)
	f(&c)

	checkDistroConstraints(assert, c, distroEqualsCurrentData.c, "TestNeedDistroEquals")
}

func TestNeedDistroNotEquals(t *testing.T) {
	assert := assert.New(t)

	c := Constraints{}

	f := NeedDistroNotEquals(distroName)
	f(&c)

	checkDistroConstraints(assert, c, distroNotEqualsCurrentData.c, "TestNeedDistroNotEquals")
}

func TestWithIssue(t *testing.T) {
	assert := assert.New(t)

	c := Constraints{}

	issue := "issue"

	f := WithIssue(issue)
	f(&c)

	assert.Equal(c.Issue, issue)
}

func TestNeedKernelVersionWithOp(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		version string
		op      Operator
	}

	version := "version"

	data := []testData{
		{version, eqOperator},
		{version, geOperator},
		{version, gtOperator},
		{version, leOperator},
		{version, ltOperator},
		{version, neOperator},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		c := NeedKernelVersionWithOp(d.version, d.op)

		checkKernelConstraint(assert, c, d.version, d.op, msg)
	}
}

func TestNeedKernelVersion(t *testing.T) {
	assert := assert.New(t)

	version := "version"
	f := NeedKernelVersion(version)
	checkKernelConstraint(assert, f, version, eqOperator, "TestNeedKernelVersion")
}

func TestNeedKernelVersionEquals(t *testing.T) {
	assert := assert.New(t)

	version := "version"
	f := NeedKernelVersionEquals(version)
	checkKernelConstraint(assert, f, version, eqOperator, "TestNeedKernelVersionEquals")
}

func TestNeedKernelVersionLE(t *testing.T) {
	assert := assert.New(t)

	version := "version"
	f := NeedKernelVersionLE(version)
	checkKernelConstraint(assert, f, version, leOperator, "TestNeedKernelVersionLE")
}

func TestNeedKernelVersionLT(t *testing.T) {
	assert := assert.New(t)

	version := "version"
	f := NeedKernelVersionLT(version)
	checkKernelConstraint(assert, f, version, ltOperator, "TestNeedKernelVersionLT")
}

func TestNeedKernelVersionGE(t *testing.T) {
	assert := assert.New(t)

	version := "version"
	f := NeedKernelVersionGE(version)
	checkKernelConstraint(assert, f, version, geOperator, "TestNeedKernelVersionGE")
}

func TestNeedKernelVersionGT(t *testing.T) {
	assert := assert.New(t)

	version := "version"
	f := NeedKernelVersionGT(version)
	checkKernelConstraint(assert, f, version, gtOperator, "TestNeedKernelVersionGT")
}

func TestConstraintNotValid(t *testing.T) {
	assert := assert.New(t)

	for _, debug := range []bool{true, false} {
		tc := NewTestConstraint(debug)

		// Ensure no params is an error
		assert.Panics(func() {
			_ = tc.NotValid()
		})

		// Test specification of a single constraint
		if root {
			result := tc.NotValid(NeedRoot())
			assert.False(result)

			result = tc.NotValid(NeedNonRoot())
			assert.True(result)
		} else {
			result := tc.NotValid(NeedRoot())
			assert.True(result)

			result = tc.NotValid(NeedNonRoot())
			assert.False(result)
		}

		// Now test specification of multiple constraints
		if root {
			result := tc.NotValid(NeedRoot(), NeedDistro(distroName))
			assert.False(result)

			result = tc.NotValid(NeedNonRoot(), NeedDistro(distroName))
			assert.True(result)
		} else {
			result := tc.NotValid(NeedRoot(), NeedDistro(distroName))
			assert.True(result)

			result = tc.NotValid(NeedNonRoot(), NeedDistro(distroName))
			assert.False(result)
		}
	}

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

func TestConstraintNotValidDistroVersion(t *testing.T) {
	assert := assert.New(t)

	assert.NotNil(distroVersion)

	// Generate new distro versions for testing purposes based on the
	// current kernel version.
	higherVersion, err := incrementVersion(distroVersion)
	assert.NoError(err)
	assert.NotEqual(distroVersion, higherVersion)

	lowerVersion, err := decrementVersion(distroVersion)
	assert.NoError(err)
	assert.NotEqual(distroVersion, lowerVersion)

	for _, debug := range []bool{true, false} {
		tc := NewTestConstraint(debug)

		result := tc.NotValid(NeedDistroVersionEquals(higherVersion))
		assert.True(result)

		result = tc.NotValid(NeedDistroVersionEquals(distroVersion))
		assert.False(result)

		result = tc.NotValid(NeedDistroVersionLE(higherVersion))
		assert.False(result)

		result = tc.NotValid(NeedDistroVersionLE(distroVersion))
		assert.False(result)

		result = tc.NotValid(NeedDistroVersionLT(higherVersion))
		assert.False(result)

		result = tc.NotValid(NeedDistroVersionLT(distroVersion))
		assert.True(result)

		result = tc.NotValid(NeedDistroVersionGE(higherVersion))
		assert.True(result)

		result = tc.NotValid(NeedDistroVersionGE(distroVersion))
		assert.False(result)

		result = tc.NotValid(NeedDistroVersionGT(higherVersion))
		assert.True(result)

		result = tc.NotValid(NeedDistroVersionGT(distroVersion))
		assert.True(result)

		result = tc.NotValid(NeedDistroVersionNotEquals(higherVersion))
		assert.False(result)

		result = tc.NotValid(NeedDistroVersionNotEquals(distroVersion))
		assert.True(result)
	}
}

func TestConstraintConstraintValid(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		fn       Constraint
		valid    bool
		expected TestConstraint
	}

	issue := "issue"

	data := []testData{
		{
			WithIssue(issue),
			true,
			TestConstraint{Issue: issue},
		},

		{
			NeedDistroWithOp(distroName, eqOperator),
			true,
			TestConstraint{
				Passed: []Result{
					{Success: true},
				},
			},
		},
		{
			NeedDistroWithOp(distroName, neOperator),
			false,
			TestConstraint{
				Failed: []Result{
					{Success: false},
				},
			},
		},
		{
			NeedDistroWithOp(getAnotherDistro(distroName), eqOperator),
			false,
			TestConstraint{
				Failed: []Result{
					{Success: false},
				},
			},
		},
		{
			NeedDistroWithOp(getAnotherDistro(distroName), neOperator),
			true,
			TestConstraint{
				Failed: []Result{
					{Success: true},
				},
			},
		},

		{
			NeedDistroEquals(distroName),
			true,
			TestConstraint{
				Passed: []Result{
					{Success: true},
				},
			},
		},
		{
			NeedDistroEquals(getAnotherDistro(distroName)),
			false,
			TestConstraint{
				Failed: []Result{
					{Success: false},
				},
			},
		},

		{
			NeedDistroNotEquals(getAnotherDistro(distroName)),
			true,
			TestConstraint{
				Passed: []Result{
					{Success: true},
				},
			},
		},
		{
			NeedDistroNotEquals(distroName),
			false,
			TestConstraint{
				Failed: []Result{
					{Success: false},
				},
			},
		},

		{
			NeedDistro(distroName),
			true,
			TestConstraint{
				Passed: []Result{
					{Success: true},
				},
			},
		},
		{
			NeedDistro(getAnotherDistro(distroName)),
			false,
			TestConstraint{
				Failed: []Result{
					{Success: false},
				},
			},
		},
	}

	if root {
		td := testData{
			fn:    NeedRoot(),
			valid: true,
			expected: TestConstraint{
				Passed: []Result{
					{Success: true},
				},
			},
		}

		data = append(data, td)

		td = testData{
			fn:    NeedNonRoot(),
			valid: false,
			expected: TestConstraint{
				Failed: []Result{
					{Success: false},
				},
			},
		}

		data = append(data, td)
	} else {
		td := testData{
			fn:    NeedRoot(),
			valid: false,
			expected: TestConstraint{
				Failed: []Result{
					{Success: false},
				},
			},
		}

		data = append(data, td)

		td = testData{
			fn:    NeedNonRoot(),
			valid: true,
			expected: TestConstraint{
				Passed: []Result{
					{Success: true},
				},
			},
		}

		data = append(data, td)
	}

	for _, debug := range []bool{true, false} {
		for i, d := range data {
			tc := NewTestConstraint(debug)

			result := tc.constraintValid(d.fn)

			msg := fmt.Sprintf("test[%d]: %+v, result: %v", i, d, result)

			if d.expected.Issue != "" {
				assert.Equal(tc.Issue, d.expected.Issue, msg)
			}

			if d.valid {
				assert.True(result, msg)

				if len(d.expected.Passed) != 0 {
					assert.Equal(d.expected.Passed[0].Success, tc.Passed[0].Success, msg)
				}
			} else {
				assert.False(result, msg)

				if len(d.expected.Failed) != 0 {
					assert.Equal(d.expected.Failed[0].Success, tc.Failed[0].Success, msg)
				}
			}
		}
	}
}

func TestEvalIntVersion(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		currentVer    string
		op            Operator
		newVer        string
		expectSuccess bool
		expectError   bool
	}

	data := []testData{
		//----------

		{"", eqOperator, "", false, true},
		{"", eqOperator, "1", false, true},
		{"1", eqOperator, "", false, true},

		{"foo", eqOperator, "", false, true},
		{"", eqOperator, "foo", false, true},
		{"foo", eqOperator, "1", false, true},
		{"1", eqOperator, "foo", false, true},

		{"1", eqOperator, "1", true, false},
		{"1", eqOperator, "2", false, false},

		//----------

		{"", geOperator, "", false, true},
		{"foo", geOperator, "", false, true},
		{"", geOperator, "foo", false, true},
		{"1", geOperator, "", false, true},
		{"", geOperator, "1", false, true},

		{"1", geOperator, "2", false, false},
		{"2", geOperator, "1", true, false},
		{"2", geOperator, "2", true, false},

		//----------

		{"", gtOperator, "", false, true},
		{"foo", gtOperator, "", false, true},
		{"", gtOperator, "foo", false, true},
		{"1", gtOperator, "", false, true},
		{"", gtOperator, "1", false, true},

		{"2", gtOperator, "1", true, false},
		{"1", gtOperator, "2", false, false},
		{"1", gtOperator, "1", false, false},

		//----------

		{"", leOperator, "", false, true},
		{"foo", leOperator, "", false, true},
		{"", leOperator, "foo", false, true},
		{"1", leOperator, "", false, true},
		{"", leOperator, "1", false, true},

		{"2", leOperator, "1", false, false},
		{"1", leOperator, "2", true, false},
		{"1", leOperator, "1", true, false},

		//----------

		{"", ltOperator, "", false, true},
		{"foo", ltOperator, "", false, true},
		{"", ltOperator, "foo", false, true},
		{"1", ltOperator, "", false, true},
		{"", ltOperator, "1", false, true},

		{"1", ltOperator, "2", true, false},
		{"2", ltOperator, "1", false, false},
		{"1", ltOperator, "1", false, false},

		//----------

		{"", neOperator, "", false, true},
		{"foo", neOperator, "", false, true},
		{"", neOperator, "foo", false, true},
		{"1", neOperator, "", false, true},
		{"", neOperator, "1", false, true},

		{"2", neOperator, "2", false, false},
		{"1", neOperator, "2", true, false},
		{"2", neOperator, "1", true, false},
	}

	for i, d := range data {
		success, err := evalIntVersion(d.currentVer, d.op, d.newVer)

		msg := fmt.Sprintf("test[%d]: %+v, success: %v", i, d, success)

		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		if d.expectSuccess {
			assert.True(success, msg)
		} else {
			assert.False(success, msg)
		}
	}
}

func TestEvalFloatVersion(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		currentVer    string
		op            Operator
		newVer        string
		expectSuccess bool
		expectError   bool
	}

	data := []testData{
		//----------

		{"", eqOperator, "", false, true},
		{"foo", eqOperator, "", false, true},
		{"", eqOperator, "foo", false, true},
		{"foo", eqOperator, "1", false, true},
		{"1", eqOperator, "foo", false, true},

		{"1", eqOperator, "1", true, false},
		{"1", eqOperator, "2", false, false},

		{"1.1", eqOperator, "1.1", true, false},
		{"1.1", eqOperator, "2.1", false, false},

		//----------

		{"", geOperator, "", false, true},
		{"foo", geOperator, "", false, true},
		{"", geOperator, "foo", false, true},

		{"1", geOperator, "2", false, false},
		{"2", geOperator, "1", true, false},
		{"2", geOperator, "2", true, false},

		{"1.1", geOperator, "2.1", false, false},
		{"2.1", geOperator, "1.1", true, false},
		{"2.1", geOperator, "2.1", true, false},

		//----------

		{"", gtOperator, "", false, true},
		{"foo", gtOperator, "", false, true},
		{"", gtOperator, "foo", false, true},

		{"2", gtOperator, "1", true, false},
		{"1", gtOperator, "2", false, false},
		{"1", gtOperator, "1", false, false},

		{"2.1", gtOperator, "1.1", true, false},
		{"1.1", gtOperator, "2.1", false, false},
		{"1.1", gtOperator, "1.1", false, false},

		//----------

		{"", leOperator, "", false, true},
		{"foo", leOperator, "", false, true},
		{"", leOperator, "foo", false, true},

		{"2", leOperator, "1", false, false},
		{"1", leOperator, "2", true, false},
		{"1", leOperator, "1", true, false},

		{"2.1", leOperator, "1.1", false, false},
		{"1.1", leOperator, "2.1", true, false},
		{"1.1", leOperator, "1.1", true, false},

		//----------

		{"", ltOperator, "", false, true},
		{"foo", ltOperator, "", false, true},
		{"", ltOperator, "foo", false, true},

		{"1", ltOperator, "2", true, false},
		{"2", ltOperator, "1", false, false},
		{"1", ltOperator, "1", false, false},

		{"1.1", ltOperator, "2.1", true, false},
		{"2.1", ltOperator, "1.1", false, false},
		{"1.1", ltOperator, "1.1", false, false},

		//----------

		{"", neOperator, "", false, true},
		{"foo", neOperator, "", false, true},
		{"", neOperator, "foo", false, true},

		{"2", neOperator, "2", false, false},
		{"1", neOperator, "2", true, false},
		{"2", neOperator, "1", true, false},

		{"2.1", neOperator, "2.1", false, false},
		{"1.1", neOperator, "2.1", true, false},
		{"2.1", neOperator, "1.1", true, false},
	}

	for i, d := range data {
		success, err := evalFloatVersion(d.currentVer, d.op, d.newVer)

		msg := fmt.Sprintf("test[%d]: %+v, success: %v", i, d, success)

		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		if d.expectSuccess {
			assert.True(success, msg)
		} else {
			assert.False(success, msg)
		}
	}
}

func TestEvalSemverVersion(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		currentVer    string
		op            Operator
		newVer        string
		expectSuccess bool
		expectError   bool
	}

	data := []testData{
		//----------

		{"", eqOperator, "", false, true},
		{"foo", eqOperator, "", false, true},
		{"", eqOperator, "foo", false, true},
		{"foo", eqOperator, "1", false, true},
		{"1", eqOperator, "foo", false, true},

		{"1.1.1", eqOperator, "1.1.1", true, false},
		{"1.1.1", eqOperator, "2.2.2", false, false},

		//----------

		{"", geOperator, "", false, true},
		{"foo", geOperator, "", false, true},
		{"", geOperator, "foo", false, true},

		{"1.1.1", geOperator, "2.2.2", false, false},
		{"2.2.2", geOperator, "1.1.1", true, false},
		{"2.2.2", geOperator, "2.2.2", true, false},

		//----------

		{"", gtOperator, "", false, true},
		{"foo", gtOperator, "", false, true},
		{"", gtOperator, "foo", false, true},

		{"2.2.2", gtOperator, "1.1.1", true, false},
		{"1.1.1", gtOperator, "2.2.2", false, false},
		{"1.1.1", gtOperator, "1.1.1", false, false},

		//----------

		{"", leOperator, "", false, true},
		{"foo", leOperator, "", false, true},
		{"", leOperator, "foo", false, true},

		{"2.2.2", leOperator, "1.1.1", false, false},
		{"1.1.1", leOperator, "2.2.2", true, false},
		{"1.1.1", leOperator, "1.1.1", true, false},

		//----------

		{"", ltOperator, "", false, true},
		{"foo", ltOperator, "", false, true},
		{"", ltOperator, "foo", false, true},

		{"1.1.1", ltOperator, "2.2.2", true, false},
		{"2.2.2", ltOperator, "1.1.1", false, false},
		{"1.1.1", ltOperator, "1.1.1", false, false},

		//----------

		{"", neOperator, "", false, true},
		{"foo", neOperator, "", false, true},
		{"", neOperator, "foo", false, true},

		{"2.2.2", neOperator, "2.2.2", false, false},
		{"1.1.1", neOperator, "2.2.2", true, false},
		{"2.2.2", neOperator, "1.1.1", true, false},
	}

	for i, d := range data {
		success, err := evalSemverVersion(d.currentVer, d.op, d.newVer)

		msg := fmt.Sprintf("test[%d]: %+v, success: %v", i, d, success)

		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		if d.expectSuccess {
			assert.True(success, msg)
		} else {
			assert.False(success, msg)
		}
	}
}
