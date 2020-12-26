// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs::File;
use std::io::{Read, Write};

fn main() {
    let protos = vec![
        "protos/types.proto",
        "protos/agent.proto",
        "protos/health.proto",
        "protos/google/protobuf/empty.proto",
        "protos/oci.proto",
    ];

    // Tell Cargo that if the .proto files changed, to rerun this build script.
    protos
        .iter()
        .for_each(|p| println!("cargo:rerun-if-changed={}", &p));

    ttrpc_codegen::Codegen::new()
        .out_dir("src")
        .inputs(&protos)
        .include("protos")
        .rust_protobuf()
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
    let mut src = File::open(file_name)?;
    let mut contents = String::new();
    src.read_to_string(&mut contents).unwrap();
    drop(src);

    let new_contents = contents.replace(from, to);

    let mut dst = File::create(&file_name)?;
    dst.write_all(new_contents.as_bytes())?;

    Ok(())
}
