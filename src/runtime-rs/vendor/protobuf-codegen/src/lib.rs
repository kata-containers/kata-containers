//! # Protobuf code generator
//!
//! This crate contains protobuf code generator implementation
//! and a `protoc-gen-rust` `protoc` plugin.
//!
//! This crate:
//! * provides `protoc-gen-rust` plugin for `protoc` command
//! * implement protobuf codegen
//!
//! This crate is not meant to be used directly, in fact, it does not provide any public API
//! (except for `protoc-gen-rust` binary).
//!
//! Code can be generated with either:
//! * `protoc-gen-rust` plugin for `protoc` or
//! * [`protoc-rust`](https://docs.rs/protoc) crate
//!   (code generator which depends on `protoc` binary for parsing of `.proto` files)
//! * [`protobuf-codegen-pure`](https://docs.rs/protobuf-codegen-pure) crate,
//!   similar API to `protoc-rust`, but uses pure rust parser of `.proto` files.
//!
//! # `protoc-gen-rust` plugin for `protoc`
//!
//! When non-cargo build system is used, consider using standard protobuf code generation pattern:
//! `protoc` command does all the work of handling paths and parsing `.proto` files.
//! When `protoc` is invoked with `--rust_out=` option, it invokes `protoc-gen-rust` plugin.
//! provided by this crate.
//!
//! When building with cargo, consider using `protoc-rust` or `protobuf-codegen-pure` crates.
//!
//! ## How to use `protoc-gen-rust` if you have to
//!
//! (Note `protoc` can be invoked programmatically with
//! [protoc crate](https://docs.rs/protoc))
//!
//! 0) Install protobuf for `protoc` binary.
//!
//! On OS X [Homebrew](https://github.com/Homebrew/brew) can be used:
//!
//! ```sh
//! brew install protobuf
//! ```
//!
//! On Ubuntu, `protobuf-compiler` package can be installed:
//!
//! ```sh
//! apt-get install protobuf-compiler
//! ```
//!
//! Protobuf is needed only for code generation, `rust-protobuf` runtime
//! does not use `protobuf` library.
//!
//! 1) Install `protoc-gen-rust` program (which is `protoc` plugin)
//!
//! It can be installed either from source or with `cargo install protobuf` command.
//!
//! 2) Add `protoc-gen-rust` to $PATH
//!
//! If you installed it with cargo, it should be
//!
//! ```sh
//! PATH="$HOME/.cargo/bin:$PATH"
//! ```
//!
//! 3) Generate .rs files:
//!
//! ```sh
//! protoc --rust_out . foo.proto
//! ```
//!
//! This will generate .rs files in current directory.
//!
//! # Version 2
//!
//! This is documentation for version 2 of the crate.
//!
//! [Version 3 of the crate](https://docs.rs/protobuf-codegen/%3E=3.0.0-alpha)
//! (currently in development) encapsulates both `protoc` and pure codegens in this crate.

#![deny(rustdoc::broken_intra_doc_links)]
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

use inside::protobuf_crate_path;
#[doc(hidden)]
pub use protobuf_name::ProtobufAbsolutePath;
#[doc(hidden)]
pub use protobuf_name::ProtobufIdent;
#[doc(hidden)]
pub use protobuf_name::ProtobufRelativePath;
use scope::FileScope;
use scope::RootScope;

use self::code_writer::CodeWriter;
use self::enums::*;
use self::extensions::*;
use self::message::*;
use crate::file::proto_path_to_rust_mod;

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
    w.lazy_static(
        "file_descriptor_proto_lazy",
        &format!(
            "{}::descriptor::FileDescriptorProto",
            protobuf_crate_path(customize)
        ),
        customize,
    );
    w.write_line("");
    w.def_fn(
        &format!(
            "parse_descriptor_proto() -> {}::descriptor::FileDescriptorProto",
            protobuf_crate_path(customize)
        ),
        |w| {
            w.write_line(&format!(
                "{}::Message::parse_from_bytes(file_descriptor_proto_data).unwrap()",
                protobuf_crate_path(customize)
            ));
        },
    );
    w.write_line("");
    w.pub_fn(
        &format!(
            "file_descriptor_proto() -> &'static {}::descriptor::FileDescriptorProto",
            protobuf_crate_path(customize)
        ),
        |w| {
            w.block("file_descriptor_proto_lazy.get(|| {", "})", |w| {
                w.write_line("parse_descriptor_proto()");
            });
        },
    );
}

struct GenFileResult {
    compiler_plugin_result: compiler_plugin::GenResult,
    mod_name: String,
}

fn gen_file(
    file: &FileDescriptorProto,
    _files_map: &HashMap<&str, &FileDescriptorProto>,
    root_scope: &RootScope,
    customize: &Customize,
) -> GenFileResult {
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

    GenFileResult {
        compiler_plugin_result: compiler_plugin::GenResult {
            name: format!("{}.rs", proto_path_to_rust_mod(file.get_name())),
            content: v,
        },
        mod_name: proto_path_to_rust_mod(file.get_name()).into_string(),
    }
}

fn gen_mod_rs(mods: &[String]) -> compiler_plugin::GenResult {
    let mut v = Vec::new();
    let mut w = CodeWriter::new(&mut v);
    w.comment("@generated");
    w.write_line("");
    for m in mods {
        w.write_line(&format!("pub mod {};", m));
    }
    drop(w);
    compiler_plugin::GenResult {
        name: "mod.rs".to_owned(),
        content: v,
    }
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

    let mut mods = Vec::new();

    for file_name in files_to_generate {
        let file = files_map.get(&file_name[..]).expect(&format!(
            "file not found in file descriptors: {:?}, files: {:?}",
            file_name, all_file_names
        ));

        let gen_file_result = gen_file(file, &files_map, &root_scope, customize);
        results.push(gen_file_result.compiler_plugin_result);
        mods.push(gen_file_result.mod_name);
    }

    if customize.gen_mod_rs.unwrap_or(false) {
        results.push(gen_mod_rs(&mods));
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

/// Used in protobuf-codegen-identical-test
#[doc(hidden)]
pub fn proto_name_to_rs(name: &str) -> String {
    format!("{}.rs", proto_path_to_rust_mod(name))
}
