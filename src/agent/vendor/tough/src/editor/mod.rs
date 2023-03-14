// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::used_underscore_binding)] // #20

//! Provides a `RepositoryEditor` object for building and editing TUF repositories.

mod keys;
pub mod signed;
pub mod targets;
mod test;

use crate::editor::signed::{SignedDelegatedTargets, SignedRepository, SignedRole};
use crate::editor::targets::TargetsEditor;
use crate::error::{self, Result};
use crate::fetch::fetch_max_size;
use crate::key_source::KeySource;
use crate::schema::decoded::{Decoded, Hex};
use crate::schema::key::Key;
use crate::schema::{
    Hashes, KeyHolder, PathSet, Role, RoleType, Root, Signed, Snapshot, SnapshotMeta, Target,
    Targets, Timestamp, TimestampMeta,
};
use crate::transport::Transport;
use crate::{encode_filename, Limits};
use crate::{Repository, TargetName};
use chrono::{DateTime, Utc};
use ring::digest::{SHA256, SHA256_OUTPUT_LEN};
use ring::rand::SystemRandom;
use serde_json::Value;
use snafu::{ensure, OptionExt, ResultExt};
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt::Display;
use std::num::NonZeroU64;
use std::path::Path;
use url::Url;

const SPEC_VERSION: &str = "1.0.0";

/// `RepositoryEditor` contains the various bits of data needed to construct
/// or edit a TUF repository.
///
/// A new repository may be started using the `new()` method.
///
/// An existing `tough::Repository` may be loaded and edited using the
/// `from_repo()` method. When a repo is loaded in this way, versions and
/// expirations are discarded. It is good practice to update these whenever
/// a repo is changed.
///
/// Targets, versions, and expirations may be added to their respective roles
/// via the provided "setter" methods. The final step in the process is the
/// `sign()` method, which takes a given set of signing keys, builds each of
/// the roles using the data provided, and signs the roles. This results in a
/// `SignedRepository` which can be used to write the repo to disk.
///
/// The following should only be used in a repository that utilizes delegated targets
/// `RepositoryEditor` uses a modal design to edit `Targets`. `TargetsEditor`
/// is used to perform all actions on a specified `Targets`. To change the
/// `Targets` being used call `change_delegated_targets()` to create a new `TargetsEditor`
/// for the specified role. To sign a `Targets` role from the `TargetsEditor` use `sign_targets_editor()`.
/// This will clear out the targets editor and insert the newly signed targets in `signed_targets`.
///
/// To update an existing targets from a metadata file use `update_delegated_targets()`.
///
/// To add a new role from metadata to the `Targets` in `TargetsEditor` use `add_role()`.
#[derive(Debug)]
pub struct RepositoryEditor {
    signed_root: SignedRole<Root>,

    snapshot_version: Option<NonZeroU64>,
    snapshot_expires: Option<DateTime<Utc>>,
    snapshot_extra: Option<HashMap<String, Value>>,

    timestamp_version: Option<NonZeroU64>,
    timestamp_expires: Option<DateTime<Utc>>,
    timestamp_extra: Option<HashMap<String, Value>>,

    targets_editor: Option<TargetsEditor>,

    /// The signed top level targets, will be None if no top level targets have been signed
    signed_targets: Option<Signed<Targets>>,

    transport: Option<Box<dyn Transport>>,
    limits: Option<Limits>,
}

impl RepositoryEditor {
    /// Create a new, bare `RepositoryEditor`
    pub fn new<P>(root_path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        // Read and parse the root.json. Without a good root, it doesn't
        // make sense to continue
        let root_path = root_path.as_ref();
        let root_buf =
            std::fs::read(root_path).context(error::FileReadSnafu { path: root_path })?;
        let root_buf_len = root_buf.len() as u64;
        let root = serde_json::from_slice::<Signed<Root>>(&root_buf)
            .context(error::FileParseJsonSnafu { path: root_path })?;

        // Quick check that root is signed by enough key IDs
        for (roletype, rolekeys) in &root.signed.roles {
            if rolekeys.threshold.get() > rolekeys.keyids.len() as u64 {
                return Err(error::Error::UnstableRoot {
                    role: *roletype,
                    threshold: rolekeys.threshold.get(),
                    actual: rolekeys.keyids.len(),
                });
            }
        }

        let mut digest = [0; SHA256_OUTPUT_LEN];
        digest.copy_from_slice(ring::digest::digest(&SHA256, &root_buf).as_ref());

        let signed_root = SignedRole {
            signed: root,
            buffer: root_buf,
            sha256: digest,
            length: root_buf_len,
        };

        let mut editor = TargetsEditor::new("targets");
        editor.key_holder = Some(KeyHolder::Root(signed_root.signed.signed.clone()));

        Ok(RepositoryEditor {
            signed_root,
            targets_editor: Some(editor),
            snapshot_version: None,
            snapshot_expires: None,
            snapshot_extra: None,
            timestamp_version: None,
            timestamp_expires: None,
            timestamp_extra: None,
            signed_targets: None,
            transport: None,
            limits: None,
        })
    }

    /// Given a `tough::Repository` and the path to a valid root.json, create a
    /// `RepositoryEditor`. This `RepositoryEditor` will include all of the targets
    /// and bits of _extra metadata from the roles included. It will not, however,
    /// include the versions or expirations and the user is expected to set them.
    pub fn from_repo<P>(root_path: P, repo: Repository) -> Result<RepositoryEditor>
    where
        P: AsRef<Path>,
    {
        let mut editor = RepositoryEditor::new(root_path)?;
        editor.targets(repo.targets)?;
        editor.snapshot(repo.snapshot.signed)?;
        editor.timestamp(repo.timestamp.signed)?;
        editor.transport = Some(repo.transport.clone());
        editor.limits = Some(repo.limits);
        Ok(editor)
    }

    /// Builds and signs each required role and returns a complete signed set
    /// of TUF repository metadata.
    ///
    /// While `RepositoryEditor`s fields are all `Option`s, this step requires,
    /// at the very least, that the "version" and "expiration" field is set for
    /// each role; e.g. `targets_version`, `targets_expires`, etc.
    pub fn sign(mut self, keys: &[Box<dyn KeySource>]) -> Result<SignedRepository> {
        let rng = SystemRandom::new();
        let root = KeyHolder::Root(self.signed_root.signed.signed.clone());
        // Sign the targets editor if able to with the provided keys
        self.sign_targets_editor(keys)?;
        let targets = self.signed_targets.clone().context(error::NoTargetsSnafu)?;
        let delegated_targets = targets.signed.signed_delegated_targets();
        let signed_targets = SignedRole::from_signed(targets)?;

        let signed_delegated_targets = if delegated_targets.is_empty() {
            // If we don't have any delegated targets, there is no reason to create
            // a `SignedDelegatedTargets`
            None
        } else {
            // If we have delegated targets
            let mut roles = Vec::new();
            for role in delegated_targets {
                // Create a `SignedRole<DelegatedTargets>` for each delegated targets
                roles.push(SignedRole::from_signed(role)?);
            }
            // SignedDelegatedTargets is a wrapper for a set of `SignedRole<DelegatedTargets>`
            Some(SignedDelegatedTargets {
                roles,
                consistent_snapshot: self.signed_root.signed.signed.consistent_snapshot,
            })
        };

        let signed_snapshot = self
            .build_snapshot(&signed_targets, &signed_delegated_targets)
            .and_then(|snapshot| SignedRole::new(snapshot, &root, keys, &rng))?;
        let signed_timestamp = self
            .build_timestamp(&signed_snapshot)
            .and_then(|timestamp| SignedRole::new(timestamp, &root, keys, &rng))?;

        // This validation can only be done from the top level targets.json role. This check verifies
        // that each target's delegate hierarchy is a match (i.e. its delegate ownership is valid).
        signed_targets
            .signed
            .signed
            .validate()
            .context(error::InvalidPathSnafu)?;

        Ok(SignedRepository {
            root: self.signed_root,
            targets: signed_targets,
            snapshot: signed_snapshot,
            timestamp: signed_timestamp,
            delegated_targets: signed_delegated_targets,
        })
    }

    /// Add an existing `Targets` struct to the repository.
    pub fn targets(&mut self, targets: Signed<Targets>) -> Result<&mut Self> {
        ensure!(
            targets.signed.spec_version == SPEC_VERSION,
            error::SpecVersionSnafu {
                given: targets.signed.spec_version,
                supported: SPEC_VERSION
            }
        );
        // Save the existing targets
        self.signed_targets = Some(targets.clone());
        // Create a targets editor so that targets can be updated
        self.targets_editor = Some(TargetsEditor::from_targets(
            "targets",
            targets.signed,
            KeyHolder::Root(self.signed_root.signed.signed.clone()),
        ));
        Ok(self)
    }

    /// Add an existing `Snapshot` to the repository. Only the `_extra` data
    /// is preserved
    pub fn snapshot(&mut self, snapshot: Snapshot) -> Result<&mut Self> {
        ensure!(
            snapshot.spec_version == SPEC_VERSION,
            error::SpecVersionSnafu {
                given: snapshot.spec_version,
                supported: SPEC_VERSION
            }
        );
        self.snapshot_extra = Some(snapshot._extra);
        Ok(self)
    }

    /// Add an existing `Timestamp` to the repository. Only the `_extra` data
    /// is preserved
    pub fn timestamp(&mut self, timestamp: Timestamp) -> Result<&mut Self> {
        ensure!(
            timestamp.spec_version == SPEC_VERSION,
            error::SpecVersionSnafu {
                given: timestamp.spec_version,
                supported: SPEC_VERSION
            }
        );
        self.timestamp_extra = Some(timestamp._extra);
        Ok(self)
    }

    /// Returns a mutable reference to the targets editor if it exists
    fn targets_editor_mut(&mut self) -> Result<&mut TargetsEditor> {
        self.targets_editor.as_mut().ok_or(error::Error::NoTargets)
    }

    /// Add a `Target` to the repository
    pub fn add_target<T, E>(&mut self, name: T, target: Target) -> Result<&mut Self>
    where
        T: TryInto<TargetName, Error = E>,
        E: Display,
    {
        self.targets_editor_mut()?.add_target(name, target)?;
        Ok(self)
    }

    /// Remove a `Target` from the repository
    pub fn remove_target(&mut self, name: &TargetName) -> Result<&mut Self> {
        self.targets_editor_mut()?.remove_target(name);

        Ok(self)
    }

    /// Add a target to the repository using its path
    ///
    /// Note: This function builds a `Target` synchronously;
    /// no multithreading or parallelism is used. If you have a large number
    /// of targets to add, and require advanced performance, you may want to
    /// construct `Target`s directly in parallel and use `add_target()`.
    pub fn add_target_path<P>(&mut self, target_path: P) -> Result<&mut Self>
    where
        P: AsRef<Path>,
    {
        let (target_name, target) = RepositoryEditor::build_target(target_path)?;
        self.add_target(target_name, target)?;
        Ok(self)
    }

    /// Add a list of target paths to the repository
    ///
    /// See the note on `add_target_path()` regarding performance.
    pub fn add_target_paths<P>(&mut self, targets: Vec<P>) -> Result<&mut Self>
    where
        P: AsRef<Path>,
    {
        for target in targets {
            let (target_name, target) = RepositoryEditor::build_target(target)?;
            self.add_target(target_name, target)?;
        }

        Ok(self)
    }

    /// Builds a target struct for the given path
    pub fn build_target<P>(target_path: P) -> Result<(TargetName, Target)>
    where
        P: AsRef<Path>,
    {
        let target_path = target_path.as_ref();

        // Get the file name as a string
        let target_name = TargetName::new(
            target_path
                .file_name()
                .context(error::NoFileNameSnafu { path: target_path })?
                .to_str()
                .context(error::PathUtf8Snafu { path: target_path })?,
        )?;

        // Build a Target from the path given. If it is not a file, this will fail
        let target = Target::from_path(target_path)
            .context(error::TargetFromPathSnafu { path: target_path })?;

        Ok((target_name, target))
    }

    /// Remove all targets from this repo
    pub fn clear_targets(&mut self) -> Result<&mut Self> {
        self.targets_editor_mut()?.clear_targets();
        Ok(self)
    }

    #[allow(clippy::too_many_arguments)]
    /// Delegate target with name as a `DelegatedRole` of the `Targets` in `targets_editor`
    /// This should be used if a role needs to be created by a user with `snapshot.json`,
    /// `timestamp.json`, and the new role's keys.
    pub fn delegate_role(
        &mut self,
        name: &str,
        key_source: &[Box<dyn KeySource>],
        paths: PathSet,
        threshold: NonZeroU64,
        expiration: DateTime<Utc>,
        version: NonZeroU64,
    ) -> Result<&mut Self> {
        // Create the new targets using targets editor
        let mut new_targets_editor = TargetsEditor::new(name);
        // Set the version and expiration
        new_targets_editor.version(version).expires(expiration);
        // Sign the new targets
        let new_targets = new_targets_editor.create_signed(key_source)?;
        // Find the keyids for key_source
        let mut keyids = Vec::new();
        let mut key_pairs = HashMap::new();
        for source in key_source {
            let key_pair = source
                .as_sign()
                .context(error::KeyPairFromKeySourceSnafu)?
                .tuf_key();
            keyids.push(
                key_pair
                    .key_id()
                    .context(error::JsonSerializationSnafu {})?,
            );
            key_pairs.insert(
                key_pair
                    .key_id()
                    .context(error::JsonSerializationSnafu {})?,
                key_pair,
            );
        }
        // Add the new role to targets_editor
        self.targets_editor_mut()?.delegate_role(
            new_targets,
            paths,
            key_pairs,
            keyids,
            threshold,
        )?;

        Ok(self)
    }

    /// Set the `Snapshot` version
    pub fn snapshot_version(&mut self, snapshot_version: NonZeroU64) -> &mut Self {
        self.snapshot_version = Some(snapshot_version);
        self
    }

    /// Set the `Snapshot` expiration
    pub fn snapshot_expires(&mut self, snapshot_expires: DateTime<Utc>) -> &mut Self {
        self.snapshot_expires = Some(snapshot_expires);
        self
    }

    /// Set the `Targets` version
    pub fn targets_version(&mut self, targets_version: NonZeroU64) -> Result<&mut Self> {
        self.targets_editor_mut()?.version(targets_version);
        Ok(self)
    }

    /// Set the `Targets` expiration
    pub fn targets_expires(&mut self, targets_expires: DateTime<Utc>) -> Result<&mut Self> {
        self.targets_editor_mut()?.expires(targets_expires);
        Ok(self)
    }

    /// Set the `Timestamp` version
    pub fn timestamp_version(&mut self, timestamp_version: NonZeroU64) -> &mut Self {
        self.timestamp_version = Some(timestamp_version);
        self
    }

    /// Set the `Timestamp` expiration
    pub fn timestamp_expires(&mut self, timestamp_expires: DateTime<Utc>) -> &mut Self {
        self.timestamp_expires = Some(timestamp_expires);
        self
    }

    /// Takes the current Targets from `targets_editor` and inserts the role to its proper place in `signed_targets`
    /// Sets `targets_editor` to None
    /// Must be called before `change_delegated_targets()`
    pub fn sign_targets_editor(&mut self, keys: &[Box<dyn KeySource>]) -> Result<&mut Self> {
        if let Some(targets_editor) = self.targets_editor.as_mut() {
            let (name, targets) = targets_editor.create_signed(keys)?.targets();
            if name == "targets" {
                self.signed_targets = Some(targets);
            } else {
                self.signed_targets
                    .as_mut()
                    .context(error::NoTargetsSnafu)?
                    .signed
                    .delegated_role_mut(&name)
                    .context(error::DelegateMissingSnafu { name })?
                    .targets = Some(targets);
            }
        }
        self.targets_editor = None;
        Ok(self)
    }

    /// Changes the targets refered to in `targets_editor` to role
    /// All `Targets` related calls will now be called on the `Targets` role named `role`
    /// Throws error if the `targets_editor` was not cleared using `sign_targets_editor()`
    /// Clones the desired targets from `signed_targets` and creates a `TargetsEditor` for it
    pub fn change_delegated_targets(&mut self, role: &str) -> Result<&mut Self> {
        if self.targets_editor.is_some() {
            return Err(error::Error::TargetsEditorSome);
        }
        let targets = &mut self
            .signed_targets
            .as_mut()
            .context(error::NoTargetsSnafu)?
            .signed;
        let (key_holder, targets) = if role == "targets" {
            (
                KeyHolder::Root(self.signed_root.signed.signed.clone()),
                targets.clone(),
            )
        } else {
            let parent = targets
                .parent_of(role)
                .context(error::DelegateMissingSnafu {
                    name: role.to_string(),
                })?
                .clone();
            let targets = targets
                .delegated_targets(role)
                .context(error::DelegateMissingSnafu {
                    name: role.to_string(),
                })?
                .clone();
            (KeyHolder::Delegations(parent), targets.signed)
        };
        self.targets_editor = Some(TargetsEditor::from_targets(role, targets, key_holder));

        Ok(self)
    }

    #[allow(clippy::too_many_lines)]
    /// Updates the metadata for the `Targets` role named `name`
    /// This method is used to load in a `Targets` metadata file located at
    /// `metadata_url` and update the repository's metadata for the role
    /// This method uses the result of `SignedDelegatedTargets::write()`
    /// Clears the current `targets_editor`
    pub fn update_delegated_targets(
        &mut self,
        name: &str,
        metadata_url: &str,
    ) -> Result<&mut Self> {
        let limits = self.limits.context(error::MissingLimitsSnafu)?;
        let transport = self
            .transport
            .as_ref()
            .context(error::MissingTransportSnafu)?;
        let targets = &mut self
            .signed_targets
            .as_mut()
            .context(error::NoTargetsSnafu)?
            .signed;
        let metadata_base_url = parse_url(metadata_url)?;
        // path to updated metadata
        let encoded_name = encode_filename(name);
        let encoded_filename = format!("{}.json", encoded_name);
        let role_url = metadata_base_url
            .join(&encoded_filename)
            .with_context(|_| error::JoinUrlEncodedSnafu {
                original: name,
                encoded: encoded_name,
                filename: encoded_filename,
                url: metadata_base_url.clone(),
            })?;
        let reader = Box::new(fetch_max_size(
            transport.as_ref(),
            role_url,
            limits.max_targets_size,
            "max targets limit",
        )?);
        // Load incoming role metadata as Signed<Targets>
        let mut role: Signed<crate::schema::Targets> =
            serde_json::from_reader(reader).context(error::ParseMetadataSnafu {
                role: RoleType::Targets,
            })?;
        //verify role with the parent delegation
        let (parent, current_targets) = if name == "targets" {
            (
                KeyHolder::Root(self.signed_root.signed.signed.clone()),
                targets,
            )
        } else {
            let parent = targets
                .parent_of(name)
                .context(error::DelegateMissingSnafu {
                    name: name.to_string(),
                })?
                .clone();
            let targets =
                targets
                    .delegated_targets_mut(name)
                    .context(error::DelegateMissingSnafu {
                        name: name.to_string(),
                    })?;
            (KeyHolder::Delegations(parent), &mut targets.signed)
        };
        parent.verify_role(&role, name)?;
        // Make sure the version isn't downgraded
        ensure!(
            role.signed.version >= current_targets.version,
            error::VersionMismatchSnafu {
                role: RoleType::Targets,
                fetched: role.signed.version,
                expected: current_targets.version
            }
        );
        // get a list of roles that we don't have metadata for yet
        // and copy current_targets delegated targets to role
        let new_roles = current_targets.update_targets(&mut role);
        let delegations = role
            .signed
            .delegations
            .as_mut()
            .context(error::NoDelegationsSnafu)?;
        // the new targets will be the keyholder for any of its newly delegated roles, so create a keyholder
        let key_holder = KeyHolder::Delegations(delegations.clone());
        // load the new roles
        for name in new_roles {
            // path to new metadata
            let encoded_name = encode_filename(&name);
            let encoded_filename = format!("{}.json", encoded_name);
            let role_url = metadata_base_url
                .join(&encoded_filename)
                .with_context(|_| error::JoinUrlEncodedSnafu {
                    original: &name,
                    encoded: encoded_name,
                    filename: encoded_filename,
                    url: metadata_base_url.clone(),
                })?;
            let reader = Box::new(fetch_max_size(
                transport.as_ref(),
                role_url,
                limits.max_targets_size,
                "max targets limit",
            )?);
            // Load new role metadata as Signed<Targets>
            let new_role: Signed<crate::schema::Targets> = serde_json::from_reader(reader)
                .context(error::ParseMetadataSnafu {
                    role: RoleType::Targets,
                })?;
            // verify the role
            key_holder.verify_role(&new_role, &name)?;
            // add the new role
            delegations
                .roles
                .iter_mut()
                .find(|delegated_role| delegated_role.name == name)
                .context(error::DelegateNotFoundSnafu { name: name.clone() })?
                .targets = Some(new_role.clone());
        }
        // Add our new role in place of the old one
        if name == "targets" {
            self.signed_targets = Some(role);
        } else {
            self.signed_targets
                .as_mut()
                .context(error::NoTargetsSnafu)?
                .signed
                .delegated_role_mut(name)
                .context(error::DelegateMissingSnafu {
                    name: name.to_string(),
                })?
                .targets = Some(role);
        }
        self.targets_editor = None;
        Ok(self)
    }

    /// Adds a role to the targets currently in `targets_editor`
    /// using a metadata file located at `metadata_url`/`name`.json
    /// `add_role()` uses `TargetsEditor::add_role()` to add a role from an existing metadata file.
    pub fn add_role(
        &mut self,
        name: &str,
        metadata_url: &str,
        paths: PathSet,
        threshold: NonZeroU64,
        keys: Option<HashMap<Decoded<Hex>, Key>>,
    ) -> Result<&mut Self> {
        let limits = self.limits.context(error::MissingLimitsSnafu)?;
        let transport = self
            .transport
            .as_ref()
            .context(error::MissingTransportSnafu)?
            .clone();
        self.targets_editor_mut()?.limits(limits);
        self.targets_editor_mut()?.transport(transport.clone());
        self.targets_editor_mut()?
            .add_role(name, metadata_url, paths, threshold, keys)?;

        Ok(self)
    }

    // =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

    /// Build the `Snapshot` struct
    fn build_snapshot(
        &self,
        signed_targets: &SignedRole<Targets>,
        signed_delegated_targets: &Option<SignedDelegatedTargets>,
    ) -> Result<Snapshot> {
        let version = self.snapshot_version.context(error::MissingSnafu {
            field: "snapshot version",
        })?;
        let expires = self.snapshot_expires.context(error::MissingSnafu {
            field: "snapshot expiration",
        })?;
        let _extra = self.snapshot_extra.clone().unwrap_or_default();

        let mut snapshot = Snapshot::new(SPEC_VERSION.to_string(), version, expires);

        // Snapshot stores metadata about targets and root
        let targets_meta = Self::snapshot_meta(signed_targets);
        snapshot
            .meta
            .insert("targets.json".to_owned(), targets_meta);

        if let Some(signed_delegated_targets) = signed_delegated_targets.as_ref() {
            for delegated_targets in &signed_delegated_targets.roles {
                let meta = Self::snapshot_meta(delegated_targets);
                snapshot.meta.insert(
                    format!("{}.json", delegated_targets.signed.signed.name),
                    meta,
                );
            }
        }

        Ok(snapshot)
    }

    /// Build a `SnapshotMeta` struct from a given `SignedRole<R>`. This metadata
    /// includes the sha256 and length of the signed role.
    fn snapshot_meta<R>(role: &SignedRole<R>) -> SnapshotMeta
    where
        R: Role,
    {
        SnapshotMeta {
            hashes: Some(Hashes {
                sha256: role.sha256.to_vec().into(),
                _extra: HashMap::new(),
            }),
            length: Some(role.length),
            version: role.signed.signed.version(),
            _extra: HashMap::new(),
        }
    }

    /// Build the `Timestamp` struct
    fn build_timestamp(&self, signed_snapshot: &SignedRole<Snapshot>) -> Result<Timestamp> {
        let version = self.timestamp_version.context(error::MissingSnafu {
            field: "timestamp version",
        })?;
        let expires = self.timestamp_expires.context(error::MissingSnafu {
            field: "timestamp expiration",
        })?;
        let _extra = self.timestamp_extra.clone().unwrap_or_default();
        let mut timestamp = Timestamp::new(SPEC_VERSION.to_string(), version, expires);

        // Timestamp stores metadata about snapshot
        let snapshot_meta = Self::timestamp_meta(signed_snapshot);
        timestamp
            .meta
            .insert("snapshot.json".to_owned(), snapshot_meta);
        timestamp._extra = _extra;

        Ok(timestamp)
    }

    /// Build a `TimestampMeta` struct from a given `SignedRole<R>`. This metadata
    /// includes the sha256 and length of the signed role.
    fn timestamp_meta<R>(role: &SignedRole<R>) -> TimestampMeta
    where
        R: Role,
    {
        TimestampMeta {
            hashes: Hashes {
                sha256: role.sha256.to_vec().into(),
                _extra: HashMap::new(),
            },
            length: role.length,
            version: role.signed.signed.version(),
            _extra: HashMap::new(),
        }
    }
}

fn parse_url(url: &str) -> Result<Url> {
    let mut url = Cow::from(url);
    if !url.ends_with('/') {
        url.to_mut().push('/');
    }
    Url::parse(&url).context(error::ParseUrlSnafu { url })
}
