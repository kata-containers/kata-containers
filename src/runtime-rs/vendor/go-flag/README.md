A command-line parser with compatibility of Go's `flag` in its main focus.

## Design Goals

Go comes with a built-in support for command-line parsing: the `flag` library.
This is known to be incompatible with GNU convention, such as:

- Short/long flags. POSIX/GNU flags sometimes have a pair of short and long
  flags like `-f`/`--force` or `-n`/`--lines`. `flag` doesn't have such
  distinction.
- Combined short flags. In POSIX/GNU convention, `-fd` means `-f` plus `-d`.
  `flag` parses it as a single flag named `fd`.
- Flags after arguments. POSIX/GNU allows flags to appear after positional
  arguments like `./command arg1 --flag arg2` unless explicitly separated
  by `--`. `flag` parses it as a consecutive list of positional arguments.

The `go-flag` crate is designed to allow Rust programmers to easily port
Go CLI programs written using `flag` without breaking compatibility.

Therefore, our priority is the following:

1. **Behavioral compatibility**. It's meant to be compatible with the Go's
   built-in `flag` library in its command-line behavior.
   Note that API compatibility (similarity) is a different matter.
2. **Migration**. Being unable to use more sophisticated parsers like
   `structopt` is painful. Therefore, this library comes with an ability to
   check typical incompatible usages to allow gradual migration.
3. **Simplicity**. This library isn't meant to provide full parser
   functionality. For example, subcommand parsing is out of scope for
   this library. Try to migrate to e.g. `structopt` if you want to extend
   your program to accept more complex flags.

## Example

Typically you can use the `parse` function.

```rust
let mut force = false;
let mut lines = 10_i32;
let args: Vec<String> = go_flag::parse(|flags| {
    flags.add_flag("f", &mut force);
    flags.add_flag("lines", &mut lines);
});
```

If you want a list of file paths, use `PathBuf` or `OsString` to allow non-UTF8 strings.

```rust
use std::path::PathBuf;
let args: Vec<PathBuf> = go_flag::parse(|_| {});
```

If an incompatible usage is detected, `parse` issues warnings and continues processing.
You can alter the behavior using `parse_with_warnings`.

For example, when enough time passed since the first release of your Rust port,
you can start to deny the incompatible usages by specifying `WarningMode::Error`:

```rust
use go_flag::WarningMode;
let mut force = false;
let mut lines = 10_i32;
let args: Vec<String> =
    go_flag::parse_with_warnings(WarningMode::Error, |flags| {
        flags.add_flag("f", &mut force);
        flags.add_flag("lines", &mut lines);
    });
```
