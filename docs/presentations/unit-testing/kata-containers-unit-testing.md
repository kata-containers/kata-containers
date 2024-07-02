## Why write unit tests?

- Catch regressions

- Improve the code being tested

  Structure, quality, security, performance, "shakes out" implicit
  assumptions, _etc_

- Extremely instructive

  Once you've fully tested a single function, you'll understand that
  code very well indeed.

## Why write unit tests? (continued)

- Fun!

  Yes, really! Don't believe me? Try it! ;)

## Run all Kata Containers agent unit tests

As an example, to run all agent unit tests:

```bash
$ cd $GOPATH/src/github.com/kata-containers/kata-containers
$ cd src/agent
$ make test
```

## List all unit tests

- Identify the full name of all the tests _in the current package_:

  ```bash
  $ cargo test -- --list
  ```

- Identify the full name of all tests in the `foo` "local crate"
  (sub-directory containing another `Cargo.toml` file):

  ```bash
  $ cargo test -p "foo" -- --list
  ```

## Run a single unit test

- Run a test in the current package in verbose mode:

  ```bash
  # Example 
  $ test="config::tests::test_get_log_level"

  $ cargo test "$test" -vv -- --exact --nocapture
  ```

## Test coverage setup

```bash
$ cargo install cargo-tarpaulin
```

## Show test coverage

```bash
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/src/agent
$ cargo -v tarpaulin --all-features --run-types AllTargets --count --force-clean -o Html
$ xdg-open "file://$PWD/tarpaulin-report.html"
```

## Testability (part 1)

- To be testable, a function should:
  - Not be "too long" (say >100 lines).
  - Not be "too complex" (say >3 levels of indentation).
  - Should return a `Result` or an `Option` so error paths
    can be tested.

- If functions don't conform, they need to be reworked (refactored)
  before writing tests.

## Testability (part 2)

- Some functions can't be fully tested.
- However, you _can_ test the initial code that checks
  the parameter values (test error paths only).

## Writing new tests: General advice (part 1)

- KISS: Keep It Simple Stupid

  You don't get extra points for cryptic code.

- DRY: Don't Repeat Yourself

  Make use of existing facilities (don't "re-invert the wheel").

- Read the [unit test advice document](https://github.com/kata-containers/kata-containers/blob/main/docs/Unit-Test-Advice.md)

## Writing new tests: General advice (part 2)

- Attack the function in all possible ways

- Use the _table driven_ approach:
  - Simple
  - Compact
  - Easy to debug
  - Makes boundary analysis easy
  - Encourages functions to be testable

## Writing new tests: Specific advice (part 1)

- Create a new "`tests`" module if necessary.
- Give each test function a "`test_`" prefix.
- Add the "`#[test]`" annotation on each test function.

## Writing new tests: Specific advice (part 2)

- If you need to `use` (import) packages for the tests,
  _only do it in the `tests` module_:
  ```rust
  use some_test_pkg::{foo, bar}; // <-- Not here

  #[cfg(test)]
  mod tests {
    use super::*;
    use some_test_pkg:{foo, bar}; // <-- Put it here
  }
  ```

## Writing new tests: Specific advice (part 3)

- You can add test-specific dependencies in `Cargo.toml`:
  ```toml
  [dev-dependencies]
  serial_test = "0.5.1"
  ```

## Writing new tests: Specific advice (part 4)

- Don't add in lots of error handling code: let the test panic!
  ```rust
  // This will panic if the unwrap fails.
  // - NOT acceptable generally for production code.
  // - PERFECTLY acceptable for test code since:
  //   - Keeps the test code simple.
  //   - Rust will detect the panic and fail the test.
  let result = func().unwrap();
  ```

## Debugging tests (part 1)

- Comment out all tests in your `TestData` array apart from the failing test.

- Add temporary `println!("FIXME: ...")` statements in the code.

- Set `RUST_BACKTRACE=full` before running `cargo test`.

## Debugging tests (part 2)

- Use a debugger (not normally necessary though):
  ```bash
  # Disable optimisation
  $ RUSTFLAGS="-C opt-level=0" cargo test --no-run

  # Find the test binary
  $ test_binary=$(find target/debug/deps | grep "kata_agent-[a-z0-9][a-z0-9]*$" | tail -1)

  $ rust-gdb "$test_binary"
  ```

## Useful tips

- Always start a test with a "clean environment":

  Create new set of objects / files / directories / _etc_
  for each test.

- Mounts
  - Linux allows mounts on top of existing mounts.
  - Bind mounts and read-only mounts can be useful.

## Gotchas (part 1)

If a test runs successfully _most of the time_:

- Review the test logic.

- Add a `#[serial]` annotation on the test function
  Requires the `serial_test` package in the `[dev-dependencies]`
  section of `Cargo.toml`.

  If this makes it work the test is probably sharing resources with
  another task (thread).

## Gotchas (part 2)

If a test works locally but fails in the CI, consider the following
attributes of each environment (local and CI):

- The version of rust being used.
- The hardware architecture.
- Number (and spec) of the CPUs.

## Before raising a PR

- Remember to check that the test runs locally:
  - As a non-privileged user.
  - As the `root` user (carefully!)

- Run the [static checker](https://github.com/kata-containers/kata-containers/blob/main/tests/static-checks.sh)
  on your changes.

  Checks formatting and many other things.

## If in doubt

- Ask for help! ;)

## Quiz 1

What's wrong with this function?

```rust
fn foo(config: &Config, path_prefix: String, container_id: String, pid: String) -> Result<()> {
    let mut full_path = format!("{}/{}", path_prefix, container_id);

    let _ = remove_recursively(&mut full_path);

    write_number_to_file(pid, full_path);

    Ok(())
}
```

## Quiz 1: Answers (part 1)

- No check that `path_prefix`, `container_id` and `pid` are not `""`.
- No check that `path_prefix` is absolute.
- No check that `container_id` does not contain slashes / contains only valid characters.
- Result of `remove_recursively()` discarded.
- `remove_recursively()` _may_ modify `full_path` without `foo()` knowing!

## Quiz 1: Answers (part 2)

- Why is `pid` not a numeric?
- No check to ensure the PID is positive.
- No check to recreate any directories in the original `path_prefix`.
- `write_number_to_file()` could fail so why doesn't it return a value?
- The `config` parameter is unused.

## Quiz 1: What if...

Imagine if the caller managed to do this:

```rust
foo(config, "", "sbin/init", r#"#!/bin/sh\n/sbin/reboot"#);
```

## Quiz 2

What makes this function difficult to test?

```rust
fn get_user_id(username: String) -> i32 {
    let line = grep_file(username, "/etc/passwd").unwrap();
    let fields = line.split(':');

    let uid = fields.nth(2).ok_or("failed").unwrap();

    uid.parse::<i32>()
}
```

## Quiz 2: Answers (part 1)

- Unhelpful error message ("failed").

- Panics on error! Return a `Result` instead!

- UID's cannot be negative so function should return an unsigned
  value.

## Quiz 2: Answers (part 2)

- Hard-coded filename.

  This would be better:

  ```rust
  const PASSWD_DB: &str = "/etc/passwd";

  // Test code can now pass valid and invalid files!
  fn get_user_id(filename: String, username: String) -> i32 {
    // ...
  }

  let id = get_user_id(PASSWD_DB, username);
  ```

## Quiz 3

What's wrong with this test code?

```rust
let mut obj = Object::new();

// Sanity check
assert_eq!(obj.num, 0);
assert_eq!(obj.wibble, false);

// Test 1
obj->foo_method(7);
assert_eq!(obj.num, 7);

// Test 2
obj->bar_method(true);
assert_eq!(obj.wibble, true);
```

## Quiz 3: Answers

- The test code is "fragile":
  - The 2nd test re-uses the object created in the first test.

## Finally

- [We need a GH action to run the unit tests](https://github.com/kata-containers/kata-containers/issues/2934)

  Needs to fail PRs that decrease test coverage<br/> by "x%".
