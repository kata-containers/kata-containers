//! # API to generate `.rs` files
//!
//! This API does not require `protoc` command present in `$PATH`.
//!
//! ```
//! extern crate protoc_rust;
//!
//! fn main() {
//!     protobuf_codegen_pure::Codegen::new()
//!         .out_dir("src/protos")
//!         .inputs(&["protos/a.proto", "protos/b.proto"])
//!         .include("protos")
//!         .run()
//!         .expect("Codegen failed.");
//! }
//! ```
//!
//! And in `Cargo.toml`:
//!
//! ```toml
//! [build-dependencies]
//! protobuf-codegen-pure = "2"
//! ```
//!
//! It is advisable that `protobuf-codegen-pure` build-dependecy version be the same as
//! `protobuf` dependency.
//!
//! The alternative is to use [`protoc-rust`](https://docs.rs/protoc-rust/=2) crate
//! which uses `protoc` command for parsing (so it uses the same parser
//! Google is using in their protobuf implementations).
//!
//! # Version 2
//!
//! This is documentation for version 2 of the crate.
//!
//! In version 3, this API is moved to
//! [`protobuf-codegen` crate](https://docs.rs/protobuf-codegen/%3E=3.0.0-alpha).

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

extern crate protobuf;
extern crate protobuf_codegen;

mod convert;

use std::error::Error;
use std::fmt;
use std::fmt::Formatter;
use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::path::StripPrefixError;

mod linked_hash_map;
mod model;
mod parser;

use linked_hash_map::LinkedHashMap;
pub use protobuf_codegen::Customize;

#[cfg(test)]
mod test_against_protobuf_protos;

/// Invoke pure rust codegen. See [crate docs](crate) for example.
// TODO: merge with protoc-rust def
#[derive(Debug, Default)]
pub struct Codegen {
    /// --lang_out= param
    out_dir: PathBuf,
    /// -I args
    includes: Vec<PathBuf>,
    /// List of .proto files to compile
    inputs: Vec<PathBuf>,
    /// Customize code generation
    customize: Customize,
}

impl Codegen {
    /// Fresh new codegen object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the output directory for codegen.
    pub fn out_dir(&mut self, out_dir: impl AsRef<Path>) -> &mut Self {
        self.out_dir = out_dir.as_ref().to_owned();
        self
    }

    /// Add an include directory.
    pub fn include(&mut self, include: impl AsRef<Path>) -> &mut Self {
        self.includes.push(include.as_ref().to_owned());
        self
    }

    /// Add include directories.
    pub fn includes(&mut self, includes: impl IntoIterator<Item = impl AsRef<Path>>) -> &mut Self {
        for include in includes {
            self.include(include);
        }
        self
    }

    /// Add an input (`.proto` file).
    pub fn input(&mut self, input: impl AsRef<Path>) -> &mut Self {
        self.inputs.push(input.as_ref().to_owned());
        self
    }

    /// Add inputs (`.proto` files).
    pub fn inputs(&mut self, inputs: impl IntoIterator<Item = impl AsRef<Path>>) -> &mut Self {
        for input in inputs {
            self.input(input);
        }
        self
    }

    /// Specify generated code [`Customize`] object.
    pub fn customize(&mut self, customize: Customize) -> &mut Self {
        self.customize = customize;
        self
    }

    /// Like `protoc --rust_out=...` but without requiring `protoc` or `protoc-gen-rust`
    /// commands in `$PATH`.
    pub fn run(&self) -> io::Result<()> {
        let includes: Vec<&Path> = self.includes.iter().map(|p| p.as_path()).collect();
        let inputs: Vec<&Path> = self.inputs.iter().map(|p| p.as_path()).collect();
        let p = parse_and_typecheck(&includes, &inputs)?;

        protobuf_codegen::gen_and_write(
            &p.file_descriptors,
            &p.relative_paths,
            &self.out_dir,
            &self.customize,
        )
    }
}

/// Arguments for pure rust codegen invocation.
// TODO: merge with protoc-rust def
#[derive(Debug, Default)]
#[deprecated(since = "2.14", note = "Use Codegen object instead")]
pub struct Args<'a> {
    /// --lang_out= param
    pub out_dir: &'a str,
    /// -I args
    pub includes: &'a [&'a str],
    /// List of .proto files to compile
    pub input: &'a [&'a str],
    /// Customize code generation
    pub customize: Customize,
}

/// Convert OS path to protobuf path (with slashes)
/// Function is `pub(crate)` for test.
pub(crate) fn relative_path_to_protobuf_path(path: &Path) -> String {
    assert!(path.is_relative());
    let path = path.to_str().expect("not a valid UTF-8 name");
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.to_owned()
    }
}

#[derive(Clone)]
struct FileDescriptorPair {
    parsed: model::FileDescriptor,
    descriptor: protobuf::descriptor::FileDescriptorProto,
}

#[derive(Debug)]
enum CodegenError {
    ParserErrorWithLocation(parser::ParserErrorWithLocation),
    ConvertError(convert::ConvertError),
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CodegenError::ParserErrorWithLocation(_) => write!(f, "Parse error"),
            CodegenError::ConvertError(_) => write!(f, "Could not typecheck parsed file"),
        }
    }
}

impl From<parser::ParserErrorWithLocation> for CodegenError {
    fn from(e: parser::ParserErrorWithLocation) -> Self {
        CodegenError::ParserErrorWithLocation(e)
    }
}

impl From<convert::ConvertError> for CodegenError {
    fn from(e: convert::ConvertError) -> Self {
        CodegenError::ConvertError(e)
    }
}

#[derive(Debug)]
struct WithFileError {
    file: String,
    error: CodegenError,
}

impl fmt::Display for WithFileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Could not write to {}: {}", self.file, self.error)
    }
}

impl Error for WithFileError {
    fn description(&self) -> &str {
        "WithFileError"
    }
}

struct Run<'a> {
    parsed_files: LinkedHashMap<String, FileDescriptorPair>,
    includes: &'a [&'a Path],
}

impl<'a> Run<'a> {
    fn get_file_and_all_deps_already_parsed(
        &self,
        protobuf_path: &str,
        result: &mut LinkedHashMap<String, FileDescriptorPair>,
    ) {
        if let Some(_) = result.get(protobuf_path) {
            return;
        }

        let pair = self
            .parsed_files
            .get(protobuf_path)
            .expect("must be already parsed");
        result.insert(protobuf_path.to_owned(), pair.clone());

        self.get_all_deps_already_parsed(&pair.parsed, result);
    }

    fn get_all_deps_already_parsed(
        &self,
        parsed: &model::FileDescriptor,
        result: &mut LinkedHashMap<String, FileDescriptorPair>,
    ) {
        for import in &parsed.imports {
            self.get_file_and_all_deps_already_parsed(&import.path, result);
        }
    }

    fn add_file(&mut self, protobuf_path: &str, fs_path: &Path) -> io::Result<()> {
        if let Some(_) = self.parsed_files.get(protobuf_path) {
            return Ok(());
        }

        let mut content = String::new();
        fs::File::open(fs_path)?.read_to_string(&mut content)?;

        self.add_file_content(protobuf_path, fs_path, &content)
    }

    fn add_file_content(
        &mut self,
        protobuf_path: &str,
        fs_path: &Path,
        content: &str,
    ) -> io::Result<()> {
        let parsed = model::FileDescriptor::parse(content).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                WithFileError {
                    file: format!("{}", fs_path.display()),
                    error: e.into(),
                },
            )
        })?;

        for import_path in &parsed.imports {
            self.add_imported_file(&import_path.path)?;
        }

        let mut this_file_deps = LinkedHashMap::new();
        self.get_all_deps_already_parsed(&parsed, &mut this_file_deps);

        let this_file_deps: Vec<_> = this_file_deps.into_iter().map(|(_, v)| v.parsed).collect();

        let descriptor =
            convert::file_descriptor(protobuf_path.to_owned(), &parsed, &this_file_deps).map_err(
                |e| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        WithFileError {
                            file: format!("{}", fs_path.display()),
                            error: e.into(),
                        },
                    )
                },
            )?;

        self.parsed_files.insert(
            protobuf_path.to_owned(),
            FileDescriptorPair { parsed, descriptor },
        );

        Ok(())
    }

    fn add_imported_file(&mut self, protobuf_path: &str) -> io::Result<()> {
        for include_dir in self.includes {
            let fs_path = Path::new(include_dir).join(protobuf_path);
            if fs_path.exists() {
                return self.add_file(protobuf_path, &fs_path);
            }
        }

        let embedded = match protobuf_path {
            "rustproto.proto" => Some(RUSTPROTO_PROTO),
            "google/protobuf/any.proto" => Some(ANY_PROTO),
            "google/protobuf/api.proto" => Some(API_PROTO),
            "google/protobuf/descriptor.proto" => Some(DESCRIPTOR_PROTO),
            "google/protobuf/duration.proto" => Some(DURATION_PROTO),
            "google/protobuf/empty.proto" => Some(EMPTY_PROTO),
            "google/protobuf/field_mask.proto" => Some(FIELD_MASK_PROTO),
            "google/protobuf/source_context.proto" => Some(SOURCE_CONTEXT_PROTO),
            "google/protobuf/struct.proto" => Some(STRUCT_PROTO),
            "google/protobuf/timestamp.proto" => Some(TIMESTAMP_PROTO),
            "google/protobuf/type.proto" => Some(TYPE_PROTO),
            "google/protobuf/wrappers.proto" => Some(WRAPPERS_PROTO),
            _ => None,
        };

        match embedded {
            Some(content) => {
                self.add_file_content(protobuf_path, Path::new(protobuf_path), content)
            }
            None => Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "protobuf path {:?} is not found in import path {:?}",
                    protobuf_path, self.includes
                ),
            )),
        }
    }

    fn strip_prefix<'b>(path: &'b Path, prefix: &Path) -> Result<&'b Path, StripPrefixError> {
        // special handling of `.` to allow successful `strip_prefix("foo.proto", ".")
        if prefix == Path::new(".") {
            Ok(path)
        } else {
            path.strip_prefix(prefix)
        }
    }

    fn add_fs_file(&mut self, fs_path: &Path) -> io::Result<String> {
        let relative_path = self
            .includes
            .iter()
            .filter_map(|include_dir| Self::strip_prefix(fs_path, include_dir).ok())
            .next();

        match relative_path {
            Some(relative_path) => {
                let protobuf_path = relative_path_to_protobuf_path(relative_path);
                self.add_file(&protobuf_path, fs_path)?;
                Ok(protobuf_path)
            }
            None => Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "file {:?} must reside in include path {:?}",
                    fs_path, self.includes
                ),
            )),
        }
    }
}

#[doc(hidden)]
pub struct ParsedAndTypechecked {
    pub relative_paths: Vec<String>,
    pub file_descriptors: Vec<protobuf::descriptor::FileDescriptorProto>,
}

#[doc(hidden)]
pub fn parse_and_typecheck(
    includes: &[&Path],
    input: &[&Path],
) -> io::Result<ParsedAndTypechecked> {
    let mut run = Run {
        parsed_files: LinkedHashMap::new(),
        includes: includes,
    };

    let mut relative_paths = Vec::new();

    for input in input {
        relative_paths.push(run.add_fs_file(&Path::new(input))?);
    }

    let file_descriptors: Vec<_> = run
        .parsed_files
        .into_iter()
        .map(|(_, v)| v.descriptor)
        .collect();

    Ok(ParsedAndTypechecked {
        relative_paths,
        file_descriptors,
    })
}

const RUSTPROTO_PROTO: &str = include_str!("proto/rustproto.proto");
const ANY_PROTO: &str = include_str!("proto/google/protobuf/any.proto");
const API_PROTO: &str = include_str!("proto/google/protobuf/api.proto");
const DESCRIPTOR_PROTO: &str = include_str!("proto/google/protobuf/descriptor.proto");
const DURATION_PROTO: &str = include_str!("proto/google/protobuf/duration.proto");
const EMPTY_PROTO: &str = include_str!("proto/google/protobuf/empty.proto");
const FIELD_MASK_PROTO: &str = include_str!("proto/google/protobuf/field_mask.proto");
const SOURCE_CONTEXT_PROTO: &str = include_str!("proto/google/protobuf/source_context.proto");
const STRUCT_PROTO: &str = include_str!("proto/google/protobuf/struct.proto");
const TIMESTAMP_PROTO: &str = include_str!("proto/google/protobuf/timestamp.proto");
const TYPE_PROTO: &str = include_str!("proto/google/protobuf/type.proto");
const WRAPPERS_PROTO: &str = include_str!("proto/google/protobuf/wrappers.proto");

/// Like `protoc --rust_out=...` but without requiring `protoc` or `protoc-gen-rust`
/// commands in `$PATH`.
#[deprecated(since = "2.14", note = "Use Codegen instead")]
#[allow(deprecated)]
pub fn run(args: Args) -> io::Result<()> {
    let includes: Vec<&Path> = args.includes.iter().map(|p| Path::new(p)).collect();
    let inputs: Vec<&Path> = args.input.iter().map(|p| Path::new(p)).collect();
    let p = parse_and_typecheck(&includes, &inputs)?;

    protobuf_codegen::gen_and_write(
        &p.file_descriptors,
        &p.relative_paths,
        &Path::new(&args.out_dir),
        &args.customize,
    )
}

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn test_relative_path_to_protobuf_path_windows() {
        assert_eq!(
            "foo/bar.proto",
            relative_path_to_protobuf_path(&Path::new("foo\\bar.proto"))
        );
    }

    #[test]
    fn test_relative_path_to_protobuf_path() {
        assert_eq!(
            "foo/bar.proto",
            relative_path_to_protobuf_path(&Path::new("foo/bar.proto"))
        );
    }
}
