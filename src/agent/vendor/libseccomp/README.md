# libseccomp-rs

[![build-test](https://github.com/libseccomp-rs/libseccomp-rs/actions/workflows/build-test.yaml/badge.svg?branch=main)](https://github.com/libseccomp-rs/libseccomp-rs/actions/workflows/build-test.yaml)
[![Latest release on crates.io](https://img.shields.io/crates/v/libseccomp.svg)](https://crates.io/crates/libseccomp)
[![Documentation on docs.rs](https://docs.rs/libseccomp/badge.svg)](https://docs.rs/libseccomp)
[![codecov](https://codecov.io/gh/libseccomp-rs/libseccomp-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/libseccomp-rs/libseccomp-rs)

Rust Language Bindings for the libseccomp Library

The libseccomp library provides an easy to use, platform independent, interface to
the Linux Kernel's syscall filtering mechanism. The libseccomp API is designed to
abstract away the underlying BPF based syscall filter language and present a more
conventional function-call based filtering interface that should be familiar to, and
easily adopted by, application developers.

The libseccomp-rs provides a Rust based interface to the libseccomp library.
This repository contains libseccomp and libseccomp-sys crates that enable developers
to use the libseccomp API in Rust.

* **libseccomp**: High-level safe API
* **libseccomp-sys**: Low-level unsafe API

[CHANGELOG](https://github.com/libseccomp-rs/libseccomp-rs/blob/main/CHANGELOG.md)

## Example

Create and load a single seccomp rule:

```rust
use libseccomp::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Creates and returns a new filter context.
    let mut filter = ScmpFilterContext::new_filter(ScmpAction::Allow)?;

    // Adds an architecture to the filter.
    filter.add_arch(ScmpArch::X8664)?;

    // Returns the number of a syscall by name.
    let syscall = ScmpSyscall::from_name("dup3")?;

    // Adds a single rule for an unconditional action on the syscall.
    filter.add_rule(ScmpAction::Errno(10), syscall)?;

    // Loads the filter context into the kernel.
    filter.load()?;

    // The dup3 fails by the seccomp rule.
    assert_eq!(
        unsafe { libc::dup3(0, 100, libc::O_CLOEXEC) } as i32,
        -libc::EPERM
    );
    assert_eq!(std::io::Error::last_os_error().raw_os_error().unwrap(), 10);

    Ok(())
}

```

## Requirements
Before using the libseccomp crate, you need to install the libseccomp library for your system.
The libseccomp library version 2.4 or newer is required.

### Installing the libseccomp library from a package

e.g. Debian-based Linux

``` sh
$ sudo apt install libseccomp-dev
```

### Building and installing the libseccomp library from sources
If you want to build the libseccomp library from an official release tarball instead of the package,
you should follow the quick step.

```sh
$ LIBSECCOMP_VERSION=2.5.3
$ wget https://github.com/seccomp/libseccomp/releases/download/v${LIBSECCOMP_VERSION}/libseccomp-${LIBSECCOMP_VERSION}.tar.gz
$ tar xvf libseccomp-${LIBSECCOMP_VERSION}.tar.gz
$ cd libseccomp-${LIBSECCOMP_VERSION}
$ ./configure
$ make
$ sudo make install
```

For more details, see the [libseccomp library repository](https://github.com/seccomp/libseccomp).

## Setup
If you use the libseccomp crate with dynamically linked the libseccomp library,
you do not need additional settings.

However, if you want to use the libseccomp crate against musl-libc with statically linked the libseccomp library,
you have to set the `LIBSECCOMP_LINK_TYPE` and `LIBSECCOMP_LIB_PATH` environment variables as follows.

```sh
$ export LIBSECCOMP_LINK_TYPE=static
$ export LIBSECCOMP_LIB_PATH="the path of the directory containing libseccomp.a (e.g. /usr/lib)"
```

> **Note**:
> To build the libseccomp crate against musl-libc, you need to build the libseccomp library manually for musl-libc
> or use a musl-based distribution that provides a package for the statically-linked libseccomp library

Now, add the following to your `Cargo.toml` to start building the libseccomp crate.

```toml
[dependencies]
libseccomp = "0.3.0"
```

## Testing the crate
The libseccomp crate provides a number of unit tests.
If you want to run the standard regression tests, you can execute the following command.

``` sh
$ make test
```

## How to contribute
Anyone is welcome to join and contribute code, documentation, and use cases.

For details on how to contribute to the libseccomp-rs project, please see the
[contributing document](https://github.com/libseccomp-rs/libseccomp-rs/blob/main/CONTRIBUTING.md).

## License
This crate is licensed under:

- MIT License (see LICENSE-MIT); or
- Apache 2.0 License (see LICENSE-APACHE),

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in libseccomp-rs by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
