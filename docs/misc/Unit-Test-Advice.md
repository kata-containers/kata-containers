# Unit Test Advice

## Overview

This document offers advice on writing a Unit Test (UT) in
[Golang](https://golang.org) and [Rust](https://www.rust-lang.org).

## General advice

### Unit test strategies

#### Positive and negative tests

Always add positive tests (where success is expected) *and* negative
tests (where failure is expected).

#### Boundary condition tests

Try to add unit tests that exercise boundary conditions such as:

- Missing values (`null` or `None`).
- Empty strings and huge strings.
- Empty (or uninitialised) complex data structures
  (such as lists, vectors and hash tables).
- Common numeric values (such as `-1`, `0`, `1` and the minimum and
  maximum values).

#### Test unusual values

Also always consider "unusual" input values such as:

- String values containing spaces, Unicode characters, special
  characters, escaped characters or null bytes.

  > **Note:** Consider these unusual values in prefix, infix and
  > suffix position.

- String values that cannot be converted into numeric values or which
  contain invalid structured data (such as invalid JSON).

#### Other types of tests

If the code requires other forms of testing (such as stress testing,
fuzz testing and integration testing), raise a GitHub issue and
reference it on the issue you are using for the main work. This
ensures the test team are aware that a new test is required.

### Test environment

#### Create unique files and directories

Ensure your tests do not write to a fixed file or directory. This can
cause problems when running multiple tests simultaneously and also
when running tests after a previous test run failure.

#### Assume parallel testing

Always assume your tests will be run *in parallel*. If this is
problematic for a test, force it to run in isolation using the
`serial_test` crate for Rust code for example.

### Running

Ensure you run the unit tests and they all pass before raising a PR.
Ideally do this on different distributions on different architectures
to maximise coverage (and so minimise surprises when your code runs in
the CI).

## Assertions

### Golang assertions

Use the `testify` assertions package to create a new assertion object as this
keeps the test code free from distracting `if` tests:

```go
func TestSomething(t *testing.T) {
    assert := assert.New(t)

    err := doSomething()
    assert.NoError(err)
}
```

### Rust assertions

Use the standard set of `assert!()` macros.

## Table driven tests

Try to write tests using a table-based approach. This allows you to distill
the logic into a compact table (rather than spreading the tests across
multiple test functions). It also makes it easy to cover all the
interesting boundary conditions:

### Golang table driven tests

Assume the following function:

```go
// The function under test.
//
// Accepts a string and an integer and returns the
// result of sticking them together separated by a dash as a string.
func joinParamsWithDash(str string, num int) (string, error) {
    if str == "" {
        return "", errors.New("string cannot be blank")
    }

    if num <= 0 {
        return "", errors.New("number must be positive")
    }

    return fmt.Sprintf("%s-%d", str, num), nil
}
```

A table driven approach to testing it:

```go
import (
    "testing"
    "github.com/stretchr/testify/assert"
)

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
        // Failure scenarios
        {"", -1, "", true},
        {"", 0, "", true},
        {"", 1, "", true},
        {"foo", 0, "", true},
        {"foo", -1, "", true},

        // Success scenarios
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

        // update the message for more information on failure
        msg = fmt.Sprintf("%s, result: %q, err: %v", msg, result, err)

        if d.expectError {
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

### Rust table driven tests

Assume the following function:

```rust
// Convenience type to allow Result return types to only specify the type
// for the true case; failures are specified as static strings.
// XXX: This is an example. In real code use the "anyhow" and
// XXX: "thiserror" crates.
pub type Result<T> = std::result::Result<T, &'static str>;

// The function under test.
//
// Accepts a string and an integer and returns the
// result of sticking them together separated by a dash as a string.
fn join_params_with_dash(str: &str, num: i32) -> Result<String> {
    if str.is_empty() {
        return Err("string cannot be blank");
    }

    if num <= 0 {
        return Err("number must be positive");
    }

    let result = format!("{}-{}", str, num);

    Ok(result)
}

```

A table driven approach to testing it:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_params_with_dash() {
        // This is a type used to record all details of the inputs
        // and outputs of the function under test.
        #[derive(Debug)]
        struct TestData<'a> {
            str: &'a str,
            num: i32,
            result: Result<String>,
        }

        // The tests can now be specified as a set of inputs and outputs
        let tests = &[
            // Failure scenarios
            TestData {
                str: "",
                num: 0,
                result: Err("string cannot be blank"),
            },
            TestData {
                str: "foo",
                num: -1,
                result: Err("number must be positive"),
            },

            // Success scenarios
            TestData {
                str: "foo",
                num: 42,
                result: Ok("foo-42".to_string()),
            },
            TestData {
                str: "-",
                num: 1,
                result: Ok("--1".to_string()),
            },
        ];

        // Run the tests
        for (i, d) in tests.iter().enumerate() {
            // Create a string containing details of the test
            let msg = format!("test[{}]: {:?}", i, d);

            // Call the function under test
            let result = join_params_with_dash(d.str, d.num);

            // Update the test details string with the results of the call
            let msg = format!("{}, result: {:?}", msg, result);

            // Perform the checks
            if d.result.is_ok() {
                assert!(result == d.result, msg);
                continue;
            }

            let expected_error = format!("{}", d.result.as_ref().unwrap_err());
            let actual_error = format!("{}", result.unwrap_err());
            assert!(actual_error == expected_error, msg);
        }
    }
}
```

## Temporary files

Use `t.TempDir()` to create temporary directory. The directory created by
`t.TempDir()` is automatically removed when the test and all its subtests
complete.

### Golang temporary files

```go
func TestSomething(t *testing.T) {
    assert := assert.New(t)

    // Create a temporary directory
    tmpdir := t.TempDir()

    // Add test logic that will use the tmpdir here...
}
```

### Rust temporary files

Use the `tempfile` crate which allows files and directories to be deleted
automatically:

```rust
#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    #[test]
    fn test_something() {

        // Create a temporary directory (which will be deleted automatically
        let dir = tempdir().expect("failed to create tmpdir");

        let filename = dir.path().join("file.txt");

        // create filename ...
    }
}

```

## Test user

[Unit tests are run *twice*](../src/runtime/go-test.sh):

- as the current user
- as the `root` user (if different to the current user)

When writing a test consider which user should run it; even if the code the
test is exercising runs as `root`, it may be necessary to *only* run the test
as a non-`root` for the test to be meaningful. Add appropriate skip
guards around code that requires `root` and non-`root` so that the test
will run if the correct type of user is detected and skipped if not.

### Run Golang tests as a different user

The main repository has the most comprehensive set of skip abilities. See:

- [`katatestutils`](../src/runtime/pkg/katatestutils)

### Run Rust tests as a different user

One method is to use the `nix` crate along with some custom macros:

```rust
#[cfg(test)]
mod tests {
    #[allow(unused_macros)]
    macro_rules! skip_if_root {
        () => {
            if nix::unistd::Uid::effective().is_root() {
                println!("INFO: skipping {} which needs non-root", module_path!());
                return;
            }
        };
    }

    #[allow(unused_macros)]
    macro_rules! skip_if_not_root {
        () => {
            if !nix::unistd::Uid::effective().is_root() {
                println!("INFO: skipping {} which needs root", module_path!());
                return;
            }
        };
    }

    #[test]
    fn test_that_must_be_run_as_root() {
        // Not running as the superuser, so skip.
        skip_if_not_root!();

        // Run test *iff* the user running the test is root

        // ...
    }
}
```
