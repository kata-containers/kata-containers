# API to generate .rs files for ttrpc from protobuf

API to generate `.rs` files to be used e. g. [from build.rs](../example/build.rs).

Example code:

```rust
use ttrpc_codegen::Codegen;
use ttrpc_codegen::Customize;

fn main() {
    Codegen::new()
        .out_dir("protocols/sync")
        .inputs(&protos)
        .include("protocols/protos")
        .rust_protobuf()
        .customize(Customize {
            ..Default::default()
        })
        .run()
        .expect("Gen code failed.");
}

```

And in `Cargo.toml`:

```
[build-dependencies]
ttrpc-codegen = "0.3"
```

The alternative is to use
[protoc-rust crate](https://github.com/stepancheg/rust-protobuf/tree/master/protoc-rust),
which relies on `protoc` command to parse descriptors. Both crates should produce the same result,
otherwise please file a bug report.
