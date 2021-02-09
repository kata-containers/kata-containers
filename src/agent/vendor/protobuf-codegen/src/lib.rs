//! This crate implement protobuf codegen.
//!
//! This crate:
//! * provides `protoc-gen-rust` plugin for `protoc` command
//! * implement protobuf codegen
//!
//! This crate is not meant to be used directly, in fact, it does not provide any public API
//! (except for `protoc-gen-rust` binary).
//!
//! Code can be generated with either:
//! * `protoc-gen-rust` binary or
//! * `protoc-rust` crate (codegen which depends on `protoc` binary for parsing)
//! * `protobuf-codegen-pure` crate

#![deny(intra_doc_link_resolution_failure)]
#![deny(missing_docs)]

extern crate protobuf;

use std::collections::hash_map::HashMap;
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;

use protobuf::compiler_plugin;
use protobuf::descriptor::*;
use protobuf::Message;

mod customize;
mod enums;
mod extensions;
mod field;
mod file;
mod file_and_mod;
mod file_descriptor;
#[doc(hidden)]
pub mod float;
mod inside;
mod message;
mod oneof;
mod protobuf_name;
mod rust_name;
mod rust_types_values;
mod serde;
mod well_known_types;

pub(crate) mod rust;
pub(crate) mod scope;
pub(crate) mod strx;
pub(crate) mod syntax;

use customize::customize_from_rustproto_for_file;
#[doc(hidden)]
pub use customize::Customize;

pub mod code_writer;

use self::code_writer::CodeWriter;
use self::enums::*;
use self::extensions::*;
use self::message::*;
use file::proto_path_to_rust_mod;
use inside::protobuf_crate_path;
use scope::FileScope;
use scope::RootScope;

fn escape_byte(s: &mut String, b: u8) {
    if b == b'\n' {
        write!(s, "\\n").unwrap();
    } else if b == b'\r' {
        write!(s, "\\r").unwrap();
    } else if b == b'\t' {
        write!(s, "\\t").unwrap();
    } else if b == b'\\' || b == b'"' {
        write!(s, "\\{}", b as char).unwrap();
    } else if b == b'\0' {
        write!(s, "\\0").unwrap();
    // ASCII printable except space
    } else if b > 0x20 && b < 0x7f {
        write!(s, "{}", b as char).unwrap();
    } else {
        write!(s, "\\x{:02x}", b).unwrap();
    }
}

fn write_file_descriptor_data(
    file: &FileDescriptorProto,
    customize: &Customize,
    w: &mut CodeWriter,
) {
    let fdp_bytes = file.write_to_bytes().unwrap();
    w.write_line("static file_descriptor_proto_data: &'static [u8] = b\"\\");
    w.indented(|w| {
        const MAX_LINE_LEN: usize = 72;

        let mut s = String::new();
        for &b in &fdp_bytes {
            let prev_len = s.len();
            escape_byte(&mut s, b);
            let truncate = s.len() > MAX_LINE_LEN;
            if truncate {
                s.truncate(prev_len);
            }
            if truncate || s.len() == MAX_LINE_LEN {
                write!(s, "\\").unwrap();
                w.write_line(&s);
                s.clear();
            }
            if truncate {
                escape_byte(&mut s, b);
            }
        }
        if !s.is_empty() {
            write!(s, "\\").unwrap();
            w.write_line(&s);
            s.clear();
        }
    });
    w.write_line("\";");
    w.write_line("");
    w.lazy_static_protobuf_path(
        "file_descriptor_proto_lazy",
        &format!(
            "{}::descriptor::FileDescriptorProto",
            protobuf_crate_path(customize)
        ),
        protobuf_crate_path(customize),
    );
    w.write_line("");
    w.def_fn(
        "parse_descriptor_proto() -> ::protobuf::descriptor::FileDescriptorProto",
        |w| {
            w.write_line("::protobuf::parse_from_bytes(file_descriptor_proto_data).unwrap()");
        },
    );
    w.write_line("");
    w.pub_fn(
        "file_descriptor_proto() -> &'static ::protobuf::descriptor::FileDescriptorProto",
        |w| {
            w.unsafe_expr(|w| {
                w.block("file_descriptor_proto_lazy.get(|| {", "})", |w| {
                    w.write_line("parse_descriptor_proto()");
                });
            });
        },
    );
}

fn gen_file(
    file: &FileDescriptorProto,
    _files_map: &HashMap<&str, &FileDescriptorProto>,
    root_scope: &RootScope,
    customize: &Customize,
) -> Option<compiler_plugin::GenResult> {
    // TODO: use it
    let mut customize = customize.clone();
    // options specified in invocation have precedence over options specified in file
    customize.update_with(&customize_from_rustproto_for_file(file.get_options()));

    let scope = FileScope {
        file_descriptor: file,
    }
    .to_scope();
    let lite_runtime = customize.lite_runtime.unwrap_or_else(|| {
        file.get_options().get_optimize_for() == FileOptions_OptimizeMode::LITE_RUNTIME
    });

    let mut v = Vec::new();

    {
        let mut w = CodeWriter::new(&mut v);

        w.write_generated_by("rust-protobuf", env!("CARGO_PKG_VERSION"));
        w.write_line(&format!("//! Generated file from `{}`", file.get_name()));

        w.write_line("");
        w.write_line("use protobuf::Message as Message_imported_for_functions;");
        w.write_line("use protobuf::ProtobufEnum as ProtobufEnum_imported_for_functions;");
        if customize.inside_protobuf != Some(true) {
            w.write_line("");
            w.write_line("/// Generated files are compatible only with the same version");
            w.write_line("/// of protobuf runtime.");
            w.commented(|w| {
                w.write_line(&format!(
                    "const _PROTOBUF_VERSION_CHECK: () = {}::{};",
                    protobuf_crate_path(&customize),
                    protobuf::VERSION_IDENT
                ));
            })
        }

        for message in &scope.get_messages() {
            // ignore map entries, because they are not used in map fields
            if message.map_entry().is_none() {
                w.write_line("");
                MessageGen::new(message, &root_scope, &customize).write(&mut w);
            }
        }
        for enum_type in &scope.get_enums() {
            w.write_line("");
            EnumGen::new(enum_type, file, &customize, root_scope).write(&mut w);
        }

        write_extensions(file, &root_scope, &mut w, &customize);

        if !lite_runtime {
            w.write_line("");
            write_file_descriptor_data(file, &customize, &mut w);
        }
    }

    Some(compiler_plugin::GenResult {
        name: format!("{}.rs", proto_path_to_rust_mod(file.get_name())),
        content: v,
    })
}

// This function is also used externally by cargo plugin
// https://github.com/plietar/rust-protobuf-build
// So be careful changing its signature.
#[doc(hidden)]
pub fn gen(
    file_descriptors: &[FileDescriptorProto],
    files_to_generate: &[String],
    customize: &Customize,
) -> Vec<compiler_plugin::GenResult> {
    let root_scope = RootScope {
        file_descriptors: file_descriptors,
    };

    let mut results: Vec<compiler_plugin::GenResult> = Vec::new();
    let files_map: HashMap<&str, &FileDescriptorProto> =
        file_descriptors.iter().map(|f| (f.get_name(), f)).collect();

    let all_file_names: Vec<&str> = file_descriptors.iter().map(|f| f.get_name()).collect();

    for file_name in files_to_generate {
        let file = files_map.get(&file_name[..]).expect(&format!(
            "file not found in file descriptors: {:?}, files: {:?}",
            file_name, all_file_names
        ));
        results.extend(gen_file(file, &files_map, &root_scope, customize));
    }
    results
}

#[doc(hidden)]
pub fn gen_and_write(
    file_descriptors: &[FileDescriptorProto],
    files_to_generate: &[String],
    out_dir: &Path,
    customize: &Customize,
) -> io::Result<()> {
    let results = gen(file_descriptors, files_to_generate, customize);

    for r in &results {
        let mut file_path = out_dir.to_owned();
        file_path.push(&r.name);
        let mut file_writer = File::create(&file_path)?;
        file_writer.write_all(&r.content)?;
        file_writer.flush()?;
    }

    Ok(())
}

#[doc(hidden)]
pub fn protoc_gen_rust_main() {
    compiler_plugin::plugin_main_2(|r| {
        let customize = Customize::parse_from_parameter(r.parameter).expect("parse options");
        gen(r.file_descriptors, r.files_to_generate, &customize)
    });
}
