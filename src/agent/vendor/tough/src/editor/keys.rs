// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::error::{self, Result};
use crate::key_source::KeySource;
use crate::schema::decoded::{Decoded, Hex};
use crate::schema::{Delegations, KeyHolder, RoleId, RoleKeys, Root, Signed, Targets};
use crate::sign::Sign;
use snafu::{ensure, OptionExt, ResultExt};
use std::collections::HashMap;

/// A map of key ID (from root.json or the Delegations field of any Targets) to its corresponding signing key
pub(crate) type KeyList = HashMap<Decoded<Hex>, Box<dyn Sign>>;

impl KeyHolder {
    /// Creates a key list for the provided keys
    pub(crate) fn get_keys(&self, keys: &[Box<dyn KeySource>]) -> Result<KeyList> {
        match self {
            Self::Delegations(delegations) => get_targets_keys(delegations, keys),
            Self::Root(root) => get_root_keys(root, keys),
        }
    }

    /// Returns role keys for the provided role id
    pub(crate) fn role_keys(&self, name: RoleId) -> Result<RoleKeys> {
        match self {
            Self::Delegations(delegations) => {
                if let RoleId::DelegatedRole(name) = name.clone() {
                    for role in &delegations.roles {
                        if role.name == name.clone() {
                            return Ok(role.keys());
                        }
                    }
                }
            }
            Self::Root(root) => {
                if let RoleId::StandardRole(roletype) = name {
                    return Ok(root
                        .roles
                        .get(&roletype)
                        .context(error::NoRoleKeysinRootSnafu {
                            role: roletype.to_string(),
                        })?
                        .clone());
                }
            }
        }
        let role = match name {
            RoleId::StandardRole(role) => role.to_string(),
            RoleId::DelegatedRole(role_name) => role_name,
        };
        Err(error::Error::SigningKeysNotFound { role })
    }

    /// Verifies the role using `KeyHolder`'s keys
    pub(crate) fn verify_role(&self, targets: &Signed<Targets>, name: &str) -> Result<()> {
        match self {
            Self::Delegations(delegations) => {
                delegations
                    .verify_role(targets, name)
                    .context(error::VerifyRoleMetadataSnafu {
                        role: name.to_string(),
                    })
            }
            Self::Root(root) => root
                .verify_role(targets)
                .context(error::VerifyRoleMetadataSnafu {
                    role: name.to_string(),
                }),
        }
    }
}

/// Gets the corresponding keys from Root (root.json) for the given `KeySource`s.
/// This is a convenience function that wraps `Root.key_id()` for multiple
/// `KeySource`s.
pub(crate) fn get_root_keys(root: &Root, keys: &[Box<dyn KeySource>]) -> Result<KeyList> {
    let mut root_keys = KeyList::new();

    for source in keys {
        // Get a keypair from the given source
        let key_pair = source.as_sign().context(error::KeyPairFromKeySourceSnafu)?;

        // If the keypair matches any of the keys in the root.json,
        // add its ID and corresponding keypair the map to be returned
        if let Some(key_id) = root.key_id(key_pair.as_ref()) {
            root_keys.insert(key_id, key_pair);
        }
    }
    ensure!(!root_keys.is_empty(), error::KeysNotFoundInRootSnafu);
    Ok(root_keys)
}

/// Gets the corresponding keys from delegations for the given `KeySource`s.
/// This is a convenience function that wraps `Delegations.key_id()` for multiple
/// `KeySource`s.
pub(crate) fn get_targets_keys(
    delegations: &Delegations,
    keys: &[Box<dyn KeySource>],
) -> Result<KeyList> {
    let mut delegations_keys = KeyList::new();
    for source in keys {
        // Get a keypair from the given source
        let key_pair = source.as_sign().context(error::KeyPairFromKeySourceSnafu)?;
        // If the keypair matches any of the keys in the delegations metadata,
        // add its ID and corresponding keypair the map to be returned
        if let Some(key_id) = delegations.key_id(key_pair.as_ref()) {
            delegations_keys.insert(key_id, key_pair);
        }
    }
    Ok(delegations_keys)
}
