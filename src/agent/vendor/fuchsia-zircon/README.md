Rust bindings for Zircon kernel
================================

This repository contains Rust language bindings for Zircon kernel syscalls. The
main crate contains type-safe wrappers, while the inner "sys" crate contains the
raw types and FFI declarations.

There are two ways to build Rust artifacts targeting Fuchsia; using the
[Fargo](https://fuchsia.googlesource.com/fargo/) cross compiling tool or
including your [artifact in the GN
build](https://fuchsia.googlesource.com/docs/+/master/rust.md). Of the two,
Fargo is likely better for exploration and experimentation.
