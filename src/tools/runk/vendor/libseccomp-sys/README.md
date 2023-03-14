# libseccomp-sys

[![Latest release on crates.io](https://img.shields.io/crates/v/libseccomp-sys.svg)](https://crates.io/crates/libseccomp-sys)
[![Documentation on docs.rs](https://docs.rs/libseccomp-sys/badge.svg)](https://docs.rs/libseccomp-sys)

Low-level bindings for the libseccomp library

The libseccomp-sys crate contains the raw FFI bindings to the
[libseccomp library](https://github.com/seccomp/libseccomp).

These low-level, mostly `unsafe` bindings are then used by the [libseccomp crate](https://crates.io/crates/libseccomp)
which wraps them in a nice to use, mostly safe API.
Therefore most users should not need to interact with this crate directly.

## Version information

Currently, the libseccomp-sys supports libseccomp version 2.5.3.
