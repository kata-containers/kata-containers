# API to generate .rs files for ttrpc from protobuf

API to generate `.rs` files to be used e. g. [from build.rs](../example/build.rs).

## Example

build.rs:

```rust
use ttrpc_codegen::Codegen;
use ttrpc_codegen::{Customize, ProtobufCustomize};

fn main() {
    let protos = vec![
        "protos/a.proto",
        "protos/b.proto",
    ];

    Codegen::new()
        .out_dir("protocols/sync")
        .inputs(&protos)
        .include("protocols/protos")
        .rust_protobuf()
        .customize(Customize {
            ..Default::default()
        })
        .rust_protobuf_customize(ProtobufCustomize {
            ..Default::default()
        }
        .run()
        .expect("Gen code failed.");
}

```

Cargo.toml:

```
[build-dependencies]
ttrpc-codegen = "0.2"
```

## Versions
| ttrpc-codegen version | ttrpc version |
| ------------- | ------------- |
| 0.1.x | <= 0.4.x  |
| 0.2.x  | >= 0.5.x  |

## Alternative
The alternative is to use
[protoc-rust crate](https://github.com/stepancheg/rust-protobuf/tree/master/protoc-rust),
which relies on `protoc` command to parse descriptors. Both crates should produce the same result,
otherwise please file a bug report.
