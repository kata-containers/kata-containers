// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package katatestutils

import (
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/blang/semver"
)

const (
	TestDisabledNeedRoot    = "Test disabled as requires root user"
	TestDisabledNeedNonRoot = "Test disabled as requires non-root user"
)

var errInvalidOpForConstraint = errors.New("invalid operator for constraint type")

// String converts the operator to a human-readable value.
func (o Operator) String() (s string) {
	switch o {
	case eqOperator:
		s = "=="
	case geOperator:
		s = ">="
	case gtOperator:
		s = ">"
	case leOperator:
		s = "<="
	case ltOperator:
		s = "<"
	case neOperator:
		s = "!="
	}

	return s
}

// Result is the outcome of a Constraint test
type Result struct {
	// Details of the constraint
	// (human-readable result of testing for a Constraint).
	Description string

	// true if constraint was valid
	Success bool
}

// GetFileContents return the file contents as a string.
func getFileContents(file string) (string, error) {
	bytes, err := os.ReadFile(file)
	if err != nil {
		return "", err
	}

	return string(bytes), nil
}

func getKernelVersion() (string, error) {
	const procVersion = "/proc/version"

	contents, err := getFileContents(procVersion)
	if err != nil {
		return "", err
	}

	fields := strings.Fields(contents)
	l := len(fields)
	if l < 3 {
		return "", fmt.Errorf("unexpected contents in %v", procVersion)
	}

	return fixKernelVersion(fields[2]), nil
}

// fixKernelVersion replaces underscores with dashes in a version string.
// This change is primarily for Fedora, RHEL and CentOS version numbers which
// can contain underscores. By replacing them with dashes, a valid semantic
// version string is created.
//
// Examples of actual kernel versions which can be made into valid semver
// format by calling this function:
//
//	centos: 3.10.0-957.12.1.el7.x86_64
//	fedora: 5.0.9-200.fc29.x86_64
//
// For some self compiled kernel, the kernel version will be with "+" as its suffix
// For example:
//
//	5.12.0-rc4+
//
// These kernel version can't be parsed by the current lib and lead to panic
// therefore the '+' should be removed.
func fixKernelVersion(version string) string {
	version = strings.Replace(version, "_", "-", -1)
	return strings.Replace(version, "+", "", -1)
}

// handleKernelVersion checks that the current kernel version is compatible with
// the constraint specified by the arguments.
func (tc *TestConstraint) handleKernelVersion(version string, op Operator) (result Result, err error) {
	return handleVersionType("kernel", tc.KernelVersion, op, version)
}

// handleVersionType checks that the current and new versions are compatible with
// the constraint specified by the arguments. The versionName argument is a
// human-readable value to represent the currentVersion.
func handleVersionType(versionName, newVersion string, op Operator, currentVersion string) (result Result, err error) {
	if versionName == "" {
		return Result{}, fmt.Errorf("version name cannot be blank")
	}

	if newVersion == "" {
		return Result{}, fmt.Errorf("new version cannot be blank")
	}

	if currentVersion == "" {
		return Result{}, fmt.Errorf("current version cannot be blank")
	}

	newVersion = strings.ToLower(newVersion)
	currentVersion = strings.ToLower(currentVersion)

	newVersionElements := len(strings.Split(newVersion, "."))
	currentVersionElements := len(strings.Split(currentVersion, "."))

	var success bool

	// Determine the type of version string based on the current version
	switch currentVersionElements {
	case 1:
		// A simple integer version number.
		if newVersionElements != 1 {
			return Result{}, fmt.Errorf("%s version type (%q) is integer, but specified version (%s) is not",
				versionName, currentVersion, newVersion)
		}

		success, err = evalIntVersion(newVersion, op, currentVersion)
	case 2:
		// A "floating point" version number in format "a.b".
		if newVersionElements > 2 {
			return Result{}, fmt.Errorf("%s version type (%q) is float, but specified version (%s) is not float or int",
				versionName, currentVersion, newVersion)
		}

		success, err = evalFloatVersion(newVersion, op, currentVersion)
	default:
		// Assumed to be a semver format version string
		// in format "a.b.c."
		//
		// Cannot check specified version here as semver is more
		// complex - let the eval function detail with it.

		success, err = evalSemverVersion(newVersion, op, currentVersion)
	}

	if err != nil {
		return Result{}, err
	}

	descr := fmt.Sprintf("need %s version %s %q, got version %q",
		versionName, op, currentVersion, newVersion)

	result = Result{
		Description: descr,
		Success:     success,
	}

	return result, nil
}

// evalIntVersion deals with integer version numbers
// (in format "a").
func evalIntVersion(newVersionStr string, op Operator, currentVersionStr string) (success bool, err error) {
	newVersion, err := strconv.Atoi(newVersionStr)
	if err != nil {
		return false, err
	}

	currentVersion, err := strconv.Atoi(currentVersionStr)
	if err != nil {
		return false, err
	}

	switch op {
	case eqOperator:
		success = newVersion == currentVersion
	case geOperator:
		success = newVersion >= currentVersion
	case gtOperator:
		success = newVersion > currentVersion
	case leOperator:
		success = newVersion <= currentVersion
	case ltOperator:
		success = newVersion < currentVersion
	case neOperator:
		success = newVersion != currentVersion
	default:
		return false, errInvalidOpForConstraint
	}

	return success, err
}

// evalFloatVersion deals with "floating point" version numbers
// (in format "a.b").
//
// Note that (implicitly) the specified version number provided by the user
// may in fact be an integer value which will be converted into a float.
func evalFloatVersion(newVersionStr string, op Operator, currentVersionStr string) (success bool, err error) {
	// If this many bits is insufficient to represent a version number, we
	// have problems...!
	const bitSize = 32

	newVersion, err := strconv.ParseFloat(newVersionStr, bitSize)
	if err != nil {
		return false, err
	}

	currentVersion, err := strconv.ParseFloat(currentVersionStr, bitSize)
	if err != nil {
		return false, err
	}

	switch op {
	case eqOperator:
		success = newVersion == currentVersion
	case geOperator:
		success = newVersion >= currentVersion
	case gtOperator:
		success = newVersion > currentVersion
	case leOperator:
		success = newVersion <= currentVersion
	case ltOperator:
		success = newVersion < currentVersion
	case neOperator:
		success = newVersion != currentVersion
	default:
		return false, errInvalidOpForConstraint
	}

	return success, err
}

// evalSemverVersion deals with semantic versioning format version strings
// (in version "a.b.c").
//
// See: https://semver.org
func evalSemverVersion(newVersionStr string, op Operator, currentVersionStr string) (success bool, err error) {
	newVersion, err := semver.Make(newVersionStr)
	if err != nil {
		return false, err
	}

	currentVersion, err := semver.Make(currentVersionStr)
	if err != nil {
		return false, err
	}

	switch op {
	case eqOperator:
		success = newVersion.EQ(currentVersion)
	case geOperator:
		success = newVersion.GE(currentVersion)
	case gtOperator:
		success = newVersion.GT(currentVersion)
	case leOperator:
		success = newVersion.LE(currentVersion)
	case ltOperator:
		success = newVersion.LT(currentVersion)
	case neOperator:
		success = newVersion.NE(currentVersion)
	default:
		return false, errInvalidOpForConstraint
	}

	return success, err
}

// handleUID checks that the current UID is compatible with the constraint
// specified by the arguments.
func (tc *TestConstraint) handleUID(uid int, op Operator) (result Result, err error) {
	if uid < 0 {
		return Result{}, fmt.Errorf("uid must be >= 0, got %d", uid)
	}

	var success bool

	switch op {
	case eqOperator:
		success = tc.ActualEUID == uid
	case neOperator:
		success = tc.ActualEUID != uid
	default:
		return Result{}, errInvalidOpForConstraint
	}

	descr := fmt.Sprintf("need uid %s %d, got euid %d", op, uid, tc.ActualEUID)

	result = Result{
		Description: descr,
		Success:     success,
	}

	return result, nil
}

// handleResults is the common handler for all constraint types. It deals with
// errors found trying to check constraints, stores results and displays
// details of valid constraints.
func (tc *TestConstraint) handleResults(result Result, err error) {
	if err != nil {
		var extra string

		if tc.Issue != "" {
			extra = fmt.Sprintf(" (issue %s)", tc.Issue)
		}

		// Display the TestConstraint object as it's may provide
		// helpful information for the caller.
		panic(fmt.Sprintf("%+v: failed to check test constraints: error: %s%s\n",
			tc, err, extra))
	}

	if !result.Success {
		tc.Failed = append(tc.Failed, result)
	} else {
		tc.Passed = append(tc.Passed, result)
	}

	if tc.Debug {
		var outcome string

		if result.Success {
			outcome = "valid"
		} else {
			outcome = "invalid"
		}

		fmt.Printf("Constraint %s: %s\n", outcome, result.Description)
	}
}

// constraintValid handles the specified constraint, returning true if the
// constraint is valid, else false.
func (tc *TestConstraint) constraintValid(fn Constraint) bool {
	c := Constraints{}

	// Call the constraint function that sets the Constraints values
	fn(&c)

	if c.Issue != "" {
		// Just record it
		tc.Issue = c.Issue
	}

	if c.UIDSet {
		result, err := tc.handleUID(c.UID, c.Operator)
		tc.handleResults(result, err)
		if !result.Success {
			return false
		}
	}

	if c.KernelVersion != "" {
		result, err := tc.handleKernelVersion(c.KernelVersion, c.Operator)
		tc.handleResults(result, err)
		if !result.Success {
			return false
		}

	}

	// Constraint is valid
	return true
}
