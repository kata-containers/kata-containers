# subprocess

[![](http://meritbadge.herokuapp.com/subprocess)](https://crates.io/crates/subprocess)
[![Build Status](https://travis-ci.org/hniksic/rust-subprocess.svg?branch=master)](https://travis-ci.org/hniksic/rust-subprocess)
[![docs.rs](https://docs.rs/subprocess/badge.svg)](https://docs.rs/subprocess)

The `subprocess` library provides facilities for execution of and
interaction with external processes and pipelines, inspired by
Python's `subprocess` module.  `subprocess` is [hosted on
crates.io](https://crates.io/crates/subprocess), with [API
Documentation on docs.rs](https://docs.rs/subprocess/).

## Features

This library is about launching external processes with optional redirection
of standard input, output, and error.  It covers similar ground as the
[`std::process`](https://doc.rust-lang.org/std/process/index.html) standard
library module, but with additional functionality:

* The *communicate* [family of
  methods](https://docs.rs/subprocess/latest/subprocess/struct.Popen.html#method.communicate_start)
  for deadlock-free capturing of subprocess output/error to memory, while
  simultaneously feeding data to its standard input.  Capturing supports
  optional timeout and read size limit.

* Connecting multiple commands into OS-level pipelines.

* Flexible redirection options, such as connecting standard streams to
  arbitrary files, or merging output streams like shell's `2>&1` and `1>&2`
  operators.

* Non-blocking and timeout methods to wait on the process: `poll`, `wait`, and
  `wait_timeout`.

The crate has minimal dependencies to third-party crates, only requiring
`libc` on Unix and `winapi` on Windows.  It is intended to work on Unix-like
platforms as well as on reasonably recent Windows.  It is regularly tested on
Linux, MacOS and Windows.

## API Overview

The API is separated in two parts: the low-level `Popen` API similar to
Python's
[`subprocess.Popen`](https://docs.python.org/3/library/subprocess.html#subprocess.Popen),
and the higher-level API for convenient creation of commands and pipelines.
The two can be mixed, so it is possible to use builder to create `Popen`
instances, and then to continue working with them directly.

While `Popen` loosely follows Python's [`subprocess`
module](https://docs.python.org/3/library/subprocess.html#popen-constructor),
it is not a literal translation.  Some of the changes accommodate the
differences between the languages, such as the lack of default and keyword
arguments in Rust, and others take advantage of Rust's more advanced type
system, or of additional capabilities such as the ownership system and the
`Drop` trait.  Python's utility functions such as `subprocess.run` are not
included because they can be better expressed using the builder pattern.

The high-level API offers an elegant process and pipeline creation interface,
along with convenience methods for capturing their output and exit status.

## Examples

### Spawning and redirecting

Execute an command and wait for it to complete:

```rust
let exit_status = Exec::cmd("umount").arg(dirname).join()?;
assert!(exit_status.success());
```

To prevent quoting issues and injection attacks, subprocess will not
spawn a shell unless explicitly requested.  To execute a command using
the OS shell, like C's `system`, use `Exec::shell`:

```rust
Exec::shell("shutdown -h now").join()?;
```

Start a subprocess and obtain its output as a `Read` trait object,
like C's `popen`:

```rust
let stream = Exec::cmd("find /").stream_stdout()?;
// Call stream.read_to_string, construct io::BufReader(stream) and iterate it
// by lines, etc...
```

Capture the output of a command:

```rust
let out = Exec::cmd("ls")
  .stdout(Redirection::Pipe)
  .capture()?
  .stdout_str();
```

Redirect standard error to standard output, and capture them in a string:

```rust
let out_and_err = Exec::cmd("ls")
  .stdout(Redirection::Pipe)
  .stderr(Redirection::Merge)
  .capture()?
  .stdout_str();
```

Provide some input to the command and read its output:

```rust
let out = Exec::cmd("sort")
  .stdin("b\nc\na\n")
  .stdout(Redirection::Pipe)
  .capture()?
  .stdout_str();
assert_eq!(out, "a\nb\nc\n");
```

Connecting `stdin` to an open file would have worked as well.

### Pipelines

`Popen` objects support connecting input and output to arbitrary open
files, including other `Popen` objects.  This can be used to form
pipelines of processes.  The builder API will do it automatically with
the `|` operator on `Exec` objects.

Execute a pipeline and return the exit status of the last command:

```rust
let exit_status =
  (Exec::shell("ls *.bak") | Exec::cmd("xargs").arg("rm")).join()?;
```

Capture the pipeline's output:

```rust
let dir_checksum = {
    Exec::shell("find . -type f") | Exec::cmd("sort") | Exec::cmd("sha1sum")
}.capture()?.stdout_str();
```

### The low-level Popen type

```rust
let mut p = Popen::create(&["command", "arg1", "arg2"], PopenConfig {
    stdout: Redirection::Pipe, ..Default::default()
})?;

// Since we requested stdout to be redirected to a pipe, the parent's
// end of the pipe is available as p.stdout.  It can either be read
// directly, or processed using the communicate() method:
let (out, err) = p.communicate(None)?;

// check if the process is still alive
if let Some(exit_status) = p.poll() {
  // the process has finished
} else {
  // it is still running, terminate it
  p.terminate()?;
}
```

### Querying and terminating

Check whether a previously launched process is still running:

```rust
let mut p = Exec::cmd("sleep").arg("2").popen()?;
thread::sleep(Duration::new(1, 0));
if p.poll().is_none() {
    // poll() returns Some(exit_status) if the process has completed
    println!("process is still running");
}
```

Give the process 1 second to run, and kill it if it didn't complete by
then.

```rust
let mut p = Exec::cmd("sleep").arg("2").popen()?;
if let Some(status) = p.wait_timeout(Duration::new(1, 0))? {
    println!("process finished as {:?}", status);
} else {
    p.kill()?;
    p.wait()?;
    println!("process killed");
}
```

## License

`subprocess` is distributed under the terms of both the MIT license
and the Apache License (Version 2.0).  See
[LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for
details.  Contributing changes is assumed to signal agreement with
these licensing terms.
