<!-- cargo-sync-readme start -->

# API to generate `.rs` files

This API does not require `protoc` command present in `$PATH`.

```rust
extern crate protoc_rust;

fn main() {
    protobuf_codegen_pure::Codegen::new()
        .out_dir("src/protos")
        .inputs(&["protos/a.proto", "protos/b.proto"])
        .include("protos")
        .run()
        .expect("Codegen failed.");
}
```

And in `Cargo.toml`:

```toml
[build-dependencies]
protobuf-codegen-pure = "2"
```

It is advisable that `protobuf-codegen-pure` build-dependecy version be the same as
`protobuf` dependency.

The alternative is to use [`protoc-rust`](https://docs.rs/protoc-rust/=2) crate
which uses `protoc` command for parsing (so it uses the same parser
Google is using in their protobuf implementations).

# Version 2

This is documentation for version 2 of the crate.

In version 3, this API is moved to
[`protobuf-codegen` crate](https://docs.rs/protobuf-codegen/%3E=3.0.0-alpha).

<!-- cargo-sync-readme end -->
