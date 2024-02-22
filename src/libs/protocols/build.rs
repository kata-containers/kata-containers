// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
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
        ProtobufCustomize::default().before("#[cfg_attr(feature = \"with-serde\", derive(::serde::Serialize, ::serde::Deserialize))]")
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

fn replace_text_in_file(file_name: &str, from: &str, to: &str) -> Result<(), std::io::Error> {
    let mut src = File::open(file_name)?;
    let mut contents = String::new();
    src.read_to_string(&mut contents).unwrap();
    drop(src);

    let new_contents = contents.replace(from, to);

    let mut dst = File::create(file_name)?;
    dst.write_all(new_contents.as_bytes())?;

    Ok(())
}

fn use_serde(protos: &[&str], out_dir: &Path) -> Result<(), std::io::Error> {
    protos
        .iter()
        .try_for_each(|f: &&str| -> Result<(), std::io::Error> {
            let out_file = Path::new(f)
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or(format!("failed to get proto file name for {:?}", f))
                .map(|s| {
                    let t = s.replace(".proto", ".rs");
                    out_dir.join(t)
                })
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                .to_str()
                .ok_or(format!("cannot convert {:?} path to string", f))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                .to_string();

            replace_text_in_file(
                &out_file,
                "derive(Serialize, Deserialize)",
                "derive(serde::Serialize, serde::Deserialize)",
            )
        })
}

fn handle_file(autogen_comment: &str, rust_filename: &str) -> Result<(), std::io::Error> {
    let mut new_contents = Vec::new();

    let file = File::open(rust_filename)?;

    let reader = BufReader::new(file);

    // Guard the code since it is only needed for the agent-ctl tool,
    // not the agent itself.
    let serde_default_code = r#"#[cfg_attr(feature = "with-serde", serde(default))]"#;

    for line in reader.lines() {
        let line = line?;

        new_contents.push(line.clone());

        let pattern = "//! Generated file from";

        if line.starts_with(pattern) {
            new_contents.push(autogen_comment.into());
        }

        let struct_pattern = "pub struct ";

        // Although we've requested serde support via `Customize`, to
        // allow the `kata-agent-ctl` tool to partially deserialise structures
        // specified in JSON, we need this bit of additional magic.
        if line.starts_with(struct_pattern) {
            new_contents.insert(new_contents.len() - 1, serde_default_code.trim().into());
        }
    }

    let data = new_contents.join("\n");

    let mut dst = File::create(rust_filename)?;

    dst.write_all(data.as_bytes())?;

    Ok(())
}

fn codegen(path: &str, protos: &[&str], async_all: bool) -> Result<(), std::io::Error> {
    fs::create_dir_all(path).unwrap();

    // Tell Cargo that if the .proto files changed, to rerun this build script.
    protos
        .iter()
        .for_each(|p| println!("cargo:rerun-if-changed={}", &p));

    let ttrpc_options = Customize {
        async_all,
        ..Default::default()
    };

    let protobuf_options = ProtobufCustomize::default()
        .gen_mod_rs(false)
        .generate_getter(true)
        .generate_accessors(true);

    let out_dir = Path::new("src");

    Codegen::new()
        .out_dir(out_dir)
        .inputs(protos)
        .include("protos")
        .customize(ttrpc_options)
        .rust_protobuf()
        .rust_protobuf_customize(protobuf_options)
        .rust_protobuf_customize_callback(GenSerde)
        .run()?;

    let autogen_comment = format!("\n//! Generated by {:?} ({:?})", file!(), module_path!());
    for file in protos.iter() {
        let proto_filename = Path::new(file).file_name().unwrap();

        let generated_file = proto_filename
            .to_str()
            .ok_or("failed")
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
            .replace(".proto", ".rs");

        let out_file = out_dir.join(generated_file);

        let out_file_str = out_file
            .to_str()
            .ok_or("failed")
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        handle_file(&autogen_comment, out_file_str)?;
    }

    use_serde(protos, out_dir)?;
    Ok(())
}
fn real_main() -> Result<(), std::io::Error> {
    codegen(
        "src",
        &[
            "protos/google/protobuf/empty.proto",
            "protos/gogo/protobuf/gogoproto/gogo.proto",
            "protos/oci.proto",
            "protos/types.proto",
            "protos/csi.proto",
        ],
        false,
    )?;

    // generate async
    #[cfg(feature = "async")]
    {
        codegen("src", &["protos/agent.proto", "protos/health.proto"], true)?;

        fs::rename("src/agent_ttrpc.rs", "src/agent_ttrpc_async.rs")?;
        fs::rename("src/health_ttrpc.rs", "src/health_ttrpc_async.rs")?;
    }

    codegen("src", &["protos/agent.proto", "protos/health.proto"], false)?;

    // There is a message named 'Box' in oci.proto
    // so there is a struct named 'Box', we should replace Box<Self> to ::std::boxed::Box<Self>
    // to avoid the conflict.
    replace_text_in_file(
        "src/oci.rs",
        "self: Box<Self>",
        "self: ::std::boxed::Box<Self>",
    )?;

    Ok(())
}

fn real_main_grpctls() -> Result<(), std::io::Error> {

    tonic_build::configure()
        .out_dir("src/grpctls")
        .type_attribute("ContainerInfoList", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("ContainerInfo", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("CheckRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("CloseStdinRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("CreateContainerRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("GetMetricsRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("WaitProcessRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("SignalProcessRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("ReadStreamRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("WriteStreamRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("UpdateContainerRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("StatsContainerRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("StatsContainerResponse", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("CgroupStats", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("CpuStats", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("CpuUsage", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("MemoryStats", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("MemoryData", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("PidsStats", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("BlkioStats", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("BlkioStatsEntry", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("HugetlbStats", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("NetworkStats", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("ThrottlingData", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("TtyWinResizeRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("ListInterfacesRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("ListRoutesRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("Route", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("Interfaces", "#[derive(serde::Deserialize, serde::Serialize)]")
        .field_attribute("Interfaces", "#[serde (rename = \"Interfaces\")]")
        .type_attribute("Device", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("Spec", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(default)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("SharedMount", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("OnlineCPUMemRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("StartContainerRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("RemoveContainerRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("PauseContainerRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("ReseedRandomDevRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("GuestDetailsRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("GuestDetailsResponse", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("AgentDetails", "#[derive(serde::Deserialize, serde::Serialize)] #[serde (rename = \"AgentDetails\")]")
        .type_attribute("ResumeContainerRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("SetGuestDateTimeRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .field_attribute("Sec", "#[serde (rename = \"Sec\")]")
        .field_attribute("Usec", "#[serde (rename = \"Usec\")]")
        .type_attribute("CopyFileRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("GetOOMEventRequest", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(default)] #[serde(rename=\"GetOOMEventRequest\")]")
        .type_attribute("OOMEvent", "#[derive(serde::Deserialize, serde::Serialize)] #[serde (rename = \"OOMEvent\")]")
        .type_attribute("StringUser", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("Process", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(default)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("Box", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("User", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(default)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxCapabilities", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("POSIXRlimit", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("ExecProcessRequest", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("Storage", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("Windows", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("Solaris", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("Linux", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .field_attribute("UIDMappings", "#[serde (rename = \"UIDMappings\")]")
        .field_attribute("GIDMappings", "#[serde (rename = \"GIDMappings\")]")
        .field_attribute("type", "#[serde (rename(serialize = \"type_\", deserialize = \"type_\"))]")
        .type_attribute("LinuxIntelRdt", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxResources", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxIDMapping", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxSyscall", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxSeccomp", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxSeccompArg", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("FSGroup", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("ErrnoRet", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("oneof", "#[derive(serde::Deserialize, serde::Serialize)]")
        .type_attribute("LinuxBlockIO", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxWeightDevice", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxThrottleDevice", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxNetwork", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxInterfacePriority", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxHugepageLimit", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxMemory", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxCPU", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxPids", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxDeviceCgroup", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("LinuxNamespace", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("Root", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(default)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("Hooks", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(default)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("Hook", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(default)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("Mount", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(default)] ")
        .type_attribute("LinuxDevice", "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"PascalCase\")] ")
        .type_attribute("ServingStatus", "#[derive(serde::Deserialize, serde::Serialize)]")
        .field_attribute("UNKNOWN", "#[serde (rename = \"UNKNOWN\")]")
        .field_attribute("SERVING", "#[serde (rename = \"SERVING\")]")
        .field_attribute("NOT_SERVING", "#[serde (rename = \"NOT_SERVING\")]")
        .type_attribute("IPFamily", "#[derive(serde::Deserialize, serde::Serialize)] #[serde (rename = \"IPFamily\")]")
        .field_attribute("v4", "#[serde (rename = \"v4\")]")
        .field_attribute("v6", "#[serde (rename(serialize = \"v6\", deserialize = \"v6\"))]")
        .type_attribute("IPAddress", "#[derive(serde::Deserialize, serde::Serialize)] #[serde (rename = \"IPAddress\")] #[serde(default)]")
        .field_attribute("IPAddresses", "#[serde (rename = \"IPAddresses\")]")
        .type_attribute("Interface", "#[derive(serde::Deserialize, serde::Serialize)]")
        .field_attribute("hwAddr", "#[serde (rename = \"hwAddr\")]")
        .field_attribute("pciPath", "#[serde (rename = \"pciPath\")]")


        .compile(
            &["secprotos/secagent.proto",
            "secprotos/oci.proto",
            "secprotos/health.proto",
            "secprotos/types.proto"],
            &["secprotos"],
        )?;

    Ok(())
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("ERROR: {}", e);
        exit(1);
    }

    if let Err(e) = real_main_grpctls() {
        eprintln!("ERROR: {}", e);
        exit(1);
    }
}
