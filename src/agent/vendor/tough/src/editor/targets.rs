// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::used_underscore_binding)] // #20

//! Provides a `TargetsEditor` object for building and editing targets roles.

use crate::editor::signed::{SignedDelegatedTargets, SignedRole};
use crate::error::{self, Result};
use crate::fetch::fetch_max_size;
use crate::key_source::KeySource;
use crate::schema::decoded::{Decoded, Hex};
use crate::schema::key::Key;
use crate::schema::{
    DelegatedRole, DelegatedTargets, Delegations, KeyHolder, PathSet, RoleType, Signed, Target,
    Targets,
};
use crate::transport::Transport;
use crate::{encode_filename, Limits};
use crate::{Repository, TargetName};
use chrono::{DateTime, Utc};
use ring::rand::SystemRandom;
use serde_json::Value;
use snafu::{OptionExt, ResultExt};
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt::Display;
use std::num::NonZeroU64;
use std::path::Path;
use url::Url;

const SPEC_VERSION: &str = "1.0.0";

/// If you are not working with a repository that utilizes delegated targets, use the `RepositoryEditor`.
///
/// `TargetsEditor` contains the various bits of data needed to construct
/// or edit a `Targets` role.
///
/// `TargetsEditor` should only be used with repositories that utilize delegated targets.
/// `TargetsEditor` cannot create a `SignedRepository`. Whenever a user has access to `snapshot.json` and `timestamp.json` keys,
/// `RepositoryEditor` should be used so that a `SignedRepository` can be created.
///
/// A new Targets may be created using the `new()` method.
///
/// An existing `Targets` may be loaded and edited using the
/// `from_targets()` method. When a targets is loaded in this way, versions and
/// expirations are discarded. It is good practice to update these whenever
/// a repo is changed.
///
/// A  `Targets` from an existing repository can be loaded using the `from_repo()` method.
/// `Targets` loaded this way will have the versions and expirations removed, but the
/// proper keyholder to sign the targets and the `Transport` used to load the repo will be saved.
///
/// Targets, versions, and expirations may be added to their respective roles
/// via the provided "setter" methods. The final step in the process is the
/// `sign()` method, which takes a given set of signing keys, builds each of
/// the roles using the data provided, and signs the roles. This results in a
/// `SignedDelegatedTargets` which can be used to write the updated metadata to disk.
#[derive(Debug, Clone)]
pub struct TargetsEditor {
    /// The name of the targets role
    name: String,
    /// The metadata containing keyids for the role
    pub(crate) key_holder: Option<KeyHolder>,
    /// The delegations field of the Targets metadata
    /// delegations should only be None if the editor is
    /// for "targets" on a repository that doesn't use delegated targets
    delegations: Option<Delegations>,
    /// New targets that were added to `name`
    new_targets: Option<HashMap<TargetName, Target>>,
    /// Targets that were previously in `name`
    existing_targets: Option<HashMap<TargetName, Target>>,
    /// Version of the `Targets`
    version: Option<NonZeroU64>,
    /// Expiration of the `Targets`
    expires: Option<DateTime<Utc>>,
    /// New roles that were created with the editor
    new_roles: Option<Vec<DelegatedRole>>,

    _extra: Option<HashMap<String, Value>>,

    limits: Option<Limits>,

    transport: Option<Box<dyn Transport>>,
}

impl TargetsEditor {
    /// Creates a `TargetsEditor` for a newly created role
    pub fn new(name: &str) -> Self {
        TargetsEditor {
            key_holder: None,
            delegations: Some(Delegations::new()),
            new_targets: None,
            existing_targets: None,
            version: None,
            expires: None,
            name: name.to_string(),
            new_roles: None,
            _extra: None,
            limits: None,
            transport: None,
        }
    }

    /// Creates a `TargetsEditor` with the provided targets and keyholder
    /// `version` and `expires` are thrown out to encourage updating the version and expiration
    pub fn from_targets(name: &str, targets: Targets, key_holder: KeyHolder) -> Self {
        TargetsEditor {
            key_holder: Some(key_holder),
            delegations: targets.delegations,
            new_targets: None,
            existing_targets: Some(targets.targets),
            version: None,
            expires: None,
            name: name.to_string(),
            new_roles: None,
            _extra: Some(targets._extra),
            limits: None,
            transport: None,
        }
    }

    /// Creates a `TargetsEditor` with the provided targets from an already loaded repo
    /// `version` and `expires` are thrown out to encourage updating the version and expiration
    /// If a `Repository` has been loaded, use `from_repo()` to preserve the `Transport` and `Limits`.
    pub fn from_repo(repo: Repository, name: &str) -> Result<Self> {
        let (targets, key_holder) = if name == "targets" {
            (
                repo.targets.signed.clone(),
                KeyHolder::Root(repo.root.signed.clone()),
            )
        } else {
            let targets = repo
                .delegated_role(name)
                .context(error::DelegateNotFoundSnafu {
                    name: name.to_string(),
                })?
                .targets
                .as_ref()
                .context(error::NoTargetsSnafu)?
                .signed
                .clone();
            let key_holder = KeyHolder::Delegations(
                repo.targets
                    .signed
                    .parent_of(name)
                    .context(error::DelegateMissingSnafu {
                        name: name.to_string(),
                    })?
                    .clone(),
            );
            (targets, key_holder)
        };
        Ok(TargetsEditor {
            key_holder: Some(key_holder),
            delegations: targets.delegations,
            new_targets: None,
            existing_targets: Some(targets.targets),
            version: None,
            expires: None,
            name: name.to_string(),
            new_roles: None,
            _extra: Some(targets._extra),
            limits: Some(repo.limits),
            transport: Some(repo.transport),
        })
    }

    /// Adds limits to the `TargetsEditor`, only necessary if loading a role
    pub fn limits(&mut self, limits: Limits) {
        self.limits = Some(limits);
    }

    /// Add a transport to the `TargetsEditor`, only necessary if loading a role
    pub fn transport(&mut self, transport: Box<dyn Transport>) {
        self.transport = Some(transport);
    }

    /// Add a `Target` to the `Targets` role
    pub fn add_target<T, E>(&mut self, name: T, target: Target) -> Result<&mut Self>
    where
        T: TryInto<TargetName, Error = E>,
        E: Display,
    {
        let target_name = name.try_into().map_err(|e| {
            error::InvalidTargetNameSnafu {
                inner: e.to_string(),
            }
            .build()
        })?;
        self.new_targets
            .get_or_insert_with(HashMap::new)
            .insert(target_name, target);
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

        self.add_target(target_name, target)?;
        Ok(self)
    }

    /// Add a list of target paths to the targets
    ///
    /// See the note on `add_target_path()` regarding performance.
    pub fn add_target_paths<P>(&mut self, targets: Vec<P>) -> Result<&mut Self>
    where
        P: AsRef<Path>,
    {
        for target in targets {
            self.add_target_path(target)?;
        }
        Ok(self)
    }

    /// Remove a `Target` from the targets if it exists
    pub fn remove_target(&mut self, name: &TargetName) -> &mut Self {
        if let Some(targets) = self.existing_targets.as_mut() {
            targets.remove(name);
        }
        if let Some(targets) = self.new_targets.as_mut() {
            targets.remove(name);
        }

        self
    }

    /// Remove all targets from this role
    pub fn clear_targets(&mut self) -> &mut Self {
        self.existing_targets
            .get_or_insert_with(HashMap::new)
            .clear();
        self.new_targets.get_or_insert_with(HashMap::new).clear();
        self
    }

    /// Set the version
    pub fn version(&mut self, version: NonZeroU64) -> &mut Self {
        self.version = Some(version);
        self
    }

    /// Set the expiration
    pub fn expires(&mut self, expires: DateTime<Utc>) -> &mut Self {
        self.expires = Some(expires);
        self
    }

    /// Adds a key to delegations keyids, adds the key to `role` if it is provided
    pub fn add_key(
        &mut self,
        keys: HashMap<Decoded<Hex>, Key>,
        role: Option<&str>,
    ) -> Result<&mut Self> {
        let delegations = self
            .delegations
            .as_mut()
            .context(error::NoDelegationsSnafu)?;
        let mut keyids = Vec::new();
        for (keyid, key) in keys {
            // Check to see if the key is present
            if !delegations
                .keys
                .values()
                .any(|candidate_key| key == *candidate_key)
            {
                // Key isn't present yet, so we need to add it
                delegations.keys.insert(keyid.clone(), key);
            };
            keyids.push(keyid.clone());
        }

        // If a role was provided add keyids to the delegated role
        if let Some(role) = role {
            for delegated_role in &mut delegations.roles {
                if delegated_role.name == role {
                    delegated_role.keyids.extend(keyids.clone());
                }
            }
            for delegated_role in self.new_roles.get_or_insert(Vec::new()).iter_mut() {
                if delegated_role.name == role {
                    delegated_role.keyids.extend(keyids.clone());
                }
            }
        }
        Ok(self)
    }

    /// Removes a key from delegations keyids, if a role is specified the key is only removed from the role
    pub fn remove_key(&mut self, keyid: &Decoded<Hex>, role: Option<&str>) -> Result<&mut Self> {
        let delegations = self
            .delegations
            .as_mut()
            .context(error::NoDelegationsSnafu)?;
        // If a role was provided remove keyid from the delegated role
        if let Some(role) = role {
            for delegated_role in &mut delegations.roles {
                if delegated_role.name == role {
                    delegated_role.keyids.retain(|key| keyid != key);
                }
            }
        } else {
            delegations.keys.remove(keyid);
        }
        Ok(self)
    }

    /// Adds a `DelegatedRole` to `new_roles`
    /// To use `delegate_role()` a new `Targets` should be created using `TargetsEditor::new()`
    /// followed by `create_signed()` to provide a `Signed<DelegatedTargets>` for the new role.
    pub fn delegate_role(
        &mut self,
        targets: Signed<DelegatedTargets>,
        paths: PathSet,
        key_pairs: HashMap<Decoded<Hex>, Key>,
        keyids: Vec<Decoded<Hex>>,
        threshold: NonZeroU64,
    ) -> Result<&mut Self> {
        self.add_key(key_pairs, None)?;
        self.new_roles
            .get_or_insert(Vec::new())
            .push(DelegatedRole {
                name: targets.signed.name,
                paths,
                keyids,
                threshold,
                terminating: false,
                targets: Some(Signed {
                    signed: targets.signed.targets,
                    signatures: targets.signatures,
                }),
            });
        Ok(self)
    }

    /// Removes a role from delegations
    /// If `recursive` is `false`, `role` is only removed if it is directly delegated by this role
    /// If `true` removes whichever role eventually delegates 'role'
    pub fn remove_role(&mut self, role: &str, recursive: bool) -> Result<&mut Self> {
        let delegations = self
            .delegations
            .as_mut()
            .context(error::NoDelegationsSnafu)?;
        // Keep all of the roles that are not `role`
        delegations
            .roles
            .retain(|delegated_role| delegated_role.name != role);
        if recursive {
            // Keep all roles that do not delegate `role` down the chain of delegations
            delegations.roles.retain(|delegated_role| {
                delegated_role
                    .targets
                    .as_ref()
                    .map_or(true, |targets| targets.signed.delegated_role(role).is_err())
            });
        }
        Ok(self)
    }

    /// Adds a role to `new_roles` using a metadata file located at `metadata_url`/`name`.json
    /// `add_role()` uses `delegate_role()` to add a role from an existing metadata file.
    pub fn add_role(
        &mut self,
        name: &str,
        metadata_url: &str,
        paths: PathSet,
        threshold: NonZeroU64,
        keys: Option<HashMap<Decoded<Hex>, Key>>,
    ) -> Result<&mut Self> {
        let limits = self.limits.context(error::MissingLimitsSnafu)?;
        let transport: &dyn Transport = self
            .transport
            .as_ref()
            .context(error::MissingTransportSnafu)?
            .as_ref();

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
                url: metadata_base_url,
            })?;
        let reader = Box::new(fetch_max_size(
            transport,
            role_url,
            limits.max_targets_size,
            "max targets limit",
        )?);
        // Load incoming role metadata as Signed<Targets>
        let role: Signed<crate::schema::Targets> =
            serde_json::from_reader(reader).context(error::ParseMetadataSnafu {
                role: RoleType::Targets,
            })?;

        // Create `Signed<DelegatedTargets>` for the role
        let delegated_targets = Signed {
            signed: DelegatedTargets {
                name: name.to_string(),
                targets: role.signed.clone(),
            },
            signatures: role.signatures.clone(),
        };
        let (keyids, key_pairs) = if let Some(keys) = keys {
            (keys.keys().cloned().collect(), keys)
        } else {
            let key_pairs = role
                .signed
                .delegations
                .context(error::NoDelegationsSnafu)?
                .keys;
            (key_pairs.keys().cloned().collect(), key_pairs)
        };

        self.delegate_role(delegated_targets, paths, key_pairs, keyids, threshold)?;

        Ok(self)
    }

    /// Build the `Targets` struct
    /// Adds in the new roles and new targets
    pub fn build_targets(&self) -> Result<DelegatedTargets> {
        let version = self.version.context(error::MissingSnafu {
            field: "targets version",
        })?;
        let expires = self.expires.context(error::MissingSnafu {
            field: "targets expiration",
        })?;

        // BEWARE!!! We are allowing targets to be empty! While this isn't
        // the most common use case, it's possible this is what a user wants.
        // If it's important to have a non-empty targets, the object can be
        // inspected by the calling code.
        let mut targets: HashMap<TargetName, Target> = HashMap::new();
        if let Some(ref existing_targets) = self.existing_targets {
            targets.extend(existing_targets.clone());
        }
        if let Some(ref new_targets) = self.new_targets {
            targets.extend(new_targets.clone());
        }

        let mut delegations = self.delegations.clone();
        if let Some(delegations) = delegations.as_mut() {
            if let Some(new_roles) = self.new_roles.as_ref() {
                delegations.roles.extend(new_roles.clone());
            }
        }

        let _extra = self._extra.clone().unwrap_or_default();
        Ok(DelegatedTargets {
            name: self.name.clone(),
            targets: Targets {
                spec_version: SPEC_VERSION.to_string(),
                version,
                expires,
                targets,
                _extra,
                delegations,
            },
        })
    }

    /// Creates a `KeyHolder` to sign the `Targets` role with the signing keys provided
    fn create_key_holder(&self, keys: &[Box<dyn KeySource>]) -> Result<KeyHolder> {
        // There isn't a KeyHolder, so create one based on the provided keys
        let mut delegations = Delegations::new();
        // First create the tuf key pairs and keyids
        let mut keyids = Vec::new();
        let mut key_pairs = HashMap::new();
        for source in keys {
            let key_pair = source
                .as_sign()
                .context(error::KeyPairFromKeySourceSnafu)?
                .tuf_key();
            key_pairs.insert(
                key_pair
                    .key_id()
                    .context(error::JsonSerializationSnafu {})?
                    .clone(),
                key_pair.clone(),
            );
            keyids.push(
                key_pair
                    .key_id()
                    .context(error::JsonSerializationSnafu {})?
                    .clone(),
            );
        }
        // Then add the keys to the new delegations keys
        delegations.keys = key_pairs;
        // Now create a DelegatedRole for the new role
        delegations.roles.push(DelegatedRole {
            name: self.name.clone(),
            threshold: NonZeroU64::new(1).unwrap(),
            paths: PathSet::Paths([].to_vec()),
            terminating: false,
            keyids,
            targets: None,
        });
        Ok(KeyHolder::Delegations(delegations))
    }

    /// Creates a `Signed<DelegatedTargets>` for only this role using the provided keys
    /// This is used to create a `Signed<DelegatedTargets>` for the role instead of a `SignedDelegatedTargets`
    /// like `sign()` creates. `SignedDelegatedTargets` can contain more than 1 `Signed<DelegatedTargets>`
    /// `create_signed()` guarantees that only 1 `Signed<DelegatedTargets>` is created and that it is the one representing
    /// the current targets. `create_signed()` should be used whenever the result of `TargetsEditor` is not being written.
    pub fn create_signed(&self, keys: &[Box<dyn KeySource>]) -> Result<Signed<DelegatedTargets>> {
        let rng = SystemRandom::new();
        let key_holder = if let Some(key_holder) = self.key_holder.as_ref() {
            key_holder.clone()
        } else {
            self.create_key_holder(keys)?
        };
        // create a signed role for the targets being edited
        let targets = self
            .build_targets()
            .and_then(|targets| SignedRole::new(targets, &key_holder, keys, &rng))?;
        Ok(targets.signed)
    }

    /// Creates a `SignedDelegatedTargets` for the Targets role being edited and all added roles
    /// If `key_holder` was not assigned then this is a newly created role and needs to be signed with a
    /// custom delegations as its `key_holder`
    pub fn sign(&self, keys: &[Box<dyn KeySource>]) -> Result<SignedDelegatedTargets> {
        let rng = SystemRandom::new();
        let mut roles = Vec::new();
        let key_holder = if let Some(key_holder) = self.key_holder.as_ref() {
            key_holder.clone()
        } else {
            self.create_key_holder(keys)?
        };

        // create a signed role for the targets we are editing
        let signed_targets = self
            .build_targets()
            .and_then(|targets| SignedRole::new(targets, &key_holder, keys, &rng))?;
        roles.push(signed_targets);
        // create signed roles for any role metadata we added to this targets
        if let Some(new_roles) = &self.new_roles {
            for role in new_roles {
                roles.push(SignedRole::from_signed(
                    role.clone()
                        .targets
                        .context(error::NoTargetsSnafu)?
                        .delegated_targets(&role.name),
                )?);
            }
        }

        Ok(SignedDelegatedTargets {
            roles,
            consistent_snapshot: false,
        })
    }
}

fn parse_url(url: &str) -> Result<Url> {
    let mut url = Cow::from(url);
    if !url.ends_with('/') {
        url.to_mut().push('/');
    }
    Url::parse(&url).context(error::ParseUrlSnafu { url })
}
