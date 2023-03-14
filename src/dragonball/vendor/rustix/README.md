<div align="center">
  <h1><code>rustix</code></h1>

  <p>
    <strong>Safe Rust bindings to POSIX/Unix/Linux/Winsock2 syscalls</strong>
  </p>

  <strong>A <a href="https://bytecodealliance.org/">Bytecode Alliance</a> project</strong>

  <p>
    <a href="https://github.com/bytecodealliance/rustix/actions?query=workflow%3ACI"><img src="https://github.com/bytecodealliance/rustix/workflows/CI/badge.svg" alt="Github Actions CI Status" /></a>
    <a href="https://bytecodealliance.zulipchat.com/#narrow/stream/206238-general"><img src="https://img.shields.io/badge/zulip-join_chat-brightgreen.svg" alt="zulip chat" /></a>
    <a href="https://crates.io/crates/rustix"><img src="https://img.shields.io/crates/v/rustix.svg" alt="crates.io page" /></a>
    <a href="https://docs.rs/rustix"><img src="https://docs.rs/rustix/badge.svg" alt="docs.rs docs" /></a>
  </p>
</div>

`rustix` provides efficient memory-safe and [I/O-safe] wrappers to POSIX-like,
Unix-like, Linux, and Winsock2 syscall-like APIs, with configurable backends.
It uses Rust references, slices, and return values instead of raw pointers, and
[`io-lifetimes`] instead of raw file descriptors, providing memory safety,
[I/O safety], and [provenance]. It uses `Result`s for reporting errors,
[`bitflags`] instead of bare integer flags, an [`Arg`] trait with optimizations
to efficiently accept any Rust string type, and several other efficient
conveniences.

`rustix` is low-level and, and while the `net` API supports Winsock2 on
Windows, the rest of the APIs do not support Windows; for higher-level and more
portable APIs built on this functionality, see the [`system-interface`],
[`cap-std`], and [`fs-set-times`] crates, for example.

`rustix` currently has two backends available:

 * linux-raw, which uses raw Linux system calls and vDSO calls, and is
   supported on Linux on x86-64, x86, aarch64, riscv64gc, powerpc64le,
   arm (v5 onwards), mipsel, and mips64el, with stable, nightly, and 1.48 Rust.
    - By being implemented entirely in Rust, avoiding `libc`, `errno`, and pthread
      cancellation, and employing some specialized optimizations, most functions
      compile down to very efficient code. On nightly Rust, they can often be
      fully inlined into user code.
    - Most functions in `linux_raw` preserve memory, I/O safety, and pointer
      provenance all the way down to the syscalls.
    - `linux_raw` uses a 64-bit `time_t` type on all platforms, avoiding the
      [y2038 bug].

 * libc, which uses the [`libc`] crate which provides bindings to native `libc`
   libraries on Unix-family platforms, and [`winapi`] for Winsock2 on Windows,
   and is portable to many OS's.

The linux-raw backend is enabled by default on platforms which support it. To
enable the libc backend instead, either enable the "use-libc" cargo feature:

```toml
rustix = { version = "...", features = ["use-libc"] }
````

or set the `RUSTFLAGS` environment variable to `--cfg=rustix_use_libc` when
building.

## Similar crates

`rustix` is similar to [`nix`], [`simple_libc`], [`unix`], [`nc`], and
[`uapi`]. `rustix` is architected for [I/O safety] with most APIs using
[`OwnedFd`] and [`AsFd`] to manipulate file descriptors rather than `File` or
even `c_int`, and supporting multiple backends so that it can use direct
syscalls while still being usable on all platforms `libc` supports. Like `nix`,
`rustix` has an optimized and flexible filename argument mechanism that allows
users to use a variety of string types, including non-UTF-8 string types.

[`relibc`] is a similar project which aims to be a full "libc", including
C-compatible interfaces and higher-level C/POSIX standard-library
functionality; `rustix` just aims to provide safe and idiomatic Rust interfaces
to low-level syscalls. `relibc` also doesn't tend to support features not
supported on Redox, such as `*at` functions like `openat`, which are
important features for `rustix`.

`rustix` has its own code for making direct syscalls, similar to the [`sc`]
and [`scall`] crates, though `rustix` can use either the unstable Rust `asm!`
macro or out-of-line `.s` files so it supports both Stable and Nightly Rust.
`rustix` can also use Linux's vDSO mechanism to optimize Linux `clock_gettime`
on all architectures, and all Linux system calls on x86. And `rustix`'s
syscalls report errors using an optimized `Error` type.

`rustix`'s `*at` functions are similar to the [`openat`] crate, but `rustix`
provides them as free functions rather than associated functions of a `Dir`
type. `rustix`'s `cwd()` function exposes the special `AT_FDCWD` value in a safe
way, so users don't need to open `.` to get a current-directory handle.

`rustix`'s `openat2` function is similar to the [`openat2`] crate, but uses
I/O safety types rather than `RawFd`. `rustix` does not provide dynamic feature
detection, so users must handle the [`NOSYS`] error themselves.

# Minimum Supported Rust Version (MSRV)

This crate currently works on the version of [Rust on Debian stable], which is
currently Rust 1.48. This policy may change in the future, in minor version
releases, so users using a fixed version of Rust should pin to a specific
version of this crate.

[Rust on Debian stable]: https://packages.debian.org/stable/rust/rustc
[`nix`]: https://crates.io/crates/nix
[`unix`]: https://crates.io/crates/unix
[`nc`]: https://crates.io/crates/nc
[`simple_libc`]: https://crates.io/crates/simple_libc
[`uapi`]: https://crates.io/crates/uapi
[`relibc`]: https://github.com/redox-os/relibc
[`syscall`]: https://crates.io/crates/syscall
[`sc`]: https://crates.io/crates/sc
[`scall`]: https://crates.io/crates/scall
[`system-interface`]: https://crates.io/crates/system-interface
[`openat`]: https://crates.io/crates/openat
[`openat2`]: https://crates.io/crates/openat2
[`fs-set-times`]: https://crates.io/crates/fs-set-times
[`io-lifetimes`]: https://crates.io/crates/io-lifetimes
[`libc`]: https://crates.io/crates/libc
[`winapi`]: https://crates.io/crates/winapi
[`cap-std`]: https://crates.io/crates/cap-std
[`bitflags`]: https://crates.io/crates/bitflags
[`Arg`]: https://docs.rs/rustix/latest/rustix/path/trait.Arg.html
[I/O-safe]: https://github.com/rust-lang/rfcs/blob/master/text/3128-io-safety.md
[I/O safety]: https://github.com/rust-lang/rfcs/blob/master/text/3128-io-safety.md
[provenance]: https://github.com/rust-lang/rust/issues/95228
[y2038 bug]: https://en.wikipedia.org/wiki/Year_2038_problem
[`OwnedFd`]: https://docs.rs/io-lifetimes/latest/io_lifetimes/struct.OwnedFd.html
[`AsFd`]: https://docs.rs/io-lifetimes/latest/io_lifetimes/trait.AsFd.html
[`NOSYS`]: https://docs.rs/rustix/latest/rustix/io/struct.Error.html#associatedconstant.NOSYS
