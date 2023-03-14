// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Contains the error type for this library.

#![allow(clippy::default_trait_access)]

use crate::schema::RoleType;
use crate::{schema, TargetName, TransportError};
use chrono::{DateTime, Utc};
use snafu::{Backtrace, Snafu};
use std::io;
use std::path::PathBuf;
use url::Url;

/// Alias for `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// The error type for this library.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum Error {
    #[snafu(display("Unable to canonicalize path '{}': {}", path.display(), source))]
    AbsolutePath {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "Failed to create temp directory for the repository datastore: {}",
        source
    ))]
    DatastoreInit {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// The library failed to create a file in the datastore.
    #[snafu(display("Failed to create file at datastore path {}: {}", path.display(), source))]
    DatastoreCreate {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// The library failed to open a file in the datastore.
    #[snafu(display("Failed to open file from datastore path {}: {}", path.display(), source))]
    DatastoreOpen {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// The library failed to remove a file in the datastore.
    #[snafu(display("Failed to remove file at datastore path {}: {}", path.display(), source))]
    DatastoreRemove {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// The library failed to serialize an object to JSON to the datastore.
    #[snafu(display("Failed to serialize {} to JSON at datastore path {}: {}", what, path.display(), source))]
    DatastoreSerialize {
        what: String,
        path: PathBuf,
        source: serde_json::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to create directory '{}': {}", path.display(), source))]
    DirCreate {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// A metadata file has expired.
    #[snafu(display("{} metadata is expired", role))]
    ExpiredMetadata {
        role: RoleType,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to stat '{}': {}", path.display(), source))]
    FileMetadata {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to open {}: {}", path.display(), source))]
    FileOpen {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to read {}: {}", path.display(), source))]
    FileRead {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to parse {}: {}", path.display(), source))]
    FileParseJson {
        path: PathBuf,
        source: serde_json::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Can't build URL from relative path '{}'", path.display()))]
    FileUrl { path: PathBuf, backtrace: Backtrace },

    #[snafu(display("Failed to write to {}: {}", path.display(), source))]
    FileWrite {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// A downloaded target's checksum does not match the checksum listed in the repository
    /// metadata.
    #[snafu(display(
        "Hash mismatch for {}: calculated {}, expected {}",
        context,
        calculated,
        expected,
    ))]
    HashMismatch {
        context: String,
        calculated: String,
        expected: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Source path for target must be file or symlink - '{}'", path.display()))]
    InvalidFileType { path: PathBuf, backtrace: Backtrace },

    #[snafu(display("Encountered an invalid target name: {}", inner))]
    InvalidTargetName { inner: String, backtrace: Backtrace },

    /// The library failed to create a URL from a base URL and a path.
    #[snafu(display("Failed to join \"{}\" to URL \"{}\": {}", path, url, source))]
    JoinUrl {
        path: String,
        url: url::Url,
        source: url::ParseError,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "After encoding the name '{}' to '{}', failed to join '{}' to URL '{}': {}",
        original,
        encoded,
        filename,
        url,
        source
    ))]
    JoinUrlEncoded {
        original: String,
        encoded: String,
        filename: String,
        url: url::Url,
        source: url::ParseError,
        backtrace: Backtrace,
    },

    #[snafu(display("Unable to parse keypair: {}", source))]
    KeyPairFromKeySource {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
        backtrace: Backtrace,
    },

    #[snafu(display("Private key rejected: {}", source))]
    KeyRejected {
        source: ring::error::KeyRejected,
        backtrace: Backtrace,
    },

    #[snafu(display("Unable to match any of the provided keys with root.json"))]
    KeysNotFoundInRoot { backtrace: Backtrace },

    #[snafu(display("Unrecognized private key format"))]
    KeyUnrecognized { backtrace: Backtrace },

    #[snafu(display("Failed to create symlink at '{}': {}", path.display(), source))]
    LinkCreate {
        path: PathBuf,
        source: io::Error,
        backtrace: Backtrace,
    },

    /// A file's maximum size exceeded a limit set by the consumer of this library or the metadata.
    #[snafu(display("Maximum size {} (specified by {}) exceeded", max_size, specifier))]
    MaxSizeExceeded {
        max_size: u64,
        specifier: &'static str,
        backtrace: Backtrace,
    },

    /// The maximum root updates setting was exceeded.
    #[snafu(display("Maximum root updates {} exceeded", max_root_updates))]
    MaxUpdatesExceeded {
        max_root_updates: u64,
        backtrace: Backtrace,
    },

    /// A required reference to a metadata file is missing from a metadata file.
    #[snafu(display("Meta for {:?} missing from {} metadata", file, role))]
    MetaMissing {
        file: &'static str,
        role: RoleType,
        backtrace: Backtrace,
    },

    #[snafu(display("Missing '{}' when building repo from RepositoryEditor", field))]
    Missing { field: String, backtrace: Backtrace },

    #[snafu(display("Unable to create NamedTempFile in directory '{}': {}", path.display(), source))]
    NamedTempFileCreate {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Unable to persist NamedTempFile to '{}': {}", path.display(), source))]
    NamedTempFilePersist {
        path: PathBuf,
        source: tempfile::PersistError,
        backtrace: Backtrace,
    },

    /// Unable to determine file name (path ends in '..' or is '/')
    #[snafu(display("Unable to determine file name from path: '{}'", path.display()))]
    NoFileName { path: PathBuf, backtrace: Backtrace },

    #[snafu(display("Key for role '{}' doesn't exist in root.json", role))]
    NoRoleKeysinRoot { role: String },

    /// A downloaded metadata file has an older version than a previously downloaded metadata file.
    #[snafu(display(
        "Found version {} of {} metadata when we had previously fetched version {}",
        new_version,
        role,
        current_version
    ))]
    OlderMetadata {
        role: RoleType,
        current_version: u64,
        new_version: u64,
        backtrace: Backtrace,
    },

    /// The library failed to parse a metadata file, either because it was not valid JSON or it did
    /// not conform to the expected schema.
    ///
    /// Invalid JSON errors read like:
    /// * EOF while parsing a string at line 1 column 14
    ///
    /// Schema non-conformance errors read like:
    /// * invalid type: integer `2`, expected a string at line 1 column 11
    /// * missing field `sig` at line 1 column 16
    #[snafu(display("Failed to parse {} metadata: {}", role, source))]
    ParseMetadata {
        role: RoleType,
        source: serde_json::Error,
        backtrace: Backtrace,
    },

    /// The library failed to parse the trusted root metadata file, either because it was not valid
    /// JSON or it did not conform to the expected schema. The *trusted* root metadata file is the
    /// file is either the `root` argument passed to `Repository::load`, or the most recently
    /// cached and validated root metadata file.
    #[snafu(display("Failed to parse trusted root metadata: {}", source))]
    ParseTrustedMetadata {
        source: serde_json::Error,
        backtrace: Backtrace,
    },

    /// Failed to parse a URL provided to [`Repository::load`][crate::Repository::load].
    #[snafu(display("Failed to parse URL {:?}: {}", url, source))]
    ParseUrl {
        url: String,
        source: url::ParseError,
        backtrace: Backtrace,
    },

    #[snafu(display("Target path exists, caller requested we fail - '{}'", path.display()))]
    PathExistsFail { path: PathBuf, backtrace: Backtrace },

    #[snafu(display("Requested copy/link of '{}' which is not a file", path.display()))]
    PathIsNotFile { path: PathBuf, backtrace: Backtrace },

    #[snafu(display("Requested copy/link of '{}' which is not a repo target", path.display()))]
    PathIsNotTarget { path: PathBuf, backtrace: Backtrace },

    /// Path isn't a valid UTF8 string
    #[snafu(display("Path {} is not valid UTF-8", path.display()))]
    PathUtf8 { path: PathBuf, backtrace: Backtrace },

    #[snafu(display("Failed to remove existing target path '{}': {}", path.display(), source))]
    RemoveTarget {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Unable to get info about the outdir '{}': {}", path.display(), source))]
    SaveTargetDirInfo {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("The outdir '{}' either does not exist or is not a directory", path.display()))]
    SaveTargetOutdir { path: PathBuf, backtrace: Backtrace },

    #[snafu(display("Unable to canonicalize the outdir '{}': {}", path.display(), source))]
    SaveTargetOutdirCanonicalize {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "The path '{}' to which we would save target '{}' has no parent",
        path.display(),
        name.raw(),
    ))]
    SaveTargetNoParent {
        path: PathBuf,
        name: TargetName,
        backtrace: Backtrace,
    },

    #[snafu(display("The target '{}' was not found", name.raw()))]
    SaveTargetNotFound {
        name: TargetName,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "The target '{}' had an unsafe name. Not writing to '{}' because it is not in the outdir '{}'",
        name.raw(),
        filepath.display(),
        outdir.display()
    ))]
    SaveTargetUnsafePath {
        name: TargetName,
        outdir: PathBuf,
        filepath: PathBuf,
    },

    #[snafu(display("Failed to serialize role '{}' for signing: {}", role, source))]
    SerializeRole {
        role: String,
        source: serde_json::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to serialize signed role '{}': {}", role, source))]
    SerializeSignedRole {
        role: String,
        source: serde_json::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to sign message"))]
    Sign {
        source: ring::error::Unspecified,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to sign message: {}", source))]
    SignMessage {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
        backtrace: Backtrace,
    },

    #[snafu(display("Unable to find signing keys for role '{}'", role))]
    SigningKeysNotFound { role: String },

    #[snafu(display(
        "Tried to use role metadata with spec version '{}', version '{}' is supported",
        given,
        supported
    ))]
    SpecVersion {
        given: String,
        supported: String,
        backtrace: Backtrace,
    },

    /// System time is behaving irrationally, went back in time
    #[snafu(display(
        "System time stepped backward: system time '{}', last known time '{}'",
        sys_time,
        latest_known_time,
    ))]
    SystemTimeSteppedBackward {
        sys_time: DateTime<Utc>,
        latest_known_time: DateTime<Utc>,
    },

    #[snafu(display("Refusing to replace {} with requested {} for target {}", found, expected, path.display()))]
    TargetFileTypeMismatch {
        expected: String,
        found: String,
        path: PathBuf,
        backtrace: Backtrace,
    },

    #[snafu(display("Unable to create Target from path '{}': {}", path.display(), source))]
    TargetFromPath {
        path: PathBuf,
        source: crate::schema::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Unable to resolve the target name '{}': {}", name, source))]
    TargetNameResolve {
        name: String,
        source: std::io::Error,
    },

    #[snafu(display(
        "Unable to resolve target name '{}', a path with no components was produced",
        name
    ))]
    TargetNameComponentsEmpty { name: String },

    #[snafu(display("Unable to resolve target name '{}', expected a rooted path", name))]
    TargetNameRootMissing { name: String },

    /// A transport error occurred while fetching a URL.
    #[snafu(display("Failed to fetch {}: {}", url, source))]
    Transport {
        url: url::Url,
        source: TransportError,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "The target name '..' is unsafe. Interpreting it as a path could escape from the intended \
        directory",
    ))]
    UnsafeTargetNameDotDot {},

    #[snafu(display(
        "The target name '{}' is unsafe. Interpreting it as a path would lead to an empty filename",
        name
    ))]
    UnsafeTargetNameEmpty { name: String },

    #[snafu(display(
        "The target name '{}' is unsafe. Interpreting it as a path would lead to a filename of '/'",
        name
    ))]
    UnsafeTargetNameSlash { name: String },

    /// A metadata file could not be verified.
    #[snafu(display("Failed to verify {} metadata: {}", role, source))]
    VerifyMetadata {
        role: RoleType,
        source: crate::schema::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to verify {} metadata: {}", role, source))]
    VerifyRoleMetadata {
        role: String,
        source: crate::schema::Error,
        backtrace: Backtrace,
    },

    /// The trusted root metadata file could not be verified.
    #[snafu(display("Failed to verify trusted root metadata: {}", source))]
    VerifyTrustedMetadata {
        source: crate::schema::Error,
        backtrace: Backtrace,
    },

    /// A fetched metadata file did not have the version we expected it to have.
    #[snafu(display(
        "{} metadata version mismatch: fetched {}, expected {}",
        role,
        fetched,
        expected
    ))]
    VersionMismatch {
        role: RoleType,
        fetched: u64,
        expected: u64,
        backtrace: Backtrace,
    },

    #[snafu(display("Error reading data from '{}': {}", url, source))]
    CacheFileRead {
        url: Url,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Error writing data to '{}': {}", path.display(), source))]
    CacheFileWrite {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Error creating the directory '{}': {}", path.display(), source))]
    CacheDirectoryCreate {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Error writing target file to '{}': {}", path.display(), source))]
    CacheTargetWrite {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("The target '{}' was not found", target_name.raw()))]
    CacheTargetMissing {
        target_name: TargetName,
        source: crate::schema::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to walk directory tree '{}': {}", directory.display(), source))]
    WalkDir {
        directory: PathBuf,
        source: walkdir::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Delegated role not found: {}", name))]
    DelegateNotFound { name: String },

    #[snafu(display("Targets role '{}' not found: {}", name, source))]
    TargetsNotFound {
        name: String,
        source: crate::schema::Error,
    },

    #[snafu(display("Delegated role not found: {}", name))]
    DelegateMissing {
        name: String,
        source: crate::schema::Error,
    },

    #[snafu(display("Delegation doesn't contain targets field"))]
    NoTargets,

    #[snafu(display("Targets doesn't contain delegations field"))]
    NoDelegations,

    #[snafu(display("Delegated roles are not consistent for {}", name))]
    DelegatedRolesNotConsistent { name: String },

    /// Target doesn't have proper permissions from parent delegations
    #[snafu(display("Invalid file permissions"))]
    InvalidPath { source: crate::schema::Error },

    #[snafu(display("Role missing from snapshot meta: {}", name))]
    RoleNotInMeta { name: String },

    #[snafu(display("The key for {} was not included", role))]
    KeyNotFound {
        role: String,
        source: schema::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("No keys were found for role '{}'", role))]
    NoKeys { role: String },

    #[snafu(display("Invalid number"))]
    InvalidInto {
        source: std::num::TryFromIntError,
        backtrace: Backtrace,
    },

    #[snafu(display("Invalid threshold number"))]
    InvalidThreshold { backtrace: Backtrace },

    /// The library failed to serialize an object to JSON.
    #[snafu(display("Failed to serialize to JSON: {}", source))]
    JsonSerialization {
        source: schema::Error,
        backtrace: Backtrace,
    },

    /// Invalid path permissions
    #[snafu(display("Invalid path permission of {} : {:?}", name, paths))]
    InvalidPathPermission {
        name: String,
        paths: Vec<String>,
        source: schema::Error,
    },

    /// SignedDelegatedTargets has more than 1 signed targets
    #[snafu(display("Exactly 1 role was required, but {} were created", count))]
    InvalidRoleCount { count: usize },

    /// Could not create a targets map
    #[snafu(display("Could not create a targets map: {}", source))]
    TargetsMap { source: schema::Error },

    /// A key_holder wasn't set
    #[snafu(display("A key holder must be set"))]
    NoKeyHolder,

    #[snafu(display("No limits in editor"))]
    MissingLimits,

    #[snafu(display("The transport is not in editor"))]
    MissingTransport,

    /// Root creates an unloadable repo
    #[snafu(display(
        "Unstable root; found {} keys for role {}, threshold is {}",
        role,
        actual,
        threshold
    ))]
    UnstableRoot {
        role: RoleType,
        actual: usize,
        threshold: u64,
    },

    #[snafu(display("The targets editor was not cleared"))]
    TargetsEditorSome,
}

// used in `std::io::Read` implementations
impl From<Error> for std::io::Error {
    fn from(err: Error) -> Self {
        Self::new(std::io::ErrorKind::Other, err)
    }
}
