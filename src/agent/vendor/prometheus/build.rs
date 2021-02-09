// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

#[cfg(feature = "gen")]
fn generate_protobuf_binding_file() {
    protobuf_codegen_pure::run(protobuf_codegen_pure::Args {
        out_dir: "proto",
        input: &["proto/proto_model.proto"],
        includes: &["proto"],
        ..Default::default()
    })
    .unwrap();
}

#[cfg(not(feature = "gen"))]
fn generate_protobuf_binding_file() {}

fn main() {
    generate_protobuf_binding_file()
}
