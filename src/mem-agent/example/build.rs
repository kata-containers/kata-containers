// Copyright (C) 2024 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use ttrpc_codegen::{Codegen, Customize, ProtobufCustomize};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protos = vec![
        "protocols/protos/mem-agent.proto",
        "protocols/protos/google/protobuf/empty.proto",
        "protocols/protos/google/protobuf/timestamp.proto",
    ];

    let protobuf_customized = ProtobufCustomize::default().gen_mod_rs(false);

    Codegen::new()
        .out_dir("protocols/")
        .inputs(&protos)
        .include("protocols/protos/")
        .rust_protobuf()
        .customize(Customize {
            async_all: true,
            ..Default::default()
        })
        .rust_protobuf_customize(protobuf_customized.clone())
        .run()?;

    Ok(())
}
