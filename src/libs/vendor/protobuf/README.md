<!-- cargo-sync-readme start -->

# Library to read and write protocol buffers data

# Version 2 is stable

Currently developed branch of rust-protobuf [is 3](https://docs.rs/protobuf/%3E=3.0.0-alpha).
It has the same spirit as version 2, but contains numerous improvements like:
* runtime reflection for mutability, not just for access
* protobuf text format and JSON parsing (which rely on reflection)
* dynamic message support: work with protobuf data without generating code from schema

Stable version of rust-protobuf will be supported until version 3 released.

[Tracking issue for version 3](https://github.com/stepancheg/rust-protobuf/issues/518).

# How to generate rust code

There are several ways to generate rust code from `.proto` files

## Invoke `protoc` programmatically with protoc-rust crate (recommended)

Have a look at readme in [protoc-rust crate](https://docs.rs/protoc-rust/=2).

## Use pure rust protobuf parser and code generator

Readme should be in
[protobuf-codegen-pure crate](https://docs.rs/protobuf-codegen-pure/=2).

## Use protoc-gen-rust plugin

Readme is [here](https://docs.rs/protobuf-codegen/=2).

## Generated code

Have a look at generated files (for current development version),
used internally in rust-protobuf:

* [descriptor.rs](https://github.com/stepancheg/rust-protobuf/blob/master/protobuf/src/descriptor.rs)
  for [descriptor.proto](https://github.com/stepancheg/rust-protobuf/blob/master/protoc-bin-vendored/include/google/protobuf/descriptor.proto)
  (that is part of Google protobuf)

# Copy on write

Rust-protobuf can be used with [bytes crate](https://github.com/tokio-rs/bytes).

To enable `Bytes` you need to:

1. Enable `with-bytes` feature in rust-protobuf:

```rust
[dependencies]
protobuf = { version = "~2.0", features = ["with-bytes"] }
```

2. Enable bytes option

with `Customize` when codegen is invoked programmatically:

```rust
protoc_rust::run(protoc_rust::Args {
    ...
    customize: Customize {
        carllerche_bytes_for_bytes: Some(true),
        carllerche_bytes_for_string: Some(true),
        ..Default::default()
    },
});
```

or in `.proto` file:

```rust
import "rustproto.proto";

option (rustproto.carllerche_bytes_for_bytes_all) = true;
option (rustproto.carllerche_bytes_for_string_all) = true;
```

With these options enabled, fields of type `bytes` or `string` are
generated as `Bytes` or `Chars` respectively. When `CodedInputStream` is constructed
from `Bytes` object, fields of these types get subslices of original `Bytes` object,
instead of being allocated on heap.

# Accompanying crates

* [`protoc-rust`](https://docs.rs/protoc-rust/=2)
  and [`protobuf-codegen-pure`](https://docs.rs/protobuf-codegen-pure/=2)
  can be used to rust code from `.proto` crates.
* [`protobuf-codegen`](https://docs.rs/protobuf-codegen/=2) for `protoc-gen-rust` protoc plugin.
* [`protoc`](https://docs.rs/protoc/=2) crate can be used to invoke `protoc` programmatically.
* [`protoc-bin-vendored`](https://docs.rs/protoc-bin-vendored/=2) contains `protoc` command
  packed into the crate.

<!-- cargo-sync-readme end -->
