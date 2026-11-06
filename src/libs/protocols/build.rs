// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::path::Path;
use std::process::exit;

use protobuf::{
    descriptor::field_descriptor_proto::Type,
    reflect::{EnumDescriptor, FieldDescriptor, MessageDescriptor, OneofDescriptor},
};
use ttrpc_codegen::{Codegen, Customize, ProtobufCustomize, ProtobufCustomizeCallback};

struct GenSerde;

impl ProtobufCustomizeCallback for GenSerde {
    fn message(&self, _message: &MessageDescriptor) -> ProtobufCustomize {
        ProtobufCustomize::default().before(
            "#[cfg_attr(feature = \"with-serde\", derive(::serde::Serialize, ::serde::Deserialize))]\n#[cfg_attr(feature = \"with-serde\", serde(default))]",
        )
    }

    fn enumeration(&self, _enum_type: &EnumDescriptor) -> ProtobufCustomize {
        ProtobufCustomize::default().before("#[cfg_attr(feature = \"with-serde\", derive(::serde::Serialize, ::serde::Deserialize))]")
    }

    fn oneof(&self, _oneof: &OneofDescriptor) -> ProtobufCustomize {
        ProtobufCustomize::default().before("#[cfg_attr(feature = \"with-serde\", derive(::serde::Serialize, ::serde::Deserialize))]")
    }

    fn field(&self, field: &FieldDescriptor) -> ProtobufCustomize {
        if field.proto().type_() == Type::TYPE_ENUM {
            ProtobufCustomize::default().before(
                    "#[cfg_attr(feature = \"with-serde\", serde(serialize_with = \"crate::serialize_enum_or_unknown\", deserialize_with = \"crate::deserialize_enum_or_unknown\"))]",
                )
        } else if field.proto().type_() == Type::TYPE_MESSAGE && field.is_singular() {
            ProtobufCustomize::default().before(
                "#[cfg_attr(feature = \"with-serde\", serde(serialize_with = \"crate::serialize_message_field\", deserialize_with = \"crate::deserialize_message_field\"))]",
            )
        } else {
            ProtobufCustomize::default()
        }
    }

    fn special_field(&self, _message: &MessageDescriptor, _field: &str) -> ProtobufCustomize {
        ProtobufCustomize::default().before("#[cfg_attr(feature = \"with-serde\", serde(skip))]")
    }
}

const SERVICE_PROTOS: &[&str] = &[
    "protos/agent.proto",
    "protos/health.proto",
    "protos/confidential_data_hub.proto",
    "protos/remote.proto",
];

fn real_main() -> std::io::Result<()> {
    // Watch imported protos too; generated Rust files are outputs, not inputs.
    println!("cargo:rerun-if-changed=protos");
    let out_dir = Path::new("src/generated");
    fs::create_dir_all(out_dir)?;

    let mut protos = vec![
        "protos/gogo/protobuf/gogoproto/gogo.proto",
        "protos/oci.proto",
        "protos/types.proto",
        "protos/csi.proto",
        "protos/runtimeoptions.proto",
        "protos/cri-api/api.proto",
    ];
    protos.extend_from_slice(SERVICE_PROTOS);

    Codegen::new()
        .out_dir(out_dir)
        .inputs(&protos)
        .include("protos")
        .rust_protobuf()
        .rust_protobuf_customize(
            ProtobufCustomize::default()
                .gen_mod_rs(true)
                .generate_getter(true)
                .generate_accessors(true),
        )
        .rust_protobuf_customize_callback(GenSerde)
        .run()?;

    // Export bindings for the agent services. CRI's streaming services are
    // message-only consumers here and must not enter the sync module index.
    Codegen::new()
        .out_dir(out_dir)
        .inputs(SERVICE_PROTOS)
        .include("protos")
        .customize(Customize {
            gen_mod: true,
            ..Default::default()
        })
        .run()?;

    #[cfg(feature = "async")]
    {
        // Reuse the messages above: only service bindings differ between runtimes.
        let async_dir = Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("async");
        fs::create_dir_all(&async_dir)?;
        Codegen::new()
            .out_dir(&async_dir)
            .inputs(SERVICE_PROTOS)
            .include("protos")
            .customize(Customize {
                async_all: true,
                ..Default::default()
            })
            .run()?;

        for proto in SERVICE_PROTOS {
            let name = Path::new(proto).file_stem().unwrap().to_str().unwrap();
            fs::copy(
                async_dir.join(format!("{name}_ttrpc.rs")),
                out_dir.join(format!("{name}_ttrpc_async.rs")),
            )?;
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("ERROR: {e}");
        exit(1);
    }
}
