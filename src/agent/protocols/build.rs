// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use ttrpc_codegen::{Codegen, Customize};

fn main() {
    let protos = vec![
        "protos/types.proto",
        "protos/agent.proto",
        "protos/health.proto",
        "protos/google/protobuf/empty.proto",
        "protos/oci.proto",
    ];

    Codegen::new()
        .out_dir("src")
        .inputs(&protos)
        .include("protos")
        .rust_protobuf()
        .customize(Customize {
            async_server: true,
            ..Default::default()
        })
        .run()
        .expect("Gen codes failed.");

    // There is a message named 'Box' in oci.proto
    // so there is a struct named 'Box', we should replace Box<Self> to ::std::boxed::Box<Self>
    // to avoid the conflict.
    replace_text_in_file(
        "src/oci.rs",
        "self: Box<Self>",
        "self: ::std::boxed::Box<Self>",
    )
    .unwrap();
}

fn replace_text_in_file(file_name: &str, from: &str, to: &str) -> Result<(), std::io::Error> {
    let new_contents = fs::read_to_string(file_name)?.replace(from, to);
    fs::write(&file_name, new_contents.as_bytes())
}
