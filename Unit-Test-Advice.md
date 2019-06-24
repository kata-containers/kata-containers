# Unit Test Advice

* [Overview](#overview)
* [Assertions](#assertions)
* [Table driven tests](#table-driven-tests)
* [Temporary files](#temporary-files)
* [User running the test](#user-running-the-test)

## Overview

This document offers advice on writing a new `golang` unit test (UT).

## Assertions

Use the `testify` assertions package to create a new assertion object as this
keeps the test code free from distracting `if` tests:

```go
func TestSomething(t *testing.T) {
    assert := assert.New(t)

    err := doSomething()
    assert.NoError(err)
}
```

## Table driven tests

Try to write tests using a table-based approach. This allows you to distill
the logic into a compact table (rather than spreading the tests across
multiple `Test*` functions). It also makes it easy to cover all the
interesting boundary conditions:

```go
func TestJoinParamsWithDash(t *testing.T) {
    assert := assert.New(t)

    // Type used to hold function parameters and expected results.
    type testData struct {
        param1         string
        param2         int
        expectedResult string
        expectError    bool
    }

    // List of tests to run including the expected results
    data := []testData{
        // failure scenarios
        {"", -1, "", true},
        {"",  0, "", true},
        {"",  1, "", true},
        {"foo", -1, "", true},

        // success scenarios
        {"foo", 1, "foo-1", false},
        {"bar", 42, "bar-42", false},
    }

    // Run the tests
    for i, d := range data {
        // Create a test-specific string that is added to each assert
        // call. It will be displayed if any assert test fails.
        msg := fmt.Sprintf("test[%d]: %+v", i, d)

        // Call the function under test
        result, err := joinParamsWithDash(d.param1, d.param2)

        if expectError {
            assert.Error(err, msg)

            // If an error is expected, there is no point
            // performing additional checks.
            continue
        }

        assert.NoError(err, msg)
        assert.Equal(d.expectedResult, result, msg)
    }
}
```

## Temporary files

Always delete temporary files on success:

```go
func TestSomething(t *testing.T) {
    assert := assert.New(t)

    // Create a temporary directory
    tmpdir, err := ioutil.TempDir("", "")    
    assert.NoError(err)             

    // Delete it at the end of the test
    defer os.RemoveAll(tmpdir) 

    // Add test logic that will use the tmpdir here...
}
```

## User running the test

[Unit tests are run *twice*](https://github.com/kata-containers/tests/blob/master/.ci/go-test.sh):

- as the current user
- as the `root` user (if different to the current user)

When writing a test consider which user should run it; even if the code the
test is exercising runs as `root`, it may be necessary to *only* run the test
as a non-`root` for the test to be meaningful.

Some repositories already provide utility functions to skip a test:

- if running as `root`
- if not running as `root`

The runtime repository has the most comprehensive set of skip abilities. See:

- https://github.com/kata-containers/runtime/tree/master/pkg/katatestutils
