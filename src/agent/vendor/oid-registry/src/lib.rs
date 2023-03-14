//! [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE-MIT)
//! [![Apache License 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](./LICENSE-APACHE)
//! [![docs.rs](https://docs.rs/oid-registry/badge.svg)](https://docs.rs/oid-registry)
//! [![crates.io](https://img.shields.io/crates/v/oid-registry.svg)](https://crates.io/crates/oid-registry)
//! [![Github CI](https://github.com/rusticata/oid-registry/workflows/Continuous%20integration/badge.svg)](https://github.com/rusticata/oid-registry/actions)
//! [![Minimum rustc version](https://img.shields.io/badge/rustc-1.53.0+-lightgray.svg)](#rust-version-requirements)
//! # OID Registry
//!
//! This crate is a helper crate, containing a database of OID objects. These objects are intended
//! for use when manipulating ASN.1 grammars and BER/DER encodings, for example.
//!
//! This crate provides only a simple registry (similar to a `HashMap`) by default. This object can
//! be used to get names and descriptions from OID.
//!
//! This crate provides default lists of known OIDs, that can be selected using the build features.
//! By default, the registry has no feature enabled, to avoid embedding a huge database in crates.
//!
//! It also declares constants for most of these OIDs.
//!
//! ```rust
//! use oid_registry::OidRegistry;
//!
//! let mut registry = OidRegistry::default()
//! # ;
//! # #[cfg(feature = "crypto")] {
//! #     registry = registry
//!     .with_crypto() // only if the 'crypto' feature is enabled
//! # }
//! ;
//!
//! let e = registry.get(&oid_registry::OID_PKCS1_SHA256WITHRSA);
//! if let Some(entry) = e {
//!     // get sn: sha256WithRSAEncryption
//!     println!("sn: {}", entry.sn());
//!     // get description: SHA256 with RSA encryption
//!     println!("description: {}", entry.description());
//! }
//!
//! ```
//!
//! ## Extending the registry
//!
//! These provided lists are often incomplete, or may lack some specific OIDs.
//! This is why the registry allows adding new entries after construction:
//!
//! ```rust
//! use asn1_rs::oid;
//! use oid_registry::{OidEntry, OidRegistry};
//!
//! let mut registry = OidRegistry::default();
//!
//! // entries can be added by creating an OidEntry object:
//! let entry = OidEntry::new("shortName", "description");
//! registry.insert(oid!(1.2.3.4), entry);
//!
//! // when using static strings, a tuple can also be used directly for the entry:
//! registry.insert(oid!(1.2.3.5), ("shortName", "A description"));
//!
//! ```
//!
//! ## Contributing OIDs
//!
//! All OID values, constants, and features are derived from files in the `assets` directory in the
//! build script (see `build.rs`).
//! See `load_file` for documentation of the file format.

#![deny(missing_docs, unstable_features, unused_import_braces, unused_qualifications, unreachable_pub)]
#![forbid(unsafe_code)]
#![warn(
      /* missing_docs,
      rust_2018_idioms,*/
      missing_debug_implementations,
  )]
// pragmas for doc
// #![deny(intra_doc_link_resolution_failure)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use asn1_rs;
pub use asn1_rs::Oid;

use asn1_rs::oid;
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::From;

mod load;

pub use load::*;

/// An entry stored in the OID registry
#[derive(Debug)]
pub struct OidEntry {
    // Short name
    sn: Cow<'static, str>,
    description: Cow<'static, str>,
}

impl OidEntry {
    /// Create a new entry
    pub fn new<S, T>(sn: S, description: T) -> OidEntry
    where
        S: Into<Cow<'static, str>>,
        T: Into<Cow<'static, str>>,
    {
        let sn = sn.into();
        let description = description.into();
        OidEntry { sn, description }
    }

    /// Get the short name for this entry
    #[inline]
    pub fn sn(&self) -> &str {
        &self.sn
    }

    /// Get the description for this entry
    #[inline]
    pub fn description(&self) -> &str {
        &self.description
    }
}

impl From<(&'static str, &'static str)> for OidEntry {
    fn from(t: (&'static str, &'static str)) -> Self {
        Self::new(t.0, t.1)
    }
}

/// Registry of known OIDs
///
/// Use `OidRegistry::default()` to create an empty registry. If the corresponding features have
/// been selected, the `with_xxx()` methods can be used to add sets of known objets to the
/// database.
///
/// # Example
///
/// ```rust
/// use asn1_rs::{oid, Oid};
/// use oid_registry::{OidEntry, OidRegistry};
///
/// let mut registry = OidRegistry::default()
/// # ;
/// # #[cfg(feature = "crypto")] {
/// #     registry = registry
///     .with_crypto() // only if the 'crypto' feature is enabled
/// # }
/// ;
///
/// // entries can be added by creating an OidEntry object:
/// let entry = OidEntry::new("shortName", "description");
/// registry.insert(oid!(1.2.3.4), entry);
///
/// // when using static strings, a tuple can also be used directly for the entry:
/// registry.insert(oid!(1.2.3.5), ("shortName", "A description"));
///
/// // To query an entry, use the `get` method:
/// const OID_1234: Oid<'static> = oid!(1.2.3.4);
/// let e = registry.get(&OID_1234);
/// assert!(e.is_some());
/// if let Some(e) = e {
///     assert_eq!(e.sn(), "shortName");
/// }
/// ```
#[derive(Debug, Default)]
pub struct OidRegistry<'a> {
    map: HashMap<Oid<'a>, OidEntry>,
}

impl<'a> OidRegistry<'a> {
    /// Insert a new entry
    pub fn insert<E>(&mut self, oid: Oid<'a>, entry: E) -> Option<OidEntry>
    where
        E: Into<OidEntry>,
    {
        self.map.insert(oid, entry.into())
    }

    /// Returns a reference to the registry entry, if found for this OID.
    pub fn get(&self, oid: &Oid<'a>) -> Option<&OidEntry> {
        self.map.get(oid)
    }

    /// Return an Iterator over references to the OID numbers (registry keys)
    pub fn keys(&self) -> impl Iterator<Item = &Oid<'a>> {
        self.map.keys()
    }

    /// Return an Iterator over references to the `OidEntry` values
    pub fn values(&self) -> impl Iterator<Item = &OidEntry> {
        self.map.values()
    }

    /// Return an Iterator over references to the `(Oid, OidEntry)` key/value pairs
    pub fn iter(&self) -> impl Iterator<Item = (&Oid<'a>, &OidEntry)> {
        self.map.iter()
    }

    /// Return the `(Oid, OidEntry)` key/value pairs, matching a short name
    ///
    /// The registry should not contain entries with same short name to avoid ambiguity, but it is
    /// not mandatory.
    ///
    /// This function returns an iterator over the key/value pairs. In most cases, it will have 0
    /// (not found) or 1 item, but can contain more if there are multiple definitions.
    ///
    /// ```rust
    /// # use oid_registry::OidRegistry;
    /// #
    /// # let registry = OidRegistry::default();
    /// // iterate all entries matching "shortName"
    /// for (oid, entry) in registry.iter_by_sn("shortName") {
    ///     // do something
    /// }
    ///
    /// // if you are *sure* that there is at most one entry:
    /// let opt_sn = registry.iter_by_sn("shortName").next();
    /// if let Some((oid, entry)) = opt_sn {
    ///     // do something
    /// }
    /// ```
    pub fn iter_by_sn<S: Into<String>>(&self, sn: S) -> impl Iterator<Item = (&Oid<'a>, &OidEntry)> {
        let s = sn.into();
        self.map.iter().filter(move |(_, entry)| entry.sn == s)
    }

    /// Populate registry with common crypto OIDs (encryption, hash algorithms)
    #[cfg(feature = "crypto")]
    #[cfg_attr(docsrs, doc(cfg(feature = "crypto")))]
    pub fn with_crypto(self) -> Self {
        self.with_pkcs1().with_x962().with_kdf().with_nist_algs()
    }

    /// Populate registry with all known crypto OIDs (encryption, hash algorithms, PKCS constants,
    /// etc.)
    #[cfg(feature = "crypto")]
    #[cfg_attr(docsrs, doc(cfg(feature = "crypto")))]
    pub fn with_all_crypto(self) -> Self {
        self.with_crypto().with_pkcs7().with_pkcs9().with_pkcs12()
    }
}

/// Format a OID to a `String`, using the provided registry to get the short name if present.
pub fn format_oid(oid: &Oid, registry: &OidRegistry) -> String {
    if let Some(entry) = registry.map.get(oid) {
        format!("{} ({})", entry.sn, oid)
    } else {
        format!("{}", oid)
    }
}

include!(concat!(env!("OUT_DIR"), "/oid_db.rs"));

#[rustfmt::skip::macros(oid)]
#[cfg(test)]
mod tests {
    use super::*;

    // This test is mostly a compile test, to ensure the API has not changed
    #[test]
    fn test_lifetimes() {
        fn add_entry(input: &str, oid: Oid<'static>, registry: &mut OidRegistry) {
            // test insertion of owned string
            let s = String::from(input);
            let entry = OidEntry::new("test", s);
            registry.insert(oid, entry);
        }

        let mut registry = OidRegistry::default();
        add_entry("a", oid!(1.2.3.4), &mut registry);
        add_entry("b", oid!(1.2.3.5), &mut registry);

        // test insertion of owned data
        let e = OidEntry::new("c", "test_c");
        registry.insert(oid!(1.2.4.1), e);

        registry.insert(oid!(1.2.5.1), ("a", "b"));

        let iter = registry.iter_by_sn("test");
        assert_eq!(iter.count(), 2);

        // dbg!(&registry);
    }
}
