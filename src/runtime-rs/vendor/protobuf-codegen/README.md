<!-- cargo-sync-readme start -->

# Protobuf code generator

This crate contains protobuf code generator implementation
and a `protoc-gen-rust` `protoc` plugin.

This crate:
* provides `protoc-gen-rust` plugin for `protoc` command
* implement protobuf codegen

This crate is not meant to be used directly, in fact, it does not provide any public API
(except for `protoc-gen-rust` binary).

Code can be generated with either:
* `protoc-gen-rust` plugin for `protoc` or
* [`protoc-rust`](https://docs.rs/protoc) crate
  (code generator which depends on `protoc` binary for parsing of `.proto` files)
* [`protobuf-codegen-pure`](https://docs.rs/protobuf-codegen-pure) crate,
  similar API to `protoc-rust`, but uses pure rust parser of `.proto` files.

# `protoc-gen-rust` plugin for `protoc`

When non-cargo build system is used, consider using standard protobuf code generation pattern:
`protoc` command does all the work of handling paths and parsing `.proto` files.
When `protoc` is invoked with `--rust_out=` option, it invokes `protoc-gen-rust` plugin.
provided by this crate.

When building with cargo, consider using `protoc-rust` or `protobuf-codegen-pure` crates.

## How to use `protoc-gen-rust` if you have to

(Note `protoc` can be invoked programmatically with
[protoc crate](https://docs.rs/protoc))

0) Install protobuf for `protoc` binary.

On OS X [Homebrew](https://github.com/Homebrew/brew) can be used:

```sh
brew install protobuf
```

On Ubuntu, `protobuf-compiler` package can be installed:

```sh
apt-get install protobuf-compiler
```

Protobuf is needed only for code generation, `rust-protobuf` runtime
does not use `protobuf` library.

1) Install `protoc-gen-rust` program (which is `protoc` plugin)

It can be installed either from source or with `cargo install protobuf` command.

2) Add `protoc-gen-rust` to $PATH

If you installed it with cargo, it should be

```sh
PATH="$HOME/.cargo/bin:$PATH"
```

3) Generate .rs files:

```sh
protoc --rust_out . foo.proto
```

This will generate .rs files in current directory.

# Version 2

This is documentation for version 2 of the crate.

[Version 3 of the crate](https://docs.rs/protobuf-codegen/%3E=3.0.0-alpha)
(currently in development) encapsulates both `protoc` and pure codegens in this crate.

<!-- cargo-sync-readme end -->
