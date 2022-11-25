// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// This file contains the public API for the test constraints facility.

package katatestutils

import (
	"os"
)

// Operator represents an operator to apply to a test constraint value.
type Operator int

const (
	eqOperator Operator = iota
	geOperator Operator = iota
	gtOperator Operator = iota
	leOperator Operator = iota
	ltOperator Operator = iota
	neOperator Operator = iota
)

// Constraints encapsulates all information about a test constraint.
type Constraints struct {
	Issue string

	// KernelVersion is the version of a particular kernel.
	KernelVersion string

	UID int

	// Operator is the operator to apply to one of the constraints.
	Operator Operator

	// Not ideal: set when UID needs to be checked. This allows
	// a test for UID 0 to be detected.
	UIDSet bool
}

// Constraint is a function that operates on a Constraints object to set
// particular values.
type Constraint func(c *Constraints)

// TestConstraint records details about test constraints.
type TestConstraint struct {
	KernelVersion string

	// Optionally used to record an issue number that relates to the
	// constraint.
	Issue string

	// Used to record all passed and failed constraints in
	// human-readable form.
	Passed []Result
	Failed []Result

	Debug bool

	// Effective user ID of running test
	ActualEUID int
}

// NewKataTest creates a new TestConstraint object and is the main interface
// to the test constraints feature.
func NewTestConstraint(debug bool) TestConstraint {
	kernelVersion, err := getKernelVersion()
	if err != nil {
		panic(err)
	}

	return TestConstraint{
		Debug: debug,

		ActualEUID:    os.Geteuid(),
		KernelVersion: kernelVersion,
	}
}

// NotValid checks if the specified list of constraints are all valid,
// returning true if any _fail_.
//
// Notes:
//
//   - Constraints are applied in the order specified.
//   - A constraint type (user, kernel) can only be specified once.
//   - If the function fails to determine whether it can check the constraints,
//     it will panic. Since this is facility is used for testing, this seems like
//     the best approach as it unburdens the caller from checking for an error
//     (which should never be ignored).
func (tc *TestConstraint) NotValid(constraints ...Constraint) bool {
	if len(constraints) == 0 {
		panic("need atleast one constraint")
	}

	// Reset in case of a previous call
	tc.Passed = nil
	tc.Failed = nil
	tc.Issue = ""

	for _, c := range constraints {
		valid := tc.constraintValid(c)
		if !valid {
			return true
		}
	}

	return false
}

// NeedUID skips the test unless running as a user with the specified user ID.
func NeedUID(uid int, op Operator) Constraint {
	return func(c *Constraints) {
		c.Operator = op
		c.UID = uid
		c.UIDSet = true
	}
}

// NeedNonRoot skips the test unless running as root.
func NeedRoot() Constraint {
	return NeedUID(0, eqOperator)
}

// NeedNonRoot skips the test if running as the root user.
func NeedNonRoot() Constraint {
	return NeedUID(0, neOperator)
}

// NeedKernelVersionWithOp skips the test unless the kernel version constraint
// specified by the arguments is true.
func NeedKernelVersionWithOp(version string, op Operator) Constraint {
	return func(c *Constraints) {
		c.KernelVersion = version
		c.Operator = op
	}
}

// NeedKernelVersionEquals will skip the test unless the kernel version is same as
// the specified version.
func NeedKernelVersionEquals(version string) Constraint {
	return NeedKernelVersionWithOp(version, eqOperator)
}

// NeedKernelVersionNotEquals will skip the test unless the kernel version is
// different to the specified version.
func NeedKernelVersionNotEquals(version string) Constraint {
	return NeedKernelVersionWithOp(version, neOperator)
}

// NeedKernelVersionLT will skip the test unless the kernel version is older
// than the specified version.
func NeedKernelVersionLT(version string) Constraint {
	return NeedKernelVersionWithOp(version, ltOperator)
}

// NeedKernelVersionLE will skip the test unless the kernel version is older
// than or the same as the specified version.
func NeedKernelVersionLE(version string) Constraint {
	return NeedKernelVersionWithOp(version, leOperator)
}

// NeedKernelVersionGT will skip the test unless the kernel version is newer
// than the specified version.
func NeedKernelVersionGT(version string) Constraint {
	return NeedKernelVersionWithOp(version, gtOperator)
}

// NeedKernelVersionGE will skip the test unless the kernel version is newer
// than or the same as the specified version.
func NeedKernelVersionGE(version string) Constraint {
	return NeedKernelVersionWithOp(version, geOperator)
}

// NeedKernelVersion will skip the test unless the kernel version is same as
// the specified version.
func NeedKernelVersion(version string) Constraint {
	return NeedKernelVersionEquals(version)
}

// WithIssue allows the specification of an issue URL.
//
// Note that the issue is not checked for validity.
func WithIssue(issue string) Constraint {
	return func(c *Constraints) {
		c.Issue = issue
	}
}
