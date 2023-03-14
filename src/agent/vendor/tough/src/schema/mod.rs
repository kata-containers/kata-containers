#![allow(clippy::used_underscore_binding)] // #20

//! Provides the schema objects as defined by the TUF spec.

mod de;
pub mod decoded;
mod error;
mod iter;
pub mod key;
mod spki;
mod verify;

use crate::schema::decoded::{Decoded, Hex};
pub use crate::schema::error::{Error, Result};
use crate::schema::iter::KeysIter;
use crate::schema::key::Key;
use crate::sign::Sign;
pub use crate::transport::{FilesystemTransport, Transport};
use crate::{encode_filename, TargetName};
use chrono::{DateTime, Utc};
use globset::{Glob, GlobMatcher};
use hex::ToHex;
use olpc_cjson::CanonicalFormatter;
use ring::digest::{digest, Context, SHA256};
use serde::de::Error as SerdeDeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use serde_plain::{derive_display_from_serialize, derive_fromstr_from_deserialize};
use snafu::ResultExt;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::num::NonZeroU64;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::str::FromStr;

/// The type of metadata role.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum RoleType {
    /// The root role delegates trust to specific keys trusted for all other top-level roles used in
    /// the system.
    Root,
    /// The snapshot role signs a metadata file that provides information about the latest version
    /// of all targets metadata on the repository (the top-level targets role and all delegated
    /// roles).
    Snapshot,
    /// The targets role's signature indicates which target files are trusted by clients.
    Targets,
    /// The timestamp role is used to prevent an adversary from replaying an out-of-date signed
    /// metadata file whose signature has not yet expired.
    Timestamp,
    /// A delegated targets role
    DelegatedTargets,
}

derive_display_from_serialize!(RoleType);
derive_fromstr_from_deserialize!(RoleType);

/// A role identifier
#[derive(Debug, Clone)]
pub enum RoleId {
    /// Top level roles are identified by a RoleType
    StandardRole(RoleType),
    /// A delegated role is identified by a String
    DelegatedRole(String),
}

/// Common trait implemented by all roles.
pub trait Role: Serialize {
    /// The type of role this object represents.
    const TYPE: RoleType;

    /// Determines when metadata should be considered expired and no longer trusted by clients.
    fn expires(&self) -> DateTime<Utc>;

    /// An integer that is greater than 0. Clients MUST NOT replace a metadata file with a version
    /// number less than the one currently trusted.
    fn version(&self) -> NonZeroU64;

    /// The filename that the role metadata should be written to
    fn filename(&self, consistent_snapshot: bool) -> String;

    /// The `RoleId` corresponding to the role
    fn role_id(&self) -> RoleId {
        RoleId::StandardRole(Self::TYPE)
    }

    /// A deterministic JSON serialization used when calculating the digest of a metadata object.
    /// [More info on canonical JSON](http://wiki.laptop.org/go/Canonical_JSON)
    fn canonical_form(&self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut data, CanonicalFormatter::new());
        self.serialize(&mut ser)
            .context(error::JsonSerializationSnafu { what: "role" })?;
        Ok(data)
    }
}

/// A signed metadata object.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct Signed<T> {
    /// The role that is signed.
    pub signed: T,
    /// A list of signatures and their key IDs.
    pub signatures: Vec<Signature>,
}

/// A signature and the key ID that made it.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct Signature {
    /// The key ID (listed in root.json) that made this signature.
    pub keyid: Decoded<Hex>,
    /// A hex-encoded signature of the canonical JSON form of a role.
    pub sig: Decoded<Hex>,
}

/// A `KeyHolder` is metadata that is responsible for verifying the signatures of a role.
/// `KeyHolder` contains either a `Delegations` of a `Targets` or a `Root`
#[derive(Debug, Clone)]
pub enum KeyHolder {
    /// Delegations verify delegated targets
    Delegations(Delegations),
    /// Root verifies the top level targets, snapshot, timestamp, and root
    Root(Root),
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

/// TUF 4.3: The root.json file is signed by the root role's keys. It indicates which keys are
/// authorized for all top-level roles, including the root role itself. Revocation and replacement
/// of top-level role keys, including for the root role, is done by changing the keys listed for the
/// roles in this file.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(tag = "_type")]
#[serde(rename = "root")]
pub struct Root {
    /// A string that contains the version number of the TUF specification. Its format follows the
    /// Semantic Versioning 2.0.0 (semver) specification.
    pub spec_version: String,

    /// A boolean indicating whether the repository supports consistent snapshots. When consistent
    /// snapshots is `true`, targets and certain metadata filenames are prefixed with either a
    /// a version number or digest.
    pub consistent_snapshot: bool,

    /// An integer that is greater than 0. Clients MUST NOT replace a metadata file with a version
    /// number less than the one currently trusted.
    pub version: NonZeroU64,

    /// Determines when metadata should be considered expired and no longer trusted by clients.
    pub expires: DateTime<Utc>,

    /// The KEYID must be correct for the specified KEY. Clients MUST calculate each KEYID to verify
    /// this is correct for the associated key. Clients MUST ensure that for any KEYID represented
    /// in this key list and in other files, only one unique key has that KEYID.
    #[serde(deserialize_with = "de::deserialize_keys")]
    pub keys: HashMap<Decoded<Hex>, Key>,

    /// A list of roles, the keys associated with each role, and the threshold of signatures used
    /// for each role.
    pub roles: HashMap<RoleType, RoleKeys>,

    /// Extra arguments found during deserialization.
    ///
    /// We must store these to correctly verify signatures for this object.
    ///
    /// If you're instantiating this struct, you should make this `HashMap::empty()`.
    #[serde(flatten)]
    #[serde(deserialize_with = "de::extra_skip_type")]
    pub _extra: HashMap<String, Value>,
}

/// Represents the key IDs used for a role and the threshold of signatures required to validate it.
/// TUF 4.3: A ROLE is one of "root", "snapshot", "targets", "timestamp", or "mirrors". A role for
/// each of "root", "snapshot", "timestamp", and "targets" MUST be specified in the key list.
/// The role of "mirror" is optional. If not specified, the mirror list will not need to be signed
/// if mirror lists are being used. The THRESHOLD for a role is an integer of the number of keys of
/// that role whose signatures are required in order to consider a file as being properly signed by
/// that role.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct RoleKeys {
    /// The key IDs used for the role.
    pub keyids: Vec<Decoded<Hex>>,

    /// The threshold of signatures required to validate the role.
    pub threshold: NonZeroU64,

    /// Extra arguments found during deserialization.
    ///
    /// We must store these to correctly verify signatures for this object.
    ///
    /// If you're instantiating this struct, you should make this `HashMap::empty()`.
    #[serde(flatten)]
    pub _extra: HashMap<String, Value>,
}

impl Root {
    /// An iterator over the keys for a given role.
    pub fn keys(&self, role: RoleType) -> impl Iterator<Item = &Key> {
        KeysIter {
            keyids_iter: match self.roles.get(&role) {
                Some(role_keys) => role_keys.keyids.iter(),
                None => [].iter(),
            },
            keys: &self.keys,
        }
    }

    /// Given an object/key that impls Sign, return the corresponding
    /// key ID from Root
    pub fn key_id(&self, key_pair: &dyn Sign) -> Option<Decoded<Hex>> {
        for (key_id, key) in &self.keys {
            if key_pair.tuf_key() == *key {
                return Some(key_id.clone());
            }
        }
        None
    }
}

impl Role for Root {
    const TYPE: RoleType = RoleType::Root;

    fn expires(&self) -> DateTime<Utc> {
        self.expires
    }

    fn version(&self) -> NonZeroU64 {
        self.version
    }

    fn filename(&self, _consistent_snapshot: bool) -> String {
        format!("{}.root.json", self.version())
    }
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

/// TUF 4.4 The snapshot.json file is signed by the snapshot role. It MUST list the version numbers
/// of the top-level targets metadata and all delegated targets metadata. It MAY also list their
/// lengths and file hashes.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(tag = "_type")]
#[serde(rename = "snapshot")]
pub struct Snapshot {
    /// A string that contains the version number of the TUF specification. Its format follows the
    /// Semantic Versioning 2.0.0 (semver) specification.
    pub spec_version: String,

    /// An integer that is greater than 0. Clients MUST NOT replace a metadata file with a version
    /// number less than the one currently trusted.
    pub version: NonZeroU64,

    /// Determines when metadata should be considered expired and no longer trusted by clients.
    pub expires: DateTime<Utc>,

    /// A list of what the TUF spec calls 'METAFILES' (`SnapshotMeta` objects). The TUF spec
    /// describes the hash key in 4.4: METAPATH is the file path of the metadata on the repository
    /// relative to the metadata base URL. For snapshot.json, these are top-level targets metadata
    /// and delegated targets metadata.
    pub meta: HashMap<String, SnapshotMeta>,

    /// Extra arguments found during deserialization.
    ///
    /// We must store these to correctly verify signatures for this object.
    ///
    /// If you're instantiating this struct, you should make this `HashMap::empty()`.
    #[serde(flatten)]
    #[serde(deserialize_with = "de::extra_skip_type")]
    pub _extra: HashMap<String, Value>,
}

/// Represents a metadata file in a `snapshot.json` file.
/// TUF 4.4: METAFILES is an object whose format is the following:
/// ```text
///  { METAPATH : {
///        "version" : VERSION,
///        ("length" : LENGTH, |
///         "hashes" : HASHES) }
///    , ...
///  }
/// ```
/// e.g.
/// ```json
///    "project1.json": {
///     "version": 1,
///     "hashes": {
///      "sha256": "f592d072e1193688a686267e8e10d7257b4ebfcf28133350dae88362d82a0c8a"
///     }
///    },
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct SnapshotMeta {
    /// LENGTH is the integer length in bytes of the metadata file at METAPATH. It is OPTIONAL and
    /// can be omitted to reduce the snapshot metadata file size. In that case the client MUST use a
    /// custom download limit for the listed metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<u64>,

    /// HASHES is a dictionary that specifies one or more hashes of the metadata file at METAPATH,
    /// including their cryptographic hash function. For example: `{ "sha256": HASH, ... }`. HASHES
    /// is OPTIONAL and can be omitted to reduce the snapshot metadata file size. In that case the
    /// repository MUST guarantee that VERSION alone unambiguously identifies the metadata at
    /// METAPATH.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hashes: Option<Hashes>,

    /// An integer that is greater than 0. Clients MUST NOT replace a metadata file with a version
    /// number less than the one currently trusted.
    pub version: NonZeroU64,

    /// Extra arguments found during deserialization.
    ///
    /// We must store these to correctly verify signatures for this object.
    ///
    /// If you're instantiating this struct, you should make this `HashMap::empty()`.
    #[serde(flatten)]
    pub _extra: HashMap<String, Value>,
}

/// Represents the hash dictionary in a `snapshot.json` file.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct Hashes {
    /// The SHA 256 digest of a metadata file.
    pub sha256: Decoded<Hex>,

    /// Extra arguments found during deserialization.
    ///
    /// We must store these to correctly verify signatures for this object.
    ///
    /// If you're instantiating this struct, you should make this `HashMap::empty()`.
    #[serde(flatten)]
    pub _extra: HashMap<String, Value>,
}

impl Snapshot {
    /// Create a new `Snapshot` object.
    pub fn new(spec_version: String, version: NonZeroU64, expires: DateTime<Utc>) -> Self {
        Snapshot {
            spec_version,
            version,
            expires,
            meta: HashMap::new(),
            _extra: HashMap::new(),
        }
    }
}
impl Role for Snapshot {
    const TYPE: RoleType = RoleType::Snapshot;

    fn expires(&self) -> DateTime<Utc> {
        self.expires
    }

    fn version(&self) -> NonZeroU64 {
        self.version
    }

    fn filename(&self, consistent_snapshot: bool) -> String {
        if consistent_snapshot {
            format!("{}.snapshot.json", self.version())
        } else {
            "snapshot.json".to_string()
        }
    }
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

/// Represents a `targets.json` file.
/// TUF 4.5:
/// The "signed" portion of targets.json is as follows:
/// ```text
/// { "_type" : "targets",
///   "spec_version" : SPEC_VERSION,
///   "version" : VERSION,
///   "expires" : EXPIRES,
///   "targets" : TARGETS,
///   ("delegations" : DELEGATIONS)
/// }
/// ```
///
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "_type")]
#[serde(rename = "targets")]
pub struct Targets {
    /// A string that contains the version number of the TUF specification. Its format follows the
    /// Semantic Versioning 2.0.0 (semver) specification.
    pub spec_version: String,

    /// An integer that is greater than 0. Clients MUST NOT replace a metadata file with a version
    /// number less than the one currently trusted.
    pub version: NonZeroU64,

    /// Determines when metadata should be considered expired and no longer trusted by clients.
    pub expires: DateTime<Utc>,

    /// Each key of the TARGETS object is a TARGETPATH. A TARGETPATH is a path to a file that is
    /// relative to a mirror's base URL of targets.
    pub targets: HashMap<TargetName, Target>,

    /// Delegations describes subsets of the targets for which responsibility is delegated to
    /// another role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegations: Option<Delegations>,

    /// Extra arguments found during deserialization.
    ///
    /// We must store these to correctly verify signatures for this object.
    ///
    /// If you're instantiating this struct, you should make this `HashMap::empty()`.
    #[serde(flatten)]
    #[serde(deserialize_with = "de::extra_skip_type")]
    pub _extra: HashMap<String, Value>,
}

/// TUF 4.5: TARGETS is an object whose format is the following:
/// ```text
/// { TARGETPATH : {
///       "length" : LENGTH,
///       "hashes" : HASHES,
///       ("custom" : { ... }) }
///   , ...
/// }
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct Target {
    /// LENGTH is the integer length in bytes of the target file at TARGETPATH.
    pub length: u64,

    /// HASHES is a dictionary that specifies one or more hashes, including the cryptographic hash
    /// function. For example: `{ "sha256": HASH, ... }`. HASH is the hexdigest of the cryptographic
    /// function computed on the target file.
    pub hashes: Hashes,

    /// If defined, the elements and values of "custom" will be made available to the client
    /// application. The information in "custom" is opaque to the framework and can include version
    /// numbers, dependencies, requirements, and any other data that the application wants to
    /// include to describe the file at TARGETPATH. The application may use this information to
    /// guide download decisions.
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub custom: HashMap<String, Value>,

    /// Extra arguments found during deserialization.
    ///
    /// We must store these to correctly verify signatures for this object.
    ///
    /// If you're instantiating this struct, you should make this `HashMap::empty()`.
    #[serde(flatten)]
    pub _extra: HashMap<String, Value>,
}

impl Target {
    /// Given a path, returns a Target struct
    pub fn from_path<P>(path: P) -> Result<Target>
    where
        P: AsRef<Path>,
    {
        // Ensure the given path is a file
        let path = path.as_ref();
        if !path.is_file() {
            return error::TargetNotAFileSnafu { path }.fail();
        }

        // Get the sha256 and length of the target
        let mut file = File::open(path).context(error::FileOpenSnafu { path })?;
        let mut digest = Context::new(&SHA256);
        let mut buf = [0; 8 * 1024];
        let mut length = 0;
        loop {
            match file.read(&mut buf).context(error::FileReadSnafu { path })? {
                0 => break,
                n => {
                    digest.update(&buf[..n]);
                    length += n as u64;
                }
            }
        }

        Ok(Target {
            length,
            hashes: Hashes {
                sha256: Decoded::from(digest.finish().as_ref().to_vec()),
                _extra: HashMap::new(),
            },
            custom: HashMap::new(),
            _extra: HashMap::new(),
        })
    }
}

impl Targets {
    /// Create a new `Targets` object.
    pub fn new(spec_version: String, version: NonZeroU64, expires: DateTime<Utc>) -> Self {
        Targets {
            spec_version,
            version,
            expires,
            targets: HashMap::new(),
            _extra: HashMap::new(),
            delegations: Some(Delegations::new()),
        }
    }

    /// Given a target url, returns a reference to the Target struct or error if the target is
    /// unreachable.
    ///
    /// **Caution**: does not imply that delegations in this struct or any child are valid.
    ///
    pub fn find_target(&self, target_name: &TargetName) -> Result<&Target> {
        if let Some(target) = self.targets.get(target_name) {
            return Ok(target);
        }
        if let Some(delegations) = &self.delegations {
            for role in &delegations.roles {
                // If the target cannot match this DelegatedRole, then we do not want to recurse and
                // check any of its child roles either.
                if !role.paths.matches_target_name(target_name) {
                    continue;
                }
                if let Some(targets) = &role.targets {
                    if let Ok(target) = targets.signed.find_target(target_name) {
                        return Ok(target);
                    }
                }
            }
        }
        error::TargetNotFoundSnafu {
            name: target_name.clone(),
        }
        .fail()
    }

    /// Returns a hashmap of all targets and all delegated targets recursively
    pub fn targets_map(&self) -> HashMap<TargetName, &Target> {
        self.targets_iter()
            .map(|(target_name, target)| (target_name.clone(), target))
            .collect()
    }

    /// Returns an iterator of all targets and all delegated targets recursively
    pub fn targets_iter(&self) -> impl Iterator<Item = (&TargetName, &Target)> + '_ {
        let mut iter: Box<dyn Iterator<Item = (&TargetName, &Target)>> =
            Box::new(self.targets.iter());
        if let Some(delegations) = &self.delegations {
            for role in &delegations.roles {
                if let Some(targets) = &role.targets {
                    iter = Box::new(iter.chain(targets.signed.targets_iter()));
                }
            }
        }
        iter
    }

    /// Recursively clears all targets
    pub fn clear_targets(&mut self) {
        self.targets = HashMap::new();
        if let Some(delegations) = &mut self.delegations {
            for delegated_role in &mut delegations.roles {
                if let Some(targets) = &mut delegated_role.targets {
                    targets.signed.clear_targets();
                }
            }
        }
    }

    /// Add a target to targets
    pub fn add_target(&mut self, name: TargetName, target: Target) {
        self.targets.insert(name, target);
    }

    /// Remove a target from targets
    pub fn remove_target(&mut self, name: &TargetName) -> Option<Target> {
        self.targets.remove(name)
    }

    /// Returns the `&Signed<Targets>` for `name`
    pub fn delegated_targets(&self, name: &str) -> Result<&Signed<Targets>> {
        self.delegated_role(name)?
            .targets
            .as_ref()
            .ok_or(error::Error::NoTargets)
    }

    /// Returns a mutable `Signed<Targets>` for `name`
    pub fn delegated_targets_mut(&mut self, name: &str) -> Result<&mut Signed<Targets>> {
        self.delegated_role_mut(name)?
            .targets
            .as_mut()
            .ok_or(error::Error::NoTargets)
    }

    /// Returns the `&DelegatedRole` for `name`
    pub fn delegated_role(&self, name: &str) -> Result<&DelegatedRole> {
        for role in &self
            .delegations
            .as_ref()
            .ok_or(error::Error::NoDelegations)?
            .roles
        {
            if role.name == name {
                return Ok(role);
            } else if let Ok(role) = role
                .targets
                .as_ref()
                .ok_or(error::Error::NoTargets)?
                .signed
                .delegated_role(name)
            {
                return Ok(role);
            }
        }
        Err(error::Error::RoleNotFound {
            name: name.to_string(),
        })
    }

    /// Returns a mutable `DelegatedRole` for `name`
    pub fn delegated_role_mut(&mut self, name: &str) -> Result<&mut DelegatedRole> {
        for role in &mut self
            .delegations
            .as_mut()
            .ok_or(error::Error::NoDelegations)?
            .roles
        {
            if role.name == name {
                return Ok(role);
            } else if let Ok(role) = role
                .targets
                .as_mut()
                .ok_or(error::Error::NoTargets)?
                .signed
                .delegated_role_mut(name)
            {
                return Ok(role);
            }
        }
        Err(error::Error::RoleNotFound {
            name: name.to_string(),
        })
    }

    ///Returns a vec of all rolenames
    pub fn role_names(&self) -> Vec<&String> {
        let mut roles = Vec::new();
        if let Some(delelegations) = &self.delegations {
            for role in &delelegations.roles {
                roles.push(&role.name);
                if let Some(targets) = &role.targets {
                    roles.append(&mut targets.signed.role_names());
                }
            }
        }

        roles
    }

    /// Returns a reference to the parent delegation of `name`
    pub fn parent_of(&self, name: &str) -> Result<&Delegations> {
        if let Some(delegations) = &self.delegations {
            for role in &delegations.roles {
                if role.name == name {
                    return Ok(delegations);
                }
                if let Some(targets) = &role.targets {
                    if let Ok(delegation) = targets.signed.parent_of(name) {
                        return Ok(delegation);
                    }
                }
            }
        }
        Err(error::Error::RoleNotFound {
            name: name.to_string(),
        })
    }

    /// Returns a vec of all targets roles delegated by this role
    pub fn signed_delegated_targets(&self) -> Vec<Signed<DelegatedTargets>> {
        let mut delegated_targets = Vec::new();
        if let Some(delegations) = &self.delegations {
            for role in &delegations.roles {
                if let Some(targets) = &role.targets {
                    delegated_targets.push(targets.clone().delegated_targets(&role.name));
                    delegated_targets.extend(targets.signed.signed_delegated_targets());
                }
            }
        }
        delegated_targets
    }

    /// Link all current targets to `new_targets` metadata, returns a list of new `Targets` not included in the original `Targets`' delegated roles
    /// This is used to insert a set of updated `Targets` metadata without reloading the rest of the chain.
    pub fn update_targets(&self, new_targets: &mut Signed<Targets>) -> Vec<String> {
        let mut needed_roles = Vec::new();
        // Copy existing targets into proper places of new_targets
        if let Some(delegations) = &mut new_targets.signed.delegations {
            for mut role in &mut delegations.roles {
                // Check to see if `role.name` has already been loaded
                if let Ok(targets) = self.delegated_targets(&role.name) {
                    // If it has been loaded, use it as the targets for the role
                    role.targets = Some(targets.clone());
                } else {
                    // If not make sure we keep track that it needs to be loaded
                    needed_roles.push(role.name.clone());
                }
            }
        }

        needed_roles
    }

    /// Calls `find_target` on each target (recursively provided by `targets_iter`). This
    /// proves that the target is either owned by us, or correctly matches through some hierarchy of
    /// [`PathSets`] below us. When called on the top level [`Targets`] of a repository, this proves
    /// that the ownership of each target is valid.
    pub(crate) fn validate(&self) -> Result<()> {
        for (target_name, _) in self.targets_iter() {
            self.find_target(target_name)?;
        }
        Ok(())
    }
}

impl Role for Targets {
    const TYPE: RoleType = RoleType::Targets;

    fn expires(&self) -> DateTime<Utc> {
        self.expires
    }

    fn version(&self) -> NonZeroU64 {
        self.version
    }

    fn filename(&self, consistent_snapshot: bool) -> String {
        if consistent_snapshot {
            format!("{}.targets.json", self.version())
        } else {
            "targets.json".to_string()
        }
    }
}

/// Wrapper for `Targets` so that a `Targets` role can be given a name
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DelegatedTargets {
    /// The name of the role
    #[serde(skip)]
    pub name: String,
    /// The targets representing the role metadata
    #[serde(flatten)]
    pub targets: Targets,
}

impl Deref for DelegatedTargets {
    type Target = Targets;

    fn deref(&self) -> &Targets {
        &self.targets
    }
}

impl DerefMut for DelegatedTargets {
    fn deref_mut(&mut self) -> &mut Targets {
        &mut self.targets
    }
}

impl Role for DelegatedTargets {
    const TYPE: RoleType = RoleType::DelegatedTargets;

    fn expires(&self) -> DateTime<Utc> {
        self.targets.expires
    }

    fn version(&self) -> NonZeroU64 {
        self.targets.version
    }

    fn filename(&self, consistent_snapshot: bool) -> String {
        if consistent_snapshot {
            format!("{}.{}.json", self.version(), encode_filename(&self.name))
        } else {
            format!("{}.json", encode_filename(&self.name))
        }
    }

    fn role_id(&self) -> RoleId {
        if self.name == "targets" {
            RoleId::StandardRole(RoleType::Targets)
        } else {
            RoleId::DelegatedRole(self.name.clone())
        }
    }
}

impl Signed<DelegatedTargets> {
    /// Convert a `Signed<DelegatedTargets>` to the string representing the role and its `Signed<Targets>`
    pub fn targets(self) -> (String, Signed<Targets>) {
        (
            self.signed.name,
            Signed {
                signed: self.signed.targets,
                signatures: self.signatures,
            },
        )
    }
}

impl Signed<Targets> {
    /// Use a string and a `Signed<Targets>` to create a `Signed<DelegatedTargets>`
    pub fn delegated_targets(self, name: &str) -> Signed<DelegatedTargets> {
        Signed {
            signed: DelegatedTargets {
                name: name.to_string(),
                targets: self.signed,
            },
            signatures: self.signatures,
        }
    }
}

/// Delegations are found in a `targets.json` file.
/// TUF 4.5: DELEGATIONS is an object whose format is the following:
/// ```text
/// { "keys" : {
///       KEYID : KEY,
///       ... },
///   "roles" : [{
///       "name": ROLENAME,
///       "keyids" : [ KEYID, ... ] ,
///       "threshold" : THRESHOLD,
///       ("path_hash_prefixes" : [ HEX_DIGEST, ... ] |
///        "paths" : [ PATHPATTERN, ... ]),
///       "terminating": TERMINATING,
///   }, ... ]
/// }
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct Delegations {
    /// Lists the public keys to verify signatures of delegated targets roles. Revocation and
    /// replacement of delegated targets roles keys is done by changing the keys in this field in
    /// the delegating role's metadata.
    #[serde(deserialize_with = "de::deserialize_keys")]
    pub keys: HashMap<Decoded<Hex>, Key>,

    /// The list of delegated roles.
    pub roles: Vec<DelegatedRole>,
}

/// Each role delegated in a targets file is considered a delegated role
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DelegatedRole {
    /// The name of the delegated role. For example, "projects".
    pub name: String,

    /// The key IDs used by this role.
    pub keyids: Vec<Decoded<Hex>>,

    /// The threshold of signatures required to validate the role.
    pub threshold: NonZeroU64,

    /// The paths governed by this role.
    #[serde(flatten)]
    pub paths: PathSet,

    /// Indicates whether subsequent delegations should be considered.
    pub terminating: bool,

    /// The targets that are signed by this role.
    #[serde(skip)]
    pub targets: Option<Signed<Targets>>,
}

/// Specifies the target paths that a delegated role controls.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum PathSet {
    /// The "paths" list describes paths that the role is trusted to provide. Clients MUST check
    /// that a target is in one of the trusted paths of all roles in a delegation chain, not just in
    /// a trusted path of the role that describes the target file. PATHPATTERN can include shell-
    /// style wildcards and supports the Unix filename pattern matching convention. Its format may
    /// either indicate a path to a single file, or to multiple paths with the use of shell-style
    /// wildcards. For example, the path pattern "targets/*.tgz" would match file paths
    /// "targets/foo.tgz" and "targets/bar.tgz", but not "targets/foo.txt". Likewise, path pattern
    /// "foo-version-?.tgz" matches "foo-version-2.tgz" and "foo-version-a.tgz", but not
    /// "foo-version-alpha.tgz". To avoid surprising behavior when matching targets with
    /// PATHPATTERN, it is RECOMMENDED that PATHPATTERN uses the forward slash (/) as directory
    /// separator and does not start with a directory separator, akin to TARGETSPATH.
    #[serde(rename = "paths")]
    Paths(Vec<PathPattern>),

    /// The "path_hash_prefixes" list is used to succinctly describe a set of target paths.
    /// Specifically, each HEX_DIGEST in "path_hash_prefixes" describes a set of target paths;
    /// therefore, "path_hash_prefixes" is the union over each prefix of its set of target paths.
    /// The target paths must meet this condition: each target path, when hashed with the SHA-256
    /// hash function to produce a 64-byte hexadecimal digest (HEX_DIGEST), must share the same
    /// prefix as one of the prefixes in "path_hash_prefixes". This is useful to split a large
    /// number of targets into separate bins identified by consistent hashing.
    #[serde(rename = "path_hash_prefixes")]
    PathHashPrefixes(Vec<PathHashPrefix>),
}

/// A glob-like path pattern for matching delegated targets, e.g. `foo/bar/*`.
///
/// `PATHPATTERN` supports the Unix shell pattern matching convention for paths
/// ([glob](https://man7.org/linux/man-pages/man7/glob.7.html)bing pathnames). Its format may either
/// indicate a path to a single file, or to multiple files with the use of shell-style wildcards
/// (`*` or `?`). To avoid surprising behavior when matching targets with `PATHPATTERN` it is
/// RECOMMENDED that `PATHPATTERN` uses the forward slash (`/`) as directory separator and does
/// not start with a directory separator, as is also recommended for `TARGETPATH`. A path
/// separator in a path SHOULD NOT be matched by a wildcard in the `PATHPATTERN`.
///
/// Some example `PATHPATTERN`s and expected matches:
/// * a `PATHPATTERN` of `"targets/*.tgz"` would match file paths `"targets/foo.tgz"` and
///   `"targets/bar.tgz"`, but not `"targets/foo.txt"`.
/// * a `PATHPATTERN` of `"foo-version-?.tgz"` matches `"foo-version-2.tgz"` and
///     `"foo-version-a.tgz"`, but not `"foo-version-alpha.tgz"`.
/// * a `PATHPATTERN` of `"*.tgz"` would match `"foo.tgz"` and `"bar.tgz"`,
///   but not `"targets/foo.tgz"`
/// * a `PATHPATTERN` of `"foo.tgz"` would match only `"foo.tgz"`
#[derive(Clone, Debug)]
pub struct PathPattern {
    value: String,
    glob: GlobMatcher,
}

impl PathPattern {
    /// Create a new, valid `PathPattern`. This will fail if we cannot parse the value as a glob. It is important that
    /// our implementation stop if it encounters a glob it cannot parse so that we do not load repositories where we
    /// cannot enforce delegate ownership.
    pub fn new<S: Into<String>>(value: S) -> Result<Self> {
        let value = value.into();
        let glob = Glob::new(&value)
            .context(error::GlobSnafu { pattern: &value })?
            .compile_matcher();
        Ok(Self { value, glob })
    }

    /// Get the inner value of this `PathPattern` as a string.
    pub fn value(&self) -> &str {
        &self.value
    }

    fn matches_target_name(&self, target_name: &TargetName) -> bool {
        self.glob.is_match(target_name.resolved())
    }
}

impl FromStr for PathPattern {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        PathPattern::new(s)
    }
}

impl PartialEq for PathPattern {
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(&self.value, &other.value)
    }
}

impl Serialize for PathPattern {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.value().as_ref())
    }
}

impl<'de> Deserialize<'de> for PathPattern {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = <String>::deserialize(deserializer)?;
        PathPattern::new(s).map_err(|e| D::Error::custom(format!("{}", e)))
    }
}

/// The first characters found in the string representation of a sha256 digest. This can be used for
/// randomly sharding a repository. See [`PathSet::PathHashDigest`] for the description of how this
/// is used.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct PathHashPrefix(String);

impl PathHashPrefix {
    /// Create a new, valid `PathPattern`.
    pub fn new<S: Into<String>>(value: S) -> Result<Self> {
        // In case we choose to reject some of these in the future, we return a result. For now this
        // will always succeed.
        Ok(PathHashPrefix(value.into()))
    }

    /// Get the inner value of this `PathPattern` as a string.
    pub fn value(&self) -> &str {
        &self.0
    }

    fn matches_target_name(&self, target_name: &TargetName) -> bool {
        let target_name_digest =
            digest(&SHA256, target_name.resolved().as_bytes()).encode_hex::<String>();
        target_name_digest.starts_with(self.value())
    }
}

impl FromStr for PathHashPrefix {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        PathHashPrefix::new(s)
    }
}

impl PathSet {
    /// Given a `target_name`, returns whether or not this `PathSet` contains a pattern or hash
    /// prefix that matches.
    fn matches_target_name(&self, target_name: &TargetName) -> bool {
        match self {
            Self::Paths(paths) => {
                for path in paths {
                    if path.matches_target_name(target_name) {
                        return true;
                    }
                }
            }

            Self::PathHashPrefixes(path_prefixes) => {
                for prefix in path_prefixes {
                    if prefix.matches_target_name(target_name) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl Delegations {
    /// Creates a new Delegations with no keys or roles
    pub fn new() -> Self {
        Delegations {
            keys: HashMap::new(),
            roles: Vec::new(),
        }
    }

    /// Determines if target passes pathset specific matching
    pub fn target_is_delegated(&self, target: &TargetName) -> bool {
        for role in &self.roles {
            if role.paths.matches_target_name(target) {
                return true;
            }
        }
        false
    }

    /// Given an object/key that impls Sign, return the corresponding
    /// key ID from Delegation
    pub fn key_id(&self, key_pair: &dyn Sign) -> Option<Decoded<Hex>> {
        for (key_id, key) in &self.keys {
            if key_pair.tuf_key() == *key {
                return Some(key_id.clone());
            }
        }
        None
    }
}

impl DelegatedRole {
    /// Returns a `RoleKeys` representation of the role
    pub fn keys(&self) -> RoleKeys {
        RoleKeys {
            keyids: self.keyids.clone(),
            threshold: self.threshold,
            _extra: HashMap::new(),
        }
    }
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

/// Represents a `timestamp.json` file.
/// TUF 4.6: The timestamp file is signed by a timestamp key. It indicates the latest version of the
/// snapshot metadata and is frequently resigned to limit the amount of time a client can be kept
/// unaware of interference with obtaining updates.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(tag = "_type")]
#[serde(rename = "timestamp")]
pub struct Timestamp {
    /// A string that contains the version number of the TUF specification. Its format follows the
    /// Semantic Versioning 2.0.0 (semver) specification.
    pub spec_version: String,

    /// An integer that is greater than 0. Clients MUST NOT replace a metadata file with a version
    /// number less than the one currently trusted.
    pub version: NonZeroU64,

    /// Determines when metadata should be considered expired and no longer trusted by clients.
    pub expires: DateTime<Utc>,

    /// METAFILES is the same as described for the snapshot.json file. In the case of the
    /// timestamp.json file, this MUST only include a description of the snapshot.json file.
    pub meta: HashMap<String, TimestampMeta>,

    /// Extra arguments found during deserialization.
    ///
    /// We must store these to correctly verify signatures for this object.
    ///
    /// If you're instantiating this struct, you should make this `HashMap::empty()`.
    #[serde(flatten)]
    #[serde(deserialize_with = "de::extra_skip_type")]
    pub _extra: HashMap<String, Value>,
}

/// METAFILES is the same as described for the snapshot.json file. In the case of the timestamp.json
/// file, this MUST only include a description of the snapshot.json file.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct TimestampMeta {
    /// The integer length in bytes of the snapshot.json file.
    pub length: u64,

    /// The hashes of the snapshot.json file.
    pub hashes: Hashes,

    /// An integer that is greater than 0. Clients MUST NOT replace a metadata file with a version
    /// number less than the one currently trusted.
    pub version: NonZeroU64,

    /// Extra arguments found during deserialization.
    ///
    /// We must store these to correctly verify signatures for this object.
    ///
    /// If you're instantiating this struct, you should make this `HashMap::empty()`.
    #[serde(flatten)]
    pub _extra: HashMap<String, Value>,
}

impl Timestamp {
    /// Creates a new `Timestamp` object.
    pub fn new(spec_version: String, version: NonZeroU64, expires: DateTime<Utc>) -> Self {
        Timestamp {
            spec_version,
            version,
            expires,
            meta: HashMap::new(),
            _extra: HashMap::new(),
        }
    }
}

impl Role for Timestamp {
    const TYPE: RoleType = RoleType::Timestamp;

    fn expires(&self) -> DateTime<Utc> {
        self.expires
    }

    fn version(&self) -> NonZeroU64 {
        self.version
    }

    fn filename(&self, _consistent_snapshot: bool) -> String {
        "timestamp.json".to_string()
    }
}

#[test]
fn targets_iter_and_map_test() {
    use maplit::hashmap;

    // Create a dummy Target object.
    let nothing = Target {
        length: 0,
        hashes: Hashes {
            sha256: [0u8].to_vec().into(),
            _extra: HashMap::default(),
        },
        custom: HashMap::default(),
        _extra: HashMap::default(),
    };

    // Create a hierarchy of targets/delegations: a -> b -> c
    let c_role = DelegatedRole {
        name: "c-role".to_string(),
        keyids: vec![],
        threshold: NonZeroU64::new(1).unwrap(),
        paths: PathSet::Paths(vec![PathPattern::new("*").unwrap()]),
        terminating: false,
        targets: Some(Signed {
            signed: Targets {
                spec_version: "".to_string(),
                version: NonZeroU64::new(1).unwrap(),
                expires: Utc::now(),
                targets: hashmap! {
                    TargetName::new("c.txt").unwrap() => nothing.clone(),
                },
                delegations: None,
                _extra: HashMap::default(),
            },
            signatures: vec![],
        }),
    };
    let b_delegations = Delegations {
        keys: HashMap::default(),
        roles: vec![c_role],
    };
    let b_role = DelegatedRole {
        name: "b-role".to_string(),
        keyids: vec![],
        threshold: NonZeroU64::new(1).unwrap(),
        paths: PathSet::Paths(vec![PathPattern::new("*").unwrap()]),
        terminating: false,
        targets: Some(Signed {
            signed: Targets {
                spec_version: "".to_string(),
                version: NonZeroU64::new(1).unwrap(),
                expires: Utc::now(),
                targets: hashmap! {
                    TargetName::new("b.txt").unwrap() => nothing.clone(),
                },
                delegations: Some(b_delegations),
                _extra: HashMap::default(),
            },
            signatures: vec![],
        }),
    };
    let a_delegations = Delegations {
        keys: HashMap::default(),
        roles: vec![b_role],
    };
    let a = Targets {
        spec_version: "".to_string(),
        version: NonZeroU64::new(1).unwrap(),
        expires: Utc::now(),
        targets: hashmap! {
            TargetName::new("a.txt").unwrap() => nothing,
        },
        delegations: Some(a_delegations),
        _extra: HashMap::default(),
    };

    // Assert that targets_iter is recursive and thus has a.txt, b.txt and c.txt
    assert!(a
        .targets_iter()
        .map(|(key, _)| key)
        .any(|item| item.raw() == "a.txt"));
    assert!(a
        .targets_iter()
        .map(|(key, _)| key)
        .any(|item| item.raw() == "b.txt"));
    assert!(a
        .targets_iter()
        .map(|(key, _)| key)
        .any(|item| item.raw() == "c.txt"));

    // Assert that targets_map is also recursive
    let map = a.targets_map();
    assert!(map.contains_key(&TargetName::new("a.txt").unwrap()));
    assert!(map.contains_key(&TargetName::new("b.txt").unwrap()));
    assert!(map.contains_key(&TargetName::new("c.txt").unwrap()));
}
