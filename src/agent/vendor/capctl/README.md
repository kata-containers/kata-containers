# capctl

[![crates.io](https://img.shields.io/crates/v/capctl.svg)](https://crates.io/crates/capctl)
[![Docs](https://docs.rs/capctl/badge.svg)](https://docs.rs/capctl)
[![GitHub Actions](https://github.com/cptpcrd/capctl/workflows/CI/badge.svg?branch=master&event=push)](https://github.com/cptpcrd/capctl/actions?query=workflow%3ACI+branch%3Amaster+event%3Apush)
[![codecov](https://codecov.io/gh/cptpcrd/capctl/branch/master/graph/badge.svg)](https://codecov.io/gh/cptpcrd/capctl)

A pure-Rust interface to `prctl()` and Linux capabilities.

### Features

This crate has the following features (by default, only `std` is enabled):

- `std`: Link against the standard library.

    Interfaces that depend on this feature are marked in the [documentation on docs.rs](https://docs.rs/capctl).

- `sc`: Allow making inline syscalls with the `sc` crate instead of calling into the system's libc for *some* operations.

    *Note: Currently, support for inline syscalls is limited to the following syscalls: `prctl()`, `capget()`, `capset()`, `setresuid()`, `setresgid()`, `setgroups()`. `capctl` will still call into the system's libc for most other syscalls.*

- `serde`: Enables implementations of `Serialize` and `Deserialize` for most (non-error) types.

### Why not [`caps`](https://crates.io/crates/caps)?

**TL;DR**: In the opinion of `capctl`'s author, `caps` adds too much abstraction and overhead.

1. The kernel APIs to access the 5 capability sets (permitted, effective, inheritable, bounding, and ambient) are very different. However, `caps` presents a unified interface that allows for manipulating all of them the same way.

   This is certainly more convenient to use. However, a) it minimizes the differences between the capabilities sets (something that is fundamental and must be understood to use capabilities properly), b) it allows users to write code that attempts to perform operations that are actually impossible (i.e. adding capabilities to the bounding capability set), and c) it can result in excessive syscalls (because operations that the kernel APIs allow to be performed together instead must done separately).

   Note: The author of `capctl` is not *completely* opposed to adding these kinds of interfaces, provided that lower-level APIs are also provided to allow users finer control. `caps`, however, does not do this.

2. `capctl` uses more efficient representations internally.

   For example, `caps` uses `HashSet`s to store sets of capabilities, which is wasteful. `capctl`, meanwhile, has a custom `CapSet` struct that stores a set of capabilities much more efficiently. (`CapSet` also has methods specially designed to work with capabilities, instead of just being a generalized set implementation.)

### Why not [`prctl`](https://crates.io/crates/prctl)?

**TL;DR**: `prctl` is a very low-level wrapper crate, and some of its "safe" code *should* be `unsafe`.

1. `prctl` concentrates on the `prctl()` system call, not Linux capabilities in general. As a result, its interface to Linux capabilities is an afterthought and incomplete.

2. `prctl` returns raw `errno` values when an error occurs. This crate returns a friendlier custom error type that can be converted into an `io::Error`.

3. Most importantly, `prctl` fails to recognize that, as the man page explains, `prctl()` is a very low-level syscall, and it should be used cautiously.

   As a result, some of the "safe" functions in `prctl` are actually highly unsafe! `prctl::set_mm()` is the worst example: it can be used to set raw addresses, such as the end of the heap (as with `brk()`), and it's a "safe" function! It even accepts these addresses as `libc::c_ulong`s instead of raw pointers, making it easy to abuse.
