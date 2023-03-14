#![doc(html_root_url = "https://docs.rs/prost-build/0.11.5")]
#![allow(clippy::option_as_ref_deref, clippy::format_push_string)]

//! `prost-build` compiles `.proto` files into Rust.
//!
//! `prost-build` is designed to be used for build-time code generation as part of a Cargo
//! build-script.
//!
//! ## Example
//!
//! Let's create a small crate, `snazzy`, that defines a collection of
//! snazzy new items in a protobuf file.
//!
//! ```bash
//! $ cargo new snazzy && cd snazzy
//! ```
//!
//! First, add `prost-build`, `prost` and its public dependencies to `Cargo.toml`
//! (see [crates.io](https://crates.io/crates/prost) for the current versions):
//!
//! ```toml
//! [dependencies]
//! bytes = <bytes-version>
//! prost = <prost-version>
//!
//! [build-dependencies]
//! prost-build = { version = <prost-version> }
//! ```
//!
//! Next, add `src/items.proto` to the project:
//!
//! ```proto
//! syntax = "proto3";
//!
//! package snazzy.items;
//!
//! // A snazzy new shirt!
//! message Shirt {
//!     enum Size {
//!         SMALL = 0;
//!         MEDIUM = 1;
//!         LARGE = 2;
//!     }
//!
//!     string color = 1;
//!     Size size = 2;
//! }
//! ```
//!
//! To generate Rust code from `items.proto`, we use `prost-build` in the crate's
//! `build.rs` build-script:
//!
//! ```rust,no_run
//! use std::io::Result;
//! fn main() -> Result<()> {
//!     prost_build::compile_protos(&["src/items.proto"], &["src/"])?;
//!     Ok(())
//! }
//! ```
//!
//! And finally, in `lib.rs`, include the generated code:
//!
//! ```rust,ignore
//! // Include the `items` module, which is generated from items.proto.
//! // It is important to maintain the same structure as in the proto.
//! pub mod snazzy {
//!     pub mod items {
//!         include!(concat!(env!("OUT_DIR"), "/snazzy.items.rs"));
//!     }
//! }
//!
//! use snazzy::items;
//!
//! pub fn create_large_shirt(color: String) -> items::Shirt {
//!     let mut shirt = items::Shirt::default();
//!     shirt.color = color;
//!     shirt.set_size(items::shirt::Size::Large);
//!     shirt
//! }
//! ```
//!
//! That's it! Run `cargo doc` to see documentation for the generated code. The full
//! example project can be found on [GitHub](https://github.com/danburkert/snazzy).
//!
//! ### Cleaning up Markdown in code docs
//!
//! If you are using protobuf files from third parties, where the author of the protobuf
//! is not treating comments as Markdown, or is, but has codeblocks in their docs,
//! then you may need to clean up the documentation in order that `cargo test --doc`
//! will not fail spuriously, and that `cargo doc` doesn't attempt to render the
//! codeblocks as Rust code.
//!
//! To do this, in your `Cargo.toml`, add `features = ["cleanup-markdown"]` to the inclusion
//! of the `prost-build` crate and when your code is generated, the code docs will automatically
//! be cleaned up a bit.
//!
//! ## Sourcing `protoc`
//!
//! `prost-build` depends on the Protocol Buffers compiler, `protoc`, to parse `.proto` files into
//! a representation that can be transformed into Rust. If set, `prost-build` uses the `PROTOC`
//! for locating `protoc`. For example, on a macOS system where Protobuf is installed
//! with Homebrew, set the environment variables to:
//!
//! ```bash
//! PROTOC=/usr/local/bin/protoc
//! ```
//!
//! and in a typical Linux installation:
//!
//! ```bash
//! PROTOC=/usr/bin/protoc
//! ```
//!
//! If no `PROTOC` environment variable is set then `prost-build` will search the
//! current path for `protoc` or `protoc.exe`. If `prost-build` can not find `protoc`
//! via these methods the `compile_protos` method will fail.
//!
//! ### Compiling `protoc` from source
//!
//! To compile `protoc` from source you can use the `protobuf-src` crate and
//! set the correct environment variables.
//! ```no_run,ignore, rust
//! std::env::set_var("PROTOC", protobuf_src::protoc());
//!
//! // Now compile your proto files via prost-build
//! ```
//!
//! [`protobuf-src`]: https://docs.rs/protobuf-src

use std::collections::HashMap;
use std::default;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io::{Error, ErrorKind, Result, Write};
use std::ops::RangeToInclusive;
use std::path::{Path, PathBuf};
use std::process::Command;

use log::debug;
use log::trace;

use prost::Message;
use prost_types::{FileDescriptorProto, FileDescriptorSet};

pub use crate::ast::{Comments, Method, Service};
use crate::code_generator::CodeGenerator;
use crate::extern_paths::ExternPaths;
use crate::ident::to_snake;
use crate::message_graph::MessageGraph;
use crate::path::PathMap;

mod ast;
mod code_generator;
mod extern_paths;
mod ident;
mod message_graph;
mod path;

/// A service generator takes a service descriptor and generates Rust code.
///
/// `ServiceGenerator` can be used to generate application-specific interfaces
/// or implementations for Protobuf service definitions.
///
/// Service generators are registered with a code generator using the
/// `Config::service_generator` method.
///
/// A viable scenario is that an RPC framework provides a service generator. It generates a trait
/// describing methods of the service and some glue code to call the methods of the trait, defining
/// details like how errors are handled or if it is asynchronous. Then the user provides an
/// implementation of the generated trait in the application code and plugs it into the framework.
///
/// Such framework isn't part of Prost at present.
pub trait ServiceGenerator {
    /// Generates a Rust interface or implementation for a service, writing the
    /// result to `buf`.
    fn generate(&mut self, service: Service, buf: &mut String);

    /// Finalizes the generation process.
    ///
    /// In case there's something that needs to be output at the end of the generation process, it
    /// goes here. Similar to [`generate`](#method.generate), the output should be appended to
    /// `buf`.
    ///
    /// An example can be a module or other thing that needs to appear just once, not for each
    /// service generated.
    ///
    /// This still can be called multiple times in a lifetime of the service generator, because it
    /// is called once per `.proto` file.
    ///
    /// The default implementation is empty and does nothing.
    fn finalize(&mut self, _buf: &mut String) {}

    /// Finalizes the generation process for an entire protobuf package.
    ///
    /// This differs from [`finalize`](#method.finalize) by where (and how often) it is called
    /// during the service generator life cycle. This method is called once per protobuf package,
    /// making it ideal for grouping services within a single package spread across multiple
    /// `.proto` files.
    ///
    /// The default implementation is empty and does nothing.
    fn finalize_package(&mut self, _package: &str, _buf: &mut String) {}
}

/// The map collection type to output for Protobuf `map` fields.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
enum MapType {
    /// The [`std::collections::HashMap`] type.
    HashMap,
    /// The [`std::collections::BTreeMap`] type.
    BTreeMap,
}

impl Default for MapType {
    fn default() -> MapType {
        MapType::HashMap
    }
}

/// The bytes collection type to output for Protobuf `bytes` fields.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
enum BytesType {
    /// The [`alloc::collections::Vec::<u8>`] type.
    Vec,
    /// The [`bytes::Bytes`] type.
    Bytes,
}

impl Default for BytesType {
    fn default() -> BytesType {
        BytesType::Vec
    }
}

/// Configuration options for Protobuf code generation.
///
/// This configuration builder can be used to set non-default code generation options.
pub struct Config {
    file_descriptor_set_path: Option<PathBuf>,
    service_generator: Option<Box<dyn ServiceGenerator>>,
    map_type: PathMap<MapType>,
    bytes_type: PathMap<BytesType>,
    type_attributes: PathMap<String>,
    field_attributes: PathMap<String>,
    prost_types: bool,
    strip_enum_prefix: bool,
    out_dir: Option<PathBuf>,
    extern_paths: Vec<(String, String)>,
    default_package_filename: String,
    protoc_args: Vec<OsString>,
    disable_comments: PathMap<()>,
    skip_protoc_run: bool,
    include_file: Option<PathBuf>,
    prost_path: Option<String>,
    fmt: bool,
}

impl Config {
    /// Creates a new code generator configuration with default options.
    pub fn new() -> Config {
        Config::default()
    }

    /// Configure the code generator to generate Rust [`BTreeMap`][1] fields for Protobuf
    /// [`map`][2] type fields.
    ///
    /// # Arguments
    ///
    /// **`paths`** - paths to specific fields, messages, or packages which should use a Rust
    /// `BTreeMap` for Protobuf `map` fields. Paths are specified in terms of the Protobuf type
    /// name (not the generated Rust type name). Paths with a leading `.` are treated as fully
    /// qualified names. Paths without a leading `.` are treated as relative, and are suffix
    /// matched on the fully qualified field name. If a Protobuf map field matches any of the
    /// paths, a Rust `BTreeMap` field is generated instead of the default [`HashMap`][3].
    ///
    /// The matching is done on the Protobuf names, before converting to Rust-friendly casing
    /// standards.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # let mut config = prost_build::Config::new();
    /// // Match a specific field in a message type.
    /// config.btree_map(&[".my_messages.MyMessageType.my_map_field"]);
    ///
    /// // Match all map fields in a message type.
    /// config.btree_map(&[".my_messages.MyMessageType"]);
    ///
    /// // Match all map fields in a package.
    /// config.btree_map(&[".my_messages"]);
    ///
    /// // Match all map fields. Specially useful in `no_std` contexts.
    /// config.btree_map(&["."]);
    ///
    /// // Match all map fields in a nested message.
    /// config.btree_map(&[".my_messages.MyMessageType.MyNestedMessageType"]);
    ///
    /// // Match all fields named 'my_map_field'.
    /// config.btree_map(&["my_map_field"]);
    ///
    /// // Match all fields named 'my_map_field' in messages named 'MyMessageType', regardless of
    /// // package or nesting.
    /// config.btree_map(&["MyMessageType.my_map_field"]);
    ///
    /// // Match all fields named 'my_map_field', and all fields in the 'foo.bar' package.
    /// config.btree_map(&["my_map_field", ".foo.bar"]);
    /// ```
    ///
    /// [1]: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html
    /// [2]: https://developers.google.com/protocol-buffers/docs/proto3#maps
    /// [3]: https://doc.rust-lang.org/std/collections/struct.HashMap.html
    pub fn btree_map<I, S>(&mut self, paths: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.map_type.clear();
        for matcher in paths {
            self.map_type
                .insert(matcher.as_ref().to_string(), MapType::BTreeMap);
        }
        self
    }

    /// Configure the code generator to generate Rust [`bytes::Bytes`][1] fields for Protobuf
    /// [`bytes`][2] type fields.
    ///
    /// # Arguments
    ///
    /// **`paths`** - paths to specific fields, messages, or packages which should use a Rust
    /// `Bytes` for Protobuf `bytes` fields. Paths are specified in terms of the Protobuf type
    /// name (not the generated Rust type name). Paths with a leading `.` are treated as fully
    /// qualified names. Paths without a leading `.` are treated as relative, and are suffix
    /// matched on the fully qualified field name. If a Protobuf map field matches any of the
    /// paths, a Rust `Bytes` field is generated instead of the default [`Vec<u8>`][3].
    ///
    /// The matching is done on the Protobuf names, before converting to Rust-friendly casing
    /// standards.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # let mut config = prost_build::Config::new();
    /// // Match a specific field in a message type.
    /// config.bytes(&[".my_messages.MyMessageType.my_bytes_field"]);
    ///
    /// // Match all bytes fields in a message type.
    /// config.bytes(&[".my_messages.MyMessageType"]);
    ///
    /// // Match all bytes fields in a package.
    /// config.bytes(&[".my_messages"]);
    ///
    /// // Match all bytes fields. Specially useful in `no_std` contexts.
    /// config.bytes(&["."]);
    ///
    /// // Match all bytes fields in a nested message.
    /// config.bytes(&[".my_messages.MyMessageType.MyNestedMessageType"]);
    ///
    /// // Match all fields named 'my_bytes_field'.
    /// config.bytes(&["my_bytes_field"]);
    ///
    /// // Match all fields named 'my_bytes_field' in messages named 'MyMessageType', regardless of
    /// // package or nesting.
    /// config.bytes(&["MyMessageType.my_bytes_field"]);
    ///
    /// // Match all fields named 'my_bytes_field', and all fields in the 'foo.bar' package.
    /// config.bytes(&["my_bytes_field", ".foo.bar"]);
    /// ```
    ///
    /// [1]: https://docs.rs/bytes/latest/bytes/struct.Bytes.html
    /// [2]: https://developers.google.com/protocol-buffers/docs/proto3#scalar
    /// [3]: https://doc.rust-lang.org/std/vec/struct.Vec.html
    pub fn bytes<I, S>(&mut self, paths: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.bytes_type.clear();
        for matcher in paths {
            self.bytes_type
                .insert(matcher.as_ref().to_string(), BytesType::Bytes);
        }
        self
    }

    /// Add additional attribute to matched fields.
    ///
    /// # Arguments
    ///
    /// **`path`** - a path matching any number of fields. These fields get the attribute.
    /// For details about matching fields see [`btree_map`](#method.btree_map).
    ///
    /// **`attribute`** - an arbitrary string that'll be placed before each matched field. The
    /// expected usage are additional attributes, usually in concert with whole-type
    /// attributes set with [`type_attribute`](method.type_attribute), but it is not
    /// checked and anything can be put there.
    ///
    /// Note that the calls to this method are cumulative ‒ if multiple paths from multiple calls
    /// match the same field, the field gets all the corresponding attributes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # let mut config = prost_build::Config::new();
    /// // Prost renames fields named `in` to `in_`. But if serialized through serde,
    /// // they should as `in`.
    /// config.field_attribute("in", "#[serde(rename = \"in\")]");
    /// ```
    pub fn field_attribute<P, A>(&mut self, path: P, attribute: A) -> &mut Self
    where
        P: AsRef<str>,
        A: AsRef<str>,
    {
        self.field_attributes
            .insert(path.as_ref().to_string(), attribute.as_ref().to_string());
        self
    }

    /// Add additional attribute to matched messages, enums and one-ofs.
    ///
    /// # Arguments
    ///
    /// **`paths`** - a path matching any number of types. It works the same way as in
    /// [`btree_map`](#method.btree_map), just with the field name omitted.
    ///
    /// **`attribute`** - an arbitrary string to be placed before each matched type. The
    /// expected usage are additional attributes, but anything is allowed.
    ///
    /// The calls to this method are cumulative. They don't overwrite previous calls and if a
    /// type is matched by multiple calls of the method, all relevant attributes are added to
    /// it.
    ///
    /// For things like serde it might be needed to combine with [field
    /// attributes](#method.field_attribute).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # let mut config = prost_build::Config::new();
    /// // Nothing around uses floats, so we can derive real `Eq` in addition to `PartialEq`.
    /// config.type_attribute(".", "#[derive(Eq)]");
    /// // Some messages want to be serializable with serde as well.
    /// config.type_attribute("my_messages.MyMessageType",
    ///                       "#[derive(Serialize)] #[serde(rename_all = \"snake_case\")]");
    /// config.type_attribute("my_messages.MyMessageType.MyNestedMessageType",
    ///                       "#[derive(Serialize)] #[serde(rename_all = \"snake_case\")]");
    /// ```
    ///
    /// # Oneof fields
    ///
    /// The `oneof` fields don't have a type name of their own inside Protobuf. Therefore, the
    /// field name can be used both with `type_attribute` and `field_attribute` ‒ the first is
    /// placed before the `enum` type definition, the other before the field inside corresponding
    /// message `struct`.
    ///
    /// In other words, to place an attribute on the `enum` implementing the `oneof`, the match
    /// would look like `my_messages.MyMessageType.oneofname`.
    pub fn type_attribute<P, A>(&mut self, path: P, attribute: A) -> &mut Self
    where
        P: AsRef<str>,
        A: AsRef<str>,
    {
        self.type_attributes
            .insert(path.as_ref().to_string(), attribute.as_ref().to_string());
        self
    }

    /// Configures the code generator to use the provided service generator.
    pub fn service_generator(&mut self, service_generator: Box<dyn ServiceGenerator>) -> &mut Self {
        self.service_generator = Some(service_generator);
        self
    }

    /// Configures the code generator to not use the `prost_types` crate for Protobuf well-known
    /// types, and instead generate Protobuf well-known types from their `.proto` definitions.
    pub fn compile_well_known_types(&mut self) -> &mut Self {
        self.prost_types = false;
        self
    }

    /// Configures the code generator to omit documentation comments on generated Protobuf types.
    ///
    /// # Example
    ///
    /// Occasionally `.proto` files contain code blocks which are not valid Rust. To avoid doctest
    /// failures, annotate the invalid code blocks with an [`ignore` or `no_run` attribute][1], or
    /// disable doctests for the crate with a [Cargo.toml entry][2]. If neither of these options
    /// are possible, then omit comments on generated code during doctest builds:
    ///
    /// ```rust,no_run
    /// # fn main() -> std::io::Result<()> {
    /// let mut config = prost_build::Config::new();
    /// config.disable_comments(&["."]);
    /// config.compile_protos(&["src/frontend.proto", "src/backend.proto"], &["src"])?;
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// As with other options which take a set of paths, comments can be disabled on a per-package
    /// or per-symbol basis.
    ///
    /// [1]: https://doc.rust-lang.org/rustdoc/documentation-tests.html#attributes
    /// [2]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html#configuring-a-target
    pub fn disable_comments<I, S>(&mut self, paths: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.disable_comments.clear();
        for matcher in paths {
            self.disable_comments
                .insert(matcher.as_ref().to_string(), ());
        }
        self
    }

    /// Declare an externally provided Protobuf package or type.
    ///
    /// `extern_path` allows `prost` types in external crates to be referenced in generated code.
    ///
    /// When `prost` compiles a `.proto` which includes an import of another `.proto`, it will
    /// automatically recursively compile the imported file as well. `extern_path` can be used
    /// to instead substitute types from an external crate.
    ///
    /// # Example
    ///
    /// As an example, consider a crate, `uuid`, with a `prost`-generated `Uuid` type:
    ///
    /// ```proto
    /// // uuid.proto
    ///
    /// syntax = "proto3";
    /// package uuid;
    ///
    /// message Uuid {
    ///     string uuid_str = 1;
    /// }
    /// ```
    ///
    /// The `uuid` crate implements some traits for `Uuid`, and publicly exports it:
    ///
    /// ```rust,ignore
    /// // lib.rs in the uuid crate
    ///
    /// include!(concat!(env!("OUT_DIR"), "/uuid.rs"));
    ///
    /// pub trait DoSomething {
    ///     fn do_it(&self);
    /// }
    ///
    /// impl DoSomething for Uuid {
    ///     fn do_it(&self) {
    ///         println!("Done");
    ///     }
    /// }
    /// ```
    ///
    /// A separate crate, `my_application`, uses `prost` to generate message types which reference
    /// `Uuid`:
    ///
    /// ```proto
    /// // my_application.proto
    ///
    /// syntax = "proto3";
    /// package my_application;
    ///
    /// import "uuid.proto";
    ///
    /// message MyMessage {
    ///     uuid.Uuid message_id = 1;
    ///     string some_payload = 2;
    /// }
    /// ```
    ///
    /// Additionally, `my_application` depends on the trait impls provided by the `uuid` crate:
    ///
    /// ```rust,ignore
    /// // `main.rs` of `my_application`
    ///
    /// use uuid::{DoSomething, Uuid};
    ///
    /// include!(concat!(env!("OUT_DIR"), "/my_application.rs"));
    ///
    /// pub fn process_message(msg: MyMessage) {
    ///     if let Some(uuid) = msg.message_id {
    ///         uuid.do_it();
    ///     }
    /// }
    /// ```
    ///
    /// Without configuring `uuid` as an external path in `my_application`'s `build.rs`, `prost`
    /// would compile a completely separate version of the `Uuid` type, and `process_message` would
    /// fail to compile. However, if `my_application` configures `uuid` as an extern path with a
    /// call to `.extern_path(".uuid", "::uuid")`, `prost` will use the external type instead of
    /// compiling a new version of `Uuid`. Note that the configuration could also be specified as
    /// `.extern_path(".uuid.Uuid", "::uuid::Uuid")` if only the `Uuid` type were externally
    /// provided, and not the whole `uuid` package.
    ///
    /// # Usage
    ///
    /// `extern_path` takes a fully-qualified Protobuf path, and the corresponding Rust path that
    /// it will be substituted with in generated code. The Protobuf path can refer to a package or
    /// a type, and the Rust path should correspondingly refer to a Rust module or type.
    ///
    /// ```rust
    /// # let mut config = prost_build::Config::new();
    /// // Declare the `uuid` Protobuf package and all nested packages and types as externally
    /// // provided by the `uuid` crate.
    /// config.extern_path(".uuid", "::uuid");
    ///
    /// // Declare the `foo.bar.baz` Protobuf package and all nested packages and types as
    /// // externally provided by the `foo_bar_baz` crate.
    /// config.extern_path(".foo.bar.baz", "::foo_bar_baz");
    ///
    /// // Declare the `uuid.Uuid` Protobuf type (and all nested types) as externally provided
    /// // by the `uuid` crate's `Uuid` type.
    /// config.extern_path(".uuid.Uuid", "::uuid::Uuid");
    /// ```
    pub fn extern_path<P1, P2>(&mut self, proto_path: P1, rust_path: P2) -> &mut Self
    where
        P1: Into<String>,
        P2: Into<String>,
    {
        self.extern_paths
            .push((proto_path.into(), rust_path.into()));
        self
    }

    /// When set, the `FileDescriptorSet` generated by `protoc` is written to the provided
    /// filesystem path.
    ///
    /// This option can be used in conjunction with the [`include_bytes!`] macro and the types in
    /// the `prost-types` crate for implementing reflection capabilities, among other things.
    ///
    /// ## Example
    ///
    /// In `build.rs`:
    ///
    /// ```rust, no_run
    /// # use std::env;
    /// # use std::path::PathBuf;
    /// # let mut config = prost_build::Config::new();
    /// config.file_descriptor_set_path(
    ///     PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR environment variable not set"))
    ///         .join("file_descriptor_set.bin"));
    /// ```
    ///
    /// In `lib.rs`:
    ///
    /// ```rust,ignore
    /// let file_descriptor_set_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/file_descriptor_set.bin"));
    /// let file_descriptor_set = prost_types::FileDescriptorSet::decode(&file_descriptor_set_bytes[..]).unwrap();
    /// ```
    pub fn file_descriptor_set_path<P>(&mut self, path: P) -> &mut Self
    where
        P: Into<PathBuf>,
    {
        self.file_descriptor_set_path = Some(path.into());
        self
    }

    /// In combination with with `file_descriptor_set_path`, this can be used to provide a file
    /// descriptor set as an input file, rather than having prost-build generate the file by calling
    /// protoc.
    ///
    /// In `build.rs`:
    ///
    /// ```rust
    /// # let mut config = prost_build::Config::new();
    /// config.file_descriptor_set_path("path/from/build/system")
    ///     .skip_protoc_run()
    ///     .compile_protos(&["src/items.proto"], &["src/"]);
    /// ```
    ///
    pub fn skip_protoc_run(&mut self) -> &mut Self {
        self.skip_protoc_run = true;
        self
    }

    /// Configures the code generator to not strip the enum name from variant names.
    ///
    /// Protobuf enum definitions commonly include the enum name as a prefix of every variant name.
    /// This style is non-idiomatic in Rust, so by default `prost` strips the enum name prefix from
    /// variants which include it. Configuring this option prevents `prost` from stripping the
    /// prefix.
    pub fn retain_enum_prefix(&mut self) -> &mut Self {
        self.strip_enum_prefix = false;
        self
    }

    /// Configures the output directory where generated Rust files will be written.
    ///
    /// If unset, defaults to the `OUT_DIR` environment variable. `OUT_DIR` is set by Cargo when
    /// executing build scripts, so `out_dir` typically does not need to be configured.
    pub fn out_dir<P>(&mut self, path: P) -> &mut Self
    where
        P: Into<PathBuf>,
    {
        self.out_dir = Some(path.into());
        self
    }

    /// Configures what filename protobufs with no package definition are written to.
    pub fn default_package_filename<S>(&mut self, filename: S) -> &mut Self
    where
        S: Into<String>,
    {
        self.default_package_filename = filename.into();
        self
    }

    /// Configures the path that's used for deriving `Message` for generated messages.
    /// This is mainly useful for generating crates that wish to re-export prost.
    /// Defaults to `::prost::Message` if not specified.
    pub fn prost_path<S>(&mut self, path: S) -> &mut Self
    where
        S: Into<String>,
    {
        self.prost_path = Some(path.into());
        self
    }

    /// Add an argument to the `protoc` protobuf compilation invocation.
    ///
    /// # Example `build.rs`
    ///
    /// ```rust,no_run
    /// # use std::io::Result;
    /// fn main() -> Result<()> {
    ///   let mut prost_build = prost_build::Config::new();
    ///   // Enable a protoc experimental feature.
    ///   prost_build.protoc_arg("--experimental_allow_proto3_optional");
    ///   prost_build.compile_protos(&["src/frontend.proto", "src/backend.proto"], &["src"])?;
    ///   Ok(())
    /// }
    /// ```
    pub fn protoc_arg<S>(&mut self, arg: S) -> &mut Self
    where
        S: AsRef<OsStr>,
    {
        self.protoc_args.push(arg.as_ref().to_owned());
        self
    }

    /// Configures the optional module filename for easy inclusion of all generated Rust files
    ///
    /// If set, generates a file (inside the `OUT_DIR` or `out_dir()` as appropriate) which contains
    /// a set of `pub mod XXX` statements combining to load all Rust files generated.  This can allow
    /// for a shortcut where multiple related proto files have been compiled together resulting in
    /// a semi-complex set of includes.
    ///
    /// Turning a need for:
    ///
    /// ```rust,no_run,ignore
    /// pub mod Foo {
    ///     pub mod Bar {
    ///         include!(concat!(env!("OUT_DIR"), "/foo.bar.rs"));
    ///     }
    ///     pub mod Baz {
    ///         include!(concat!(env!("OUT_DIR"), "/foo.baz.rs"));
    ///     }
    /// }
    /// ```
    ///
    /// Into the simpler:
    ///
    /// ```rust,no_run,ignore
    /// include!(concat!(env!("OUT_DIR"), "/_includes.rs"));
    /// ```
    pub fn include_file<P>(&mut self, path: P) -> &mut Self
    where
        P: Into<PathBuf>,
    {
        self.include_file = Some(path.into());
        self
    }

    /// Configures the code generator to format the output code via `prettyplease`.
    ///
    /// By default, this is enabled but if the `format` feature is not enabled this does
    /// nothing.
    pub fn format(&mut self, enabled: bool) -> &mut Self {
        self.fmt = enabled;
        self
    }

    /// Compile `.proto` files into Rust files during a Cargo build with additional code generator
    /// configuration options.
    ///
    /// This method is like the `prost_build::compile_protos` function, with the added ability to
    /// specify non-default code generation options. See that function for more information about
    /// the arguments and generated outputs.
    ///
    /// The `protos` and `includes` arguments are ignored if `skip_protoc_run` is specified.
    ///
    /// # Example `build.rs`
    ///
    /// ```rust,no_run
    /// # use std::io::Result;
    /// fn main() -> Result<()> {
    ///   let mut prost_build = prost_build::Config::new();
    ///   prost_build.btree_map(&["."]);
    ///   prost_build.compile_protos(&["src/frontend.proto", "src/backend.proto"], &["src"])?;
    ///   Ok(())
    /// }
    /// ```
    pub fn compile_protos(
        &mut self,
        protos: &[impl AsRef<Path>],
        includes: &[impl AsRef<Path>],
    ) -> Result<()> {
        let mut target_is_env = false;
        let target: PathBuf = self.out_dir.clone().map(Ok).unwrap_or_else(|| {
            env::var_os("OUT_DIR")
                .ok_or_else(|| {
                    Error::new(ErrorKind::Other, "OUT_DIR environment variable is not set")
                })
                .map(|val| {
                    target_is_env = true;
                    Into::into(val)
                })
        })?;

        // TODO: This should probably emit 'rerun-if-changed=PATH' directives for cargo, however
        // according to [1] if any are output then those paths replace the default crate root,
        // which is undesirable. Figure out how to do it in an additive way; perhaps gcc-rs has
        // this figured out.
        // [1]: http://doc.crates.io/build-script.html#outputs-of-the-build-script

        let tmp;
        let file_descriptor_set_path = if let Some(path) = &self.file_descriptor_set_path {
            path.clone()
        } else {
            if self.skip_protoc_run {
                return Err(Error::new(
                    ErrorKind::Other,
                    "file_descriptor_set_path is required with skip_protoc_run",
                ));
            }
            tmp = tempfile::Builder::new().prefix("prost-build").tempdir()?;
            tmp.path().join("prost-descriptor-set")
        };

        if !self.skip_protoc_run {
            let protoc = protoc_from_env();

            let mut cmd = Command::new(protoc.clone());
            cmd.arg("--include_imports")
                .arg("--include_source_info")
                .arg("-o")
                .arg(&file_descriptor_set_path);

            for include in includes {
                if include.as_ref().exists() {
                    cmd.arg("-I").arg(include.as_ref());
                } else {
                    debug!(
                        "ignoring {} since it does not exist.",
                        include.as_ref().display()
                    )
                }
            }

            // Set the protoc include after the user includes in case the user wants to
            // override one of the built-in .protos.
            if let Some(protoc_include) = protoc_include_from_env() {
                cmd.arg("-I").arg(protoc_include);
            }

            for arg in &self.protoc_args {
                cmd.arg(arg);
            }

            for proto in protos {
                cmd.arg(proto.as_ref());
            }

            debug!("Running: {:?}", cmd);

            let output = cmd.output().map_err(|error| {
                Error::new(
                    error.kind(),
                    format!("failed to invoke protoc (hint: https://docs.rs/prost-build/#sourcing-protoc): (path: {:?}): {}", &protoc, error),
                )
            })?;

            if !output.status.success() {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("protoc failed: {}", String::from_utf8_lossy(&output.stderr)),
                ));
            }
        }

        let buf = fs::read(&file_descriptor_set_path).map_err(|e| {
            Error::new(
                e.kind(),
                format!(
                    "unable to open file_descriptor_set_path: {:?}, OS: {}",
                    &file_descriptor_set_path, e
                ),
            )
        })?;
        let file_descriptor_set = FileDescriptorSet::decode(&*buf).map_err(|error| {
            Error::new(
                ErrorKind::InvalidInput,
                format!("invalid FileDescriptorSet: {}", error),
            )
        })?;

        let requests = file_descriptor_set
            .file
            .into_iter()
            .map(|descriptor| {
                (
                    Module::from_protobuf_package_name(descriptor.package()),
                    descriptor,
                )
            })
            .collect::<Vec<_>>();

        let file_names = requests
            .iter()
            .map(|req| {
                (
                    req.0.clone(),
                    req.0.to_file_name_or(&self.default_package_filename),
                )
            })
            .collect::<HashMap<Module, String>>();

        let modules = self.generate(requests)?;
        for (module, content) in &modules {
            let file_name = file_names
                .get(module)
                .expect("every module should have a filename");
            let output_path = target.join(file_name);

            let previous_content = fs::read(&output_path);

            if previous_content
                .map(|previous_content| previous_content == content.as_bytes())
                .unwrap_or(false)
            {
                trace!("unchanged: {:?}", file_name);
            } else {
                trace!("writing: {:?}", file_name);
                fs::write(output_path, content)?;
            }
        }

        if let Some(ref include_file) = self.include_file {
            trace!("Writing include file: {:?}", target.join(include_file));
            let mut file = fs::File::create(target.join(include_file))?;
            self.write_includes(
                modules.keys().collect(),
                &mut file,
                0,
                if target_is_env { None } else { Some(&target) },
            )?;
            file.flush()?;
        }

        Ok(())
    }

    fn write_includes(
        &self,
        mut entries: Vec<&Module>,
        outfile: &mut fs::File,
        depth: usize,
        basepath: Option<&PathBuf>,
    ) -> Result<usize> {
        let mut written = 0;
        entries.sort();

        while !entries.is_empty() {
            let modident = entries[0].part(depth);
            let matching: Vec<&Module> = entries
                .iter()
                .filter(|&v| v.part(depth) == modident)
                .copied()
                .collect();
            {
                // Will NLL sort this mess out?
                let _temp = entries
                    .drain(..)
                    .filter(|&v| v.part(depth) != modident)
                    .collect();
                entries = _temp;
            }
            self.write_line(outfile, depth, &format!("pub mod {} {{", modident))?;
            let subwritten = self.write_includes(
                matching
                    .iter()
                    .filter(|v| v.len() > depth + 1)
                    .copied()
                    .collect(),
                outfile,
                depth + 1,
                basepath,
            )?;
            written += subwritten;
            if subwritten != matching.len() {
                let modname = matching[0].to_partial_file_name(..=depth);
                if basepath.is_some() {
                    self.write_line(
                        outfile,
                        depth + 1,
                        &format!("include!(\"{}.rs\");", modname),
                    )?;
                } else {
                    self.write_line(
                        outfile,
                        depth + 1,
                        &format!("include!(concat!(env!(\"OUT_DIR\"), \"/{}.rs\"));", modname),
                    )?;
                }
                written += 1;
            }

            self.write_line(outfile, depth, "}")?;
        }
        Ok(written)
    }

    fn write_line(&self, outfile: &mut fs::File, depth: usize, line: &str) -> Result<()> {
        outfile.write_all(format!("{}{}\n", ("    ").to_owned().repeat(depth), line).as_bytes())
    }

    /// Processes a set of modules and file descriptors, returning a map of modules to generated
    /// code contents.
    ///
    /// This is generally used when control over the output should not be managed by Prost,
    /// such as in a flow for a `protoc` code generating plugin. When compiling as part of a
    /// `build.rs` file, instead use [`compile_protos()`].
    pub fn generate(
        &mut self,
        requests: Vec<(Module, FileDescriptorProto)>,
    ) -> Result<HashMap<Module, String>> {
        let mut modules = HashMap::new();
        let mut packages = HashMap::new();

        let message_graph = MessageGraph::new(requests.iter().map(|x| &x.1))
            .map_err(|error| Error::new(ErrorKind::InvalidInput, error))?;
        let extern_paths = ExternPaths::new(&self.extern_paths, self.prost_types)
            .map_err(|error| Error::new(ErrorKind::InvalidInput, error))?;

        for (request_module, request_fd) in requests {
            // Only record packages that have services
            if !request_fd.service.is_empty() {
                packages.insert(request_module.clone(), request_fd.package().to_string());
            }
            let buf = modules
                .entry(request_module.clone())
                .or_insert_with(String::new);
            CodeGenerator::generate(self, &message_graph, &extern_paths, request_fd, buf);
            if buf.is_empty() {
                // Did not generate any code, remove from list to avoid inclusion in include file or output file list
                modules.remove(&request_module);
            }
        }

        if let Some(ref mut service_generator) = self.service_generator {
            for (module, package) in packages {
                let buf = modules.get_mut(&module).unwrap();
                service_generator.finalize_package(&package, buf);
            }
        }

        if self.fmt {
            self.fmt_modules(&mut modules);
        }

        Ok(modules)
    }

    #[cfg(feature = "format")]
    fn fmt_modules(&mut self, modules: &mut HashMap<Module, String>) {
        for buf in modules.values_mut() {
            let file = syn::parse_file(buf).unwrap();
            let formatted = prettyplease::unparse(&file);
            *buf = formatted;
        }
    }

    #[cfg(not(feature = "format"))]
    fn fmt_modules(&mut self, _: &mut HashMap<Module, String>) {}
}

impl default::Default for Config {
    fn default() -> Config {
        Config {
            file_descriptor_set_path: None,
            service_generator: None,
            map_type: PathMap::default(),
            bytes_type: PathMap::default(),
            type_attributes: PathMap::default(),
            field_attributes: PathMap::default(),
            prost_types: true,
            strip_enum_prefix: true,
            out_dir: None,
            extern_paths: Vec::new(),
            default_package_filename: "_".to_string(),
            protoc_args: Vec::new(),
            disable_comments: PathMap::default(),
            skip_protoc_run: false,
            include_file: None,
            prost_path: None,
            fmt: true,
        }
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Config")
            .field("file_descriptor_set_path", &self.file_descriptor_set_path)
            .field("service_generator", &self.service_generator.is_some())
            .field("map_type", &self.map_type)
            .field("bytes_type", &self.bytes_type)
            .field("type_attributes", &self.type_attributes)
            .field("field_attributes", &self.field_attributes)
            .field("prost_types", &self.prost_types)
            .field("strip_enum_prefix", &self.strip_enum_prefix)
            .field("out_dir", &self.out_dir)
            .field("extern_paths", &self.extern_paths)
            .field("default_package_filename", &self.default_package_filename)
            .field("protoc_args", &self.protoc_args)
            .field("disable_comments", &self.disable_comments)
            .field("prost_path", &self.prost_path)
            .finish()
    }
}

/// A Rust module path for a Protobuf package.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Module {
    components: Vec<String>,
}

impl Module {
    /// Construct a module path from an iterator of parts.
    pub fn from_parts<I>(parts: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        Self {
            components: parts.into_iter().map(|s| s.into()).collect(),
        }
    }

    /// Construct a module path from a Protobuf package name.
    ///
    /// Constituent parts are automatically converted to snake case in order to follow
    /// Rust module naming conventions.
    pub fn from_protobuf_package_name(name: &str) -> Self {
        Self {
            components: name
                .split('.')
                .filter(|s| !s.is_empty())
                .map(to_snake)
                .collect(),
        }
    }

    /// An iterator over the parts of the path.
    pub fn parts(&self) -> impl Iterator<Item = &str> {
        self.components.iter().map(|s| s.as_str())
    }

    /// Format the module path into a filename for generated Rust code.
    ///
    /// If the module path is empty, `default` is used to provide the root of the filename.
    pub fn to_file_name_or(&self, default: &str) -> String {
        let mut root = if self.components.is_empty() {
            default.to_owned()
        } else {
            self.components.join(".")
        };

        root.push_str(".rs");

        root
    }

    /// The number of parts in the module's path.
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Whether the module's path contains any components.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    fn to_partial_file_name(&self, range: RangeToInclusive<usize>) -> String {
        self.components[range].join(".")
    }

    fn part(&self, idx: usize) -> &str {
        self.components[idx].as_str()
    }
}

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut parts = self.parts();
        if let Some(first) = parts.next() {
            f.write_str(first)?;
        }
        for part in parts {
            f.write_str("::")?;
            f.write_str(part)?;
        }
        Ok(())
    }
}

/// Compile `.proto` files into Rust files during a Cargo build.
///
/// The generated `.rs` files are written to the Cargo `OUT_DIR` directory, suitable for use with
/// the [include!][1] macro. See the [Cargo `build.rs` code generation][2] example for more info.
///
/// This function should be called in a project's `build.rs`.
///
/// # Arguments
///
/// **`protos`** - Paths to `.proto` files to compile. Any transitively [imported][3] `.proto`
/// files are automatically be included.
///
/// **`includes`** - Paths to directories in which to search for imports. Directories are searched
/// in order. The `.proto` files passed in **`protos`** must be found in one of the provided
/// include directories.
///
/// # Errors
///
/// This function can fail for a number of reasons:
///
///   - Failure to locate or download `protoc`.
///   - Failure to parse the `.proto`s.
///   - Failure to locate an imported `.proto`.
///   - Failure to compile a `.proto` without a [package specifier][4].
///
/// It's expected that this function call be `unwrap`ed in a `build.rs`; there is typically no
/// reason to gracefully recover from errors during a build.
///
/// # Example `build.rs`
///
/// ```rust,no_run
/// # use std::io::Result;
/// fn main() -> Result<()> {
///   prost_build::compile_protos(&["src/frontend.proto", "src/backend.proto"], &["src"])?;
///   Ok(())
/// }
/// ```
///
/// [1]: https://doc.rust-lang.org/std/macro.include.html
/// [2]: http://doc.crates.io/build-script.html#case-study-code-generation
/// [3]: https://developers.google.com/protocol-buffers/docs/proto3#importing-definitions
/// [4]: https://developers.google.com/protocol-buffers/docs/proto#packages
pub fn compile_protos(protos: &[impl AsRef<Path>], includes: &[impl AsRef<Path>]) -> Result<()> {
    Config::new().compile_protos(protos, includes)
}

/// Returns the path to the `protoc` binary.
pub fn protoc_from_env() -> PathBuf {
    let os_specific_hint = if cfg!(target_os = "macos") {
        "You could try running `brew install protobuf` or downloading it from https://github.com/protocolbuffers/protobuf/releases"
    } else if cfg!(target_os = "linux") {
        "If you're on debian, try `apt-get install protobuf-compiler` or download it from https://github.com/protocolbuffers/protobuf/releases"
    } else {
        "You can download it from https://github.com/protocolbuffers/protobuf/releases or from your package manager."
    };
    let error_msg =
        "Could not find `protoc` installation and this build crate cannot proceed without
    this knowledge. If `protoc` is installed and this crate had trouble finding
    it, you can set the `PROTOC` environment variable with the specific path to your
    installed `protoc` binary.";
    let msg = format!(
        "{}{}

For more information: https://docs.rs/prost-build/#sourcing-protoc
",
        error_msg, os_specific_hint
    );

    env::var_os("PROTOC")
        .map(PathBuf::from)
        .or_else(|| which::which("protoc").ok())
        .expect(&msg)
}

/// Returns the path to the Protobuf include directory.
pub fn protoc_include_from_env() -> Option<PathBuf> {
    let protoc_include: PathBuf = env::var_os("PROTOC_INCLUDE")?.into();

    if !protoc_include.exists() {
        panic!(
            "PROTOC_INCLUDE environment variable points to non-existent directory ({:?})",
            protoc_include
        );
    }
    if !protoc_include.is_dir() {
        panic!(
            "PROTOC_INCLUDE environment variable points to a non-directory file ({:?})",
            protoc_include
        );
    }

    Some(protoc_include)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;
    use std::rc::Rc;

    use super::*;

    /// An example service generator that generates a trait with methods corresponding to the
    /// service methods.
    struct ServiceTraitGenerator;

    impl ServiceGenerator for ServiceTraitGenerator {
        fn generate(&mut self, service: Service, buf: &mut String) {
            // Generate a trait for the service.
            service.comments.append_with_indent(0, buf);
            buf.push_str(&format!("trait {} {{\n", &service.name));

            // Generate the service methods.
            for method in service.methods {
                method.comments.append_with_indent(1, buf);
                buf.push_str(&format!(
                    "    fn {}(_: {}) -> {};\n",
                    method.name, method.input_type, method.output_type
                ));
            }

            // Close out the trait.
            buf.push_str("}\n");
        }
        fn finalize(&mut self, buf: &mut String) {
            // Needs to be present only once, no matter how many services there are
            buf.push_str("pub mod utils { }\n");
        }
    }

    /// Implements `ServiceGenerator` and provides some state for assertions.
    struct MockServiceGenerator {
        state: Rc<RefCell<MockState>>,
    }

    /// Holds state for `MockServiceGenerator`
    #[derive(Default)]
    struct MockState {
        service_names: Vec<String>,
        package_names: Vec<String>,
        finalized: u32,
    }

    impl MockServiceGenerator {
        fn new(state: Rc<RefCell<MockState>>) -> Self {
            Self { state }
        }
    }

    impl ServiceGenerator for MockServiceGenerator {
        fn generate(&mut self, service: Service, _buf: &mut String) {
            let mut state = self.state.borrow_mut();
            state.service_names.push(service.name);
        }

        fn finalize(&mut self, _buf: &mut String) {
            let mut state = self.state.borrow_mut();
            state.finalized += 1;
        }

        fn finalize_package(&mut self, package: &str, _buf: &mut String) {
            let mut state = self.state.borrow_mut();
            state.package_names.push(package.to_string());
        }
    }

    #[test]
    fn smoke_test() {
        let _ = env_logger::try_init();
        Config::new()
            .service_generator(Box::new(ServiceTraitGenerator))
            .out_dir(std::env::temp_dir())
            .compile_protos(&["src/fixtures/smoke_test/smoke_test.proto"], &["src"])
            .unwrap();
    }

    #[test]
    fn finalize_package() {
        let _ = env_logger::try_init();

        let state = Rc::new(RefCell::new(MockState::default()));
        let gen = MockServiceGenerator::new(Rc::clone(&state));

        Config::new()
            .service_generator(Box::new(gen))
            .include_file("_protos.rs")
            .out_dir(std::env::temp_dir())
            .compile_protos(
                &[
                    "src/fixtures/helloworld/hello.proto",
                    "src/fixtures/helloworld/goodbye.proto",
                ],
                &["src/fixtures/helloworld"],
            )
            .unwrap();

        let state = state.borrow();
        assert_eq!(&state.service_names, &["Greeting", "Farewell"]);
        assert_eq!(&state.package_names, &["helloworld"]);
        assert_eq!(state.finalized, 3);
    }

    #[test]
    fn test_generate_no_empty_outputs() {
        let _ = env_logger::try_init();
        let state = Rc::new(RefCell::new(MockState::default()));
        let gen = MockServiceGenerator::new(Rc::clone(&state));
        let include_file = "_include.rs";
        let out_dir = std::env::temp_dir()
            .as_path()
            .join("test_generate_no_empty_outputs");
        let previously_empty_proto_path = out_dir.as_path().join(Path::new("google.protobuf.rs"));
        // For reproducibility, ensure we start with the out directory created and empty
        let _ = fs::remove_dir_all(&out_dir);
        let _ = fs::create_dir(&out_dir);

        Config::new()
            .service_generator(Box::new(gen))
            .include_file(include_file)
            .out_dir(&out_dir)
            .compile_protos(
                &["src/fixtures/imports_empty/imports_empty.proto"],
                &["src/fixtures/imports_empty"],
            )
            .unwrap();

        // Prior to PR introducing this test, the generated include file would have the file
        // google.protobuf.rs which was an empty file. Now that file should only exist if it has content
        if let Ok(mut f) = File::open(&previously_empty_proto_path) {
            // Since this file was generated, it should not be empty.
            let mut contents = String::new();
            f.read_to_string(&mut contents).unwrap();
            assert!(!contents.is_empty());
        } else {
            // The file wasn't generated so the result include file should not reference it
            let expected = read_all_content("src/fixtures/imports_empty/_expected_include.rs");
            let actual = read_all_content(
                out_dir
                    .as_path()
                    .join(Path::new(include_file))
                    .display()
                    .to_string()
                    .as_str(),
            );
            // Normalizes windows and Linux-style EOL
            let expected = expected.replace("\r\n", "\n");
            let actual = actual.replace("\r\n", "\n");
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn deterministic_include_file() {
        let _ = env_logger::try_init();

        for _ in 1..10 {
            let state = Rc::new(RefCell::new(MockState::default()));
            let gen = MockServiceGenerator::new(Rc::clone(&state));
            let include_file = "_include.rs";
            let tmp_dir = std::env::temp_dir();

            Config::new()
                .service_generator(Box::new(gen))
                .include_file(include_file)
                .out_dir(std::env::temp_dir())
                .compile_protos(
                    &[
                        "src/fixtures/alphabet/a.proto",
                        "src/fixtures/alphabet/b.proto",
                        "src/fixtures/alphabet/c.proto",
                        "src/fixtures/alphabet/d.proto",
                        "src/fixtures/alphabet/e.proto",
                        "src/fixtures/alphabet/f.proto",
                    ],
                    &["src/fixtures/alphabet"],
                )
                .unwrap();

            let expected = read_all_content("src/fixtures/alphabet/_expected_include.rs");
            let actual = read_all_content(
                tmp_dir
                    .as_path()
                    .join(Path::new(include_file))
                    .display()
                    .to_string()
                    .as_str(),
            );
            // Normalizes windows and Linux-style EOL
            let expected = expected.replace("\r\n", "\n");
            let actual = actual.replace("\r\n", "\n");

            assert_eq!(expected, actual);
        }
    }

    fn read_all_content(filepath: &str) -> String {
        let mut f = File::open(filepath).unwrap();
        let mut content = String::new();
        f.read_to_string(&mut content).unwrap();
        content
    }
}
