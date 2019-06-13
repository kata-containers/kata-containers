# Kata test utilities

* [Test Constraints](#test-constraints)
    * [Usage](#usage)
        * [Displaying the `TestConstraint`](#displaying-the-testconstraint)
        * [Associating an issue with a constraint](#associating-an-issue-with-a-constraint)
    * [Examples](#examples)
        * [Skip tests based on user](#skip-tests-based-on-user)
        * [Skip tests based on distro](#skip-tests-based-on-distro)
        * [Skip tests based on kernel version](#skip-tests-based-on-kernel-version)
    * [Full details](#full-details)

This package provides a small set of test utilities. See the
[GoDoc](https://godoc.org/github.com/kata-containers/runtime/pkg/katatestutils)
for full details.

## Test Constraints

This package provides helper functions that accept user-specified constraints
that allow you to skip tests.

### Usage

Create a `TestConstraint` object using the `NewTestConstraint()` constructor.
This takes a single boolean parameter that specifies if debug output is generated.

In each test that has particular test constraints, call the `NotValid()`
method on the `TestConstraint` object, passing one or more constraints that
you want to be valid.

The `NotValid()` function returns `true` if any of the specified constraints
are not available. This allows for a more natural way to code an arbitrarily
complex test skip as shown in the following example.

The main object is created in the `init()` function to make it available for
all tests:

```go

import ktu "katatestutils"

var tc ktu.TestConstraint

func init() {
    tc = NewTestConstraint(true)
}

func TestFoo(t *testing.T) {

    // Specify one or more constraint functions. If not satisfied, the test
    // will be skipped.
    if tc.NotValid(...) {
        t.Skip("skipping test")
    }

    // Test code ...
}
```

#### Displaying the `TestConstraint`

Note that you could add the `TestConstraint` object to the `Skip()` call as it
will provide details of why the skip occurred:

```go
if tc.NotValid(...) {
    t.Skipf("skipping test as requirements not met: %v", tc)
}
```

#### Associating an issue with a constraint

You can add a constraint which specifies an issue URL for the skip. No
checking is performed on the issue but if specified, it will be added to the
`TestConstraint` and recorded in error messages and when that object is
displayed:

```go
if tc.NotValid(WithIssue("https://github.com/kata-containers/runtime/issues/1586"), ...) {
    t.Skipf("skipping test as requirements not met: %v", tc)
}
```

### Examples

#### Skip tests based on user

Use the `NeedRoot()` constraint to skip a test unless running as `root`:

```go
func TestOnlyRunWhenRoot(t *testing.T) {

    if tc.NotValid(ktu.NeedRoot()) {
        t.Skip("skipping test as not running as root user")
    }

    // Test code to run as root user ...
}
```

Use the `NeedNonRoot()` constraint to skip a test unless running as a
non-`root` user:

```go
func TestOnlyRunWhenNotRoot(t *testing.T) {

    if tc.NotValid(ktu.NeedNonRoot()) {
        t.Skip("skipping test as running as root user")
    }

    // Test code to run as non-root user ...
}
```

#### Skip tests based on distro

Use the `NeedDistro()` constraint to skip a test unless running on a
particular Linux distribution:

```go
func TestOnlyRunOnUbuntu(t *testing.T) {

    if tc.NotValid(ktu.NeedDistro("ubuntu")) {
        t.Skip("skipping test as not running on ubuntu")
    }

    // Test code to run on Ubuntu only ...
}
```

Use the `NeedDistroNotEquals()` constraint to skip a test unless running
on a Linux distribution other than the one specified:

```go
func TestDontRunOnFedora(t *testing.T) {

    if tc.NotValid(ktu.NeedDistroNotEquals("fedora")) {
        t.Skip("skipping test as running on fedora")
    }

    // Test code to run on any distro apart from Fedora ...
}
```

#### Skip tests based on kernel version

Use the `NeedKernelVersionGE()` constraint to skip a test unless running on a
system with at least the specified kernel version:

```go
func TestNewKernelVersion(t *testing.T) {

    if tc.NotValid(ktu.NeedKernelVersionGE("5.0.10")) {
        t.Skip("skipping test as kernel is too old")
    }

    // Test code to run on specified kernel version (or newer) ...
}
```

Use the `NeedKernelVersionLT()` constraint to skip a test unless running on a
system whose kernel is older than the specified kernel version:

```go
func TestOldKernelVersion(t *testing.T) {

    if tc.NotValid(ktu.NeedKernelVersionLT("4.14.114")) {
        t.Skip("skipping test as kernel is too new")
    }

    // Test code to run on specified kernel version (or newer) ...
}
```

### Full details

The public API is shown in [`constraints_api.go`](constraints_api.go) or
the [GoDoc](https://godoc.org/github.com/kata-containers/runtime/pkg/katatestutils).
