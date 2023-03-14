//! Certificates and related data structures.
//!
//! An OpenPGP certificate, often called a `PGP key` or just a `key,`
//! is a collection of keys, identity information, and certifications
//! about those keys and identities.
//!
//! The foundation of an OpenPGP certificate is the so-called primary
//! key.  A primary key has three essential functions.  First, the
//! primary key is used to derive a universally unique identifier
//! (UUID) for the certificate, the certificate's so-called
//! fingerprint.  Second, the primary key is used to certify
//! assertions that the certificate holder makes about their
//! certificate.  For instance, to associate a subkey or a User ID
//! with a certificate, the certificate holder uses the primary key to
//! create a self signature called a binding signature.  This binding
//! signature is distributed with the certificate.  It allows anyone
//! who has the certificate to verify that the certificate holder
//! (identified by the primary key) really intended for the subkey to
//! be associated with the certificate.  Finally, the primary key can
//! be used to make assertions about other certificates.  For
//! instance, Alice can make a so-called third-party certification
//! that attests that she is convinced that `Bob` (as described by
//! some User ID) controls a particular certificate.  These
//! third-party certifications are typically distributed alongside the
//! signee's certificate, and are used by trust models like the Web of
//! Trust to authenticate certificates.
//!
//! # Common Operations
//!
//!  - *Generating a certificate*: See the [`CertBuilder`] module.
//!  - *Parsing a certificate*: See the [`Parser` implementation] for `Cert`.
//!  - *Parsing a keyring*: See the [`CertParser`] module.
//!  - *Serializing a certificate*: See the [`Serialize`
//!    implementation] for `Cert`, and the [`Cert::as_tsk`] method to
//!    also include any secret key material.
//!  - *Using a certificate*: See the [`Cert`] and [`ValidCert`] data structures.
//!  - *Revoking a certificate*: See the [`CertRevocationBuilder`] data structure.
//!  - *Decrypt or encrypt secret keys*: See [`packet::Key::encrypt_secret`]'s example.
//!  - *Merging packets*: See the [`Cert::insert_packets`] method.
//!  - *Merging certificates*: See the [`Cert::merge_public`] method.
//!  - *Creating third-party certifications*: See the [`UserID::certify`]
//!     and [`UserAttribute::certify`] methods.
//!  - *Using User IDs and User Attributes*: See the [`ComponentAmalgamation`] module.
//!  - *Using keys*: See the [`KeyAmalgamation`] module.
//!  - *Updating a binding signature*: See the [`UserID::bind`],
//!    [`UserAttribute::bind`], and [`Key::bind`] methods.
//!  - *Checking third-party signatures*: See the
//!    [`Signature::verify_direct_key`],
//!    [`Signature::verify_userid_binding`], and
//!    [`Signature::verify_user_attribute_binding`] methods.
//!  - *Checking third-party revocations*: See the
//!    [`ValidCert::revocation_keys`],
//!    [`ValidAmalgamation::revocation_keys`],
//!    [`Signature::verify_primary_key_revocation`],
//!    [`Signature::verify_userid_revocation`],
//!    [`Signature::verify_user_attribute_revocation`] methods.
//!
//! # Data Structures
//!
//! ## `Cert`
//!
//! The [`Cert`] data structure closely mirrors the transferable
//! public key (`TPK`) data structure described in [Section 11.1] of
//! RFC 4880: it contains the certificate's `Component`s and their
//! associated signatures.
//!
//! ## `Component`s
//!
//! In Sequoia, we refer to `User ID`s, `User Attribute`s, and `Key`s
//! as `Component`s.  To accommodate unsupported components (e.g.,
//! deprecated v3 keys) and unknown components (e.g., the
//! yet-to-be-defined `Xyzzy Property`), we also define an `Unknown`
//! component.
//!
//! ## `ComponentBundle`s
//!
//! We call a Component and any associated signatures a
//! [`ComponentBundle`].  There are four types of associated
//! signatures: self signatures, third-party signatures, self
//! revocations, and third-party revocations.
//!
//! Although some information about a given `Component` is stored in
//! the `Component` itself, most of the information is stored on the
//! associated signatures.  For instance, a key's creation time is
//! stored in the key packet, but the key's capabilities (e.g.,
//! whether it can be used for encryption or signing), and its expiry
//! are stored in the associated self signatures.  Thus, to use a
//! component, we usually need its corresponding self signature.
//!
//! When a certificate is parsed, Sequoia ensures that all components
//! (except the primary key) have at least one valid self signature.
//! However, when using a component, it is still necessary to find the
//! right self signature.  And, unfortunately, finding the
//! self signature for the primary `Key` is non-trivial: that's the
//! primary User ID's self signature.  Another complication is that if
//! the self signature doesn't contain the required information, then
//! the implementation should look for the information on a direct key
//! signature.  Thus, a `ComponentBundle` doesn't contain all of the
//! information that is needed to use a component.
//!
//! ## `ComponentAmalgamation`s
//!
//! To workaround this lack of context, we introduce another data
//! structure called a [`ComponentAmalgamation`].  A
//! `ComponentAmalgamation` references a `ComponentBundle` and its
//! associated `Cert`.  Unfortunately, we can't include a reference to
//! the `Cert` in the `ComponentBundle`, because the `Cert` owns the
//! `ComponentBundle`, and that would create a self-referential data
//! structure, which is currently not supported in Rust.
//!
//! [Section 11.1]: https://tools.ietf.org/html/rfc4880#section-11.1
//! [`ComponentBundle`]: bundle::ComponentBundle
//! [`ComponentAmalgamation`]: amalgamation::ComponentAmalgamation
//! [`Parser` implementation]: struct.Cert.html#impl-Parse%3C%27a%2C%20Cert%3E
//! [`Serialize` implementation]: struct.Cert.html#impl-Serialize
//! [`UserID::certify`]: crate::packet::UserID::certify()
//! [`UserAttribute::certify`]: crate::packet::user_attribute::UserAttribute::certify()
//! [`KeyAmalgamation`]: amalgamation::key
//! [`UserID::bind`]: crate::packet::UserID::bind()
//! [`UserAttribute::bind`]: crate::packet::user_attribute::UserAttribute::bind()
//! [`Key::bind`]: crate::packet::Key::bind()
//! [`Signature::verify_direct_key`]: crate::packet::Signature::verify_direct_key()
//! [`Signature::verify_userid_binding`]: crate::packet::Signature::verify_userid_binding()
//! [`Signature::verify_user_attribute_binding`]: crate::packet::Signature::verify_user_attribute_binding()
//! [`ValidAmalgamation::revocation_keys`]: amalgamation::ValidAmalgamation::revocation_keys
//! [`Signature::verify_primary_key_revocation`]: crate::packet::Signature::verify_primary_key_revocation()
//! [`Signature::verify_userid_revocation`]: crate::packet::Signature::verify_userid_revocation()
//! [`Signature::verify_user_attribute_revocation`]: crate::packet::Signature::verify_user_attribute_revocation()

use std::io;
use std::collections::btree_map::BTreeMap;
use std::collections::btree_map::Entry;
use std::collections::hash_map::DefaultHasher;
use std::cmp;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::hash::Hasher;
use std::path::Path;
use std::mem;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::time;

use crate::{
    crypto::{
        Signer,
        hash::Digest,
    },
    Error,
    Result,
    SignatureType,
    packet,
    packet::Signature,
    packet::Key,
    packet::key,
    packet::Tag,
    packet::UserID,
    packet::UserAttribute,
    packet::Unknown,
    Packet,
    PacketPile,
    seal,
    KeyID,
    Fingerprint,
    KeyHandle,
    policy::Policy,
};
use crate::parse::{Parse, PacketParserResult, PacketParser};
use crate::types::{
    AEADAlgorithm,
    CompressionAlgorithm,
    Features,
    HashAlgorithm,
    KeyServerPreferences,
    ReasonForRevocation,
    RevocationKey,
    RevocationStatus,
    SymmetricAlgorithm,
};

pub mod amalgamation;
mod builder;
mod bindings;
pub mod bundle;
mod parser;
mod revoke;

pub use self::builder::{CertBuilder, CipherSuite, KeyBuilder, SubkeyBuilder};

pub use parser::{
    CertParser,
};

pub(crate) use parser::{
    CertValidator,
    CertValidity,
    KeyringValidator,
    KeyringValidity,
};

pub use revoke::{
    SubkeyRevocationBuilder,
    CertRevocationBuilder,
    UserAttributeRevocationBuilder,
    UserIDRevocationBuilder,
};

pub mod prelude;
use prelude::*;

const TRACE : bool = false;

// Helper functions.

/// Compare the creation time of two signatures.  Order them so that
/// the more recent signature is first.
fn canonical_signature_order(a: Option<time::SystemTime>, b: Option<time::SystemTime>)
                             -> Ordering {
    // Note: None < Some, so the normal ordering is:
    //
    //   None, Some(old), Some(new)
    //
    // Reversing the ordering puts the signatures without a creation
    // time at the end, which is where they belong.
    a.cmp(&b).reverse()
}

/// Compares two signatures by creation time using the MPIs as tie
/// breaker.
///
/// Useful to sort signatures so that the most recent ones are at the
/// front.
fn sig_cmp(a: &Signature, b: &Signature) -> Ordering {
    match canonical_signature_order(a.signature_creation_time(),
                                    b.signature_creation_time()) {
        Ordering::Equal => a.mpis().cmp(b.mpis()),
        r => r
    }
}

impl fmt::Display for Cert {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.fingerprint())
    }
}

/// A collection of `ComponentBundles`.
///
/// Note: we need this, because we can't `impl Vec<ComponentBundles>`.
#[derive(Debug, Clone, PartialEq)]
struct ComponentBundles<C>
    where ComponentBundle<C>: cmp::PartialEq
{
    bundles: Vec<ComponentBundle<C>>,
}

impl<C> Deref for ComponentBundles<C>
    where ComponentBundle<C>: cmp::PartialEq
{
    type Target = Vec<ComponentBundle<C>>;

    fn deref(&self) -> &Self::Target {
        &self.bundles
    }
}

impl<C> DerefMut for ComponentBundles<C>
    where ComponentBundle<C>: cmp::PartialEq
{
    fn deref_mut(&mut self) -> &mut Vec<ComponentBundle<C>> {
        &mut self.bundles
    }
}

impl<C> From<ComponentBundles<C>> for Vec<ComponentBundle<C>>
    where ComponentBundle<C>: cmp::PartialEq
{
    fn from(cb: ComponentBundles<C>) -> Vec<ComponentBundle<C>> {
        cb.bundles
    }
}

impl<C> IntoIterator for ComponentBundles<C>
    where ComponentBundle<C>: cmp::PartialEq
{
    type Item = ComponentBundle<C>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.bundles.into_iter()
    }
}

impl<C> ComponentBundles<C>
    where ComponentBundle<C>: cmp::PartialEq
{
    fn new() -> Self {
        Self { bundles: vec![] }
    }
}

impl<C> ComponentBundles<C>
    where ComponentBundle<C>: cmp::PartialEq
{
    // Sort and dedup the components.
    //
    // `cmp` is a function to sort the components for deduping.
    //
    // `merge` is a function that merges the first component into the
    // second component.
    fn sort_and_dedup<F, F2>(&mut self, cmp: F, merge: F2)
        where F: Fn(&C, &C) -> Ordering,
              F2: Fn(&mut C, &mut C)
    {
        // We dedup by component (not bundles!).  To do this, we need
        // to sort the bundles by their components.

        self.bundles.sort_by(
            |a, b| cmp(&a.component, &b.component));

        self.bundles.dedup_by(|a, b| {
            if cmp(&a.component, &b.component) == Ordering::Equal {
                // Merge.
                merge(&mut a.component, &mut b.component);

                // Recall: if a and b are equal, a will be dropped.
                // Also, the elements are given in the opposite order
                // from their order in the vector.
                b.self_signatures.append(&mut a.self_signatures);
                b.attestations.append(&mut a.attestations);
                b.certifications.append(&mut a.certifications);
                b.self_revocations.append(&mut a.self_revocations);
                b.other_revocations.append(&mut a.other_revocations);

                true
            } else {
                false
            }
        });

        // And sort the certificates.
        for b in self.bundles.iter_mut() {
            b.sort_and_dedup();
        }
    }
}

/// A vecor of key (primary or subkey, public or private) and any
/// associated signatures.
type KeyBundles<KeyPart, KeyRole> = ComponentBundles<Key<KeyPart, KeyRole>>;

/// A vector of subkeys and any associated signatures.
type SubkeyBundles<KeyPart> = KeyBundles<KeyPart, key::SubordinateRole>;

/// A vector of key (primary or subkey, public or private) and any
/// associated signatures.
#[allow(dead_code)]
type GenericKeyBundles
    = ComponentBundles<Key<key::UnspecifiedParts, key::UnspecifiedRole>>;

/// A vector of User ID bundles and any associated signatures.
type UserIDBundles = ComponentBundles<UserID>;

/// A vector of User Attribute bundles and any associated signatures.
type UserAttributeBundles = ComponentBundles<UserAttribute>;

/// A vector of unknown components and any associated signatures.
///
/// Note: all signatures are stored as certifications.
type UnknownBundles = ComponentBundles<Unknown>;

/// Returns the certificate holder's preferences.
///
/// OpenPGP provides a mechanism for a certificate holder to transmit
/// information about communication preferences, and key management to
/// communication partners in an asynchronous manner.  This
/// information is attached to the certificate itself.  Specifically,
/// the different types of information are stored as signature
/// subpackets in the User IDs' self signatures, and in the
/// certificate's direct key signature.
///
/// OpenPGP allows the certificate holder to specify different
/// information depending on the way the certificate is addressed.
/// When addressed by User ID, that User ID's self signature is first
/// checked for the subpacket in question.  If the subpacket is not
/// present or the certificate is addressed is some other way, for
/// instance, by its fingerprint, then the primary User ID's
/// self signature is checked.  If the subpacket is also not there,
/// then the direct key signature is checked.  This policy and its
/// justification are described in [Section 5.2.3.3] of RFC 4880.
///
/// Note: User IDs may be stripped.  For instance, the [WKD] standard
/// requires User IDs that are unrelated to the WKD's domain be
/// stripped from the certificate prior to publication.  As such, any
/// User ID may be considered the primary User ID.  Consequently, if
/// any User ID includes a particular subpacket, then all User IDs
/// should include it.  Furthermore, RFC 4880bis allows certificates
/// [without any User ID packets].  To handle this case, certificates
/// should also create a direct key signature with this information.
///
/// [Section 5.2.3.3]: https://tools.ietf.org/html/rfc4880#section-5.2.3.3
/// [WKD]: https://tools.ietf.org/html/draft-koch-openpgp-webkey-service-09#section-5
/// [without any User ID packets]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09#section-11.1
///
/// # Algorithm Preferences
///
/// Algorithms are ordered with the most preferred algorithm first.
/// According to RFC 4880, if an algorithm is not listed, then the
/// implementation should assume that it is not supported by the
/// certificate holder's software.
///
/// # Examples
///
/// ```
/// use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// use openpgp::cert::prelude::*;
/// use sequoia_openpgp::policy::StandardPolicy;
///
/// # fn main() -> Result<()> {
/// let p = &StandardPolicy::new();
///
/// # let (cert, _) =
/// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
/// #     .generate()?;
/// match cert.with_policy(p, None)?.primary_userid()?.preferred_symmetric_algorithms() {
///     Some(algos) => {
///         println!("Certificate Holder's preferred symmetric algorithms:");
///         for (i, algo) in algos.iter().enumerate() {
///             println!("{}. {}", i, algo);
///         }
///     }
///     None => {
///         println!("Certificate Holder did not specify any preferred \
///                   symmetric algorithms, or the subpacket is missing.");
///     }
/// }
/// # Ok(()) }
/// ```
///
/// # Sealed trait
///
/// This trait is [sealed] and cannot be implemented for types outside this crate.
/// Therefore it can be extended in a non-breaking way.
/// If you want to implement the trait inside the crate
/// you also need to implement the `seal::Sealed` marker trait.
///
/// [sealed]: https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
pub trait Preferences<'a>: seal::Sealed {
    /// Returns the supported symmetric algorithms ordered by
    /// preference.
    ///
    /// The algorithms are ordered according by the certificate
    /// holder's preference.
    fn preferred_symmetric_algorithms(&self)
        -> Option<&'a [SymmetricAlgorithm]>;

    /// Returns the supported hash algorithms ordered by preference.
    ///
    /// The algorithms are ordered according by the certificate
    /// holder's preference.
    fn preferred_hash_algorithms(&self) -> Option<&'a [HashAlgorithm]>;

    /// Returns the supported compression algorithms ordered by
    /// preference.
    ///
    /// The algorithms are ordered according by the certificate
    /// holder's preference.
    fn preferred_compression_algorithms(&self)
        -> Option<&'a [CompressionAlgorithm]>;

    /// Returns the supported AEAD algorithms ordered by preference.
    ///
    /// The algorithms are ordered according by the certificate holder's
    /// preference.
    fn preferred_aead_algorithms(&self) -> Option<&'a [AEADAlgorithm]>;

    /// Returns the certificate holder's keyserver preferences.
    fn key_server_preferences(&self) -> Option<KeyServerPreferences>;

    /// Returns the certificate holder's preferred keyserver for
    /// updates.
    fn preferred_key_server(&self) -> Option<&'a [u8]>;

    /// Returns the certificate holder's feature set.
    fn features(&self) -> Option<Features>;

    /// Returns the URI of a document describing the policy
    /// the certificate was issued under.
    fn policy_uri(&self) -> Option<&'a [u8]>;
}

/// A collection of components and their associated signatures.
///
/// The `Cert` data structure mirrors the [TPK and TSK data
/// structures] defined in RFC 4880.  Specifically, it contains
/// components ([`Key`]s, [`UserID`]s, and [`UserAttribute`]s), their
/// associated self signatures, self revocations, third-party
/// signatures, and third-party revocations, as well as useful methods.
///
/// [TPK and TSK data structures]: https://tools.ietf.org/html/rfc4880#section-11
/// [`Key`]: crate::packet::Key
/// [`UserID`]: crate::packet::UserID
/// [`UserAttribute`]: crate::packet::user_attribute::UserAttribute
///
/// `Cert`s are canonicalized in the sense that their `Component`s are
/// deduplicated, and their signatures and revocations are
/// deduplicated and checked for validity.  The canonicalization
/// routine does *not* throw away components that have no self
/// signatures.  These are returned as usual by, e.g.,
/// [`Cert::userids`].
///
/// [`Cert::userids`]: Cert::userids()
///
/// Keys are deduplicated by comparing their public bits using
/// [`Key::public_cmp`].  If two keys are considered equal, and only
/// one of them has secret key material, the key with the secret key
/// material is preferred.  If both keys have secret material, then
/// one of them is chosen in a deterministic, but undefined manner,
/// which is subject to change.  ***Note***: the secret key material
/// is not integrity checked.  Hence when updating a certificate with
/// secret key material, it is essential to first strip the secret key
/// material from copies that came from an untrusted source.
///
/// [`Key::public_cmp`]: crate::packet::Key::public_cmp()
///
/// Signatures are deduplicated using [their `Eq` implementation],
/// which compares the data that is hashed and the MPIs.  That is, it
/// does not compare [the unhashed data], the digest prefix and the
/// unhashed subpacket area.  If two signatures are considered equal,
/// but have different unhashed data, the unhashed data are merged in
/// a deterministic, but undefined manner, which is subject to change.
/// This policy prevents an attacker from flooding a certificate with
/// valid signatures that only differ in their unhashed data.
///
/// [their `Eq` implementation]: crate::packet::Signature#a-note-on-equality
/// [the unhashed data]: https://tools.ietf.org/html/rfc4880#section-5.2.3
///
/// Self signatures and self revocations are checked for validity by
/// making sure that the signature is *mathematically* correct.  At
/// this point, the signature is *not* checked against a [`Policy`].
///
/// Third-party signatures and revocations are checked for validity by
/// making sure the computed digest matches the [digest prefix] stored
/// in the signature packet.  This is *not* an integrity check and is
/// easily spoofed.  Unfortunately, at the time of canonicalization,
/// the actual signatures cannot be checked, because the public keys
/// are not available.  If you rely on these signatures, it is up to
/// you to check their validity by using an appropriate signature
/// verification method, e.g., [`Signature::verify_userid_binding`]
/// or [`Signature::verify_userid_revocation`].
///
/// [`Policy`]: crate::policy::Policy
/// [digest prefix]: crate::packet::signature::Signature4::digest_prefix()
/// [`Signature::verify_userid_binding`]: crate::packet::Signature::verify_userid_binding()
/// [`Signature::verify_userid_revocation`]: crate::packet::Signature::verify_userid_revocation()
///
/// If a signature or a revocation is not valid,
/// we check to see whether it is simply out of place (i.e., belongs
/// to a different component) and, if so, we reorder it.  If not, it
/// is added to a list of bad signatures.  These can be retrieved
/// using [`Cert::bad_signatures`].
///
/// [`Cert::bad_signatures`]: Cert::bad_signatures()
///
/// Signatures and revocations are sorted so that the newest signature
/// comes first.  Components are sorted, but in an undefined manner
/// (i.e., when parsing the same certificate multiple times, the
/// components will be in the same order, but we reserve the right to
/// change the sort function between versions).
///
/// # Secret Keys
///
/// Any key in a certificate may include secret key material.  To
/// protect secret key material from being leaked, secret keys are not
/// written out when a `Cert` is serialized.  To also serialize secret
/// key material, you need to serialize the object returned by
/// [`Cert::as_tsk()`].
///
///
/// Secret key material may be protected with a password.  In such
/// cases, it needs to be decrypted before it can be used to decrypt
/// data or generate a signature.  Refer to [`Key::decrypt_secret`]
/// for details.
///
/// [`Key::decrypt_secret`]: crate::packet::Key::decrypt_secret()
///
/// # Filtering Certificates
///
/// Component-wise filtering of userids, user attributes, and subkeys
/// can be done with [`Cert::retain_userids`],
/// [`Cert::retain_user_attributes`], and [`Cert::retain_subkeys`].
///
/// [`Cert::retain_userids`]: Cert::retain_userids()
/// [`Cert::retain_user_attributes`]: Cert::retain_user_attributes()
/// [`Cert::retain_subkeys`]: Cert::retain_subkeys()
///
/// If you need even more control, iterate over all components, clone
/// what you want to keep, and then reassemble the certificate.  The
/// following example simply copies all the packets, and can be
/// adapted to suit your policy:
///
/// ```rust
/// # use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// # use openpgp::parse::{Parse, PacketParserResult, PacketParser};
/// use std::convert::TryFrom;
/// use openpgp::cert::prelude::*;
///
/// # fn main() -> Result<()> {
/// fn identity_filter(cert: &Cert) -> Result<Cert> {
///     // Iterate over all of the Cert components, pushing packets we
///     // want to keep into the accumulator.
///     let mut acc = Vec::new();
///
///     // Primary key and related signatures.
///     let c = cert.primary_key();
///     acc.push(c.key().clone().into());
///     for s in c.self_signatures()   { acc.push(s.clone().into()) }
///     for s in c.certifications()    { acc.push(s.clone().into()) }
///     for s in c.self_revocations()  { acc.push(s.clone().into()) }
///     for s in c.other_revocations() { acc.push(s.clone().into()) }
///
///     // UserIDs and related signatures.
///     for c in cert.userids() {
///         acc.push(c.userid().clone().into());
///         for s in c.self_signatures()   { acc.push(s.clone().into()) }
///         for s in c.attestations()      { acc.push(s.clone().into()) }
///         for s in c.certifications()    { acc.push(s.clone().into()) }
///         for s in c.self_revocations()  { acc.push(s.clone().into()) }
///         for s in c.other_revocations() { acc.push(s.clone().into()) }
///     }
///
///     // UserAttributes and related signatures.
///     for c in cert.user_attributes() {
///         acc.push(c.user_attribute().clone().into());
///         for s in c.self_signatures()   { acc.push(s.clone().into()) }
///         for s in c.attestations()      { acc.push(s.clone().into()) }
///         for s in c.certifications()    { acc.push(s.clone().into()) }
///         for s in c.self_revocations()  { acc.push(s.clone().into()) }
///         for s in c.other_revocations() { acc.push(s.clone().into()) }
///     }
///
///     // Subkeys and related signatures.
///     for c in cert.keys().subkeys() {
///         acc.push(c.key().clone().into());
///         for s in c.self_signatures()   { acc.push(s.clone().into()) }
///         for s in c.certifications()    { acc.push(s.clone().into()) }
///         for s in c.self_revocations()  { acc.push(s.clone().into()) }
///         for s in c.other_revocations() { acc.push(s.clone().into()) }
///     }
///
///     // Unknown components and related signatures.
///     for c in cert.unknowns() {
///         acc.push(c.unknown().clone().into());
///         for s in c.self_signatures()   { acc.push(s.clone().into()) }
///         for s in c.certifications()    { acc.push(s.clone().into()) }
///         for s in c.self_revocations()  { acc.push(s.clone().into()) }
///         for s in c.other_revocations() { acc.push(s.clone().into()) }
///     }
///
///     // Any signatures that we could not associate with a component.
///     for s in cert.bad_signatures()     { acc.push(s.clone().into()) }
///
///     // Finally, parse into Cert.
///     Cert::try_from(acc)
/// }
///
/// let (cert, _) =
///     CertBuilder::general_purpose(None, Some("alice@example.org"))
///     .generate()?;
/// assert_eq!(cert, identity_filter(&cert)?);
/// #     Ok(())
/// # }
/// ```
///
/// # A note on equality
///
/// We define equality on `Cert` as the equality of the serialized
/// form as defined by RFC 4880.  That is, two certs are considered
/// equal if and only if their serialized forms are equal, modulo the
/// OpenPGP packet framing (see [`Packet`#a-note-on-equality]).
///
/// Because secret key material is not emitted when a `Cert` is
/// serialized, two certs are considered equal even if only one of
/// them has secret key material.  To take secret key material into
/// account, compare the [`TSK`s](crate::serialize::TSK) instead:
///
/// ```rust
/// # fn main() -> sequoia_openpgp::Result<()> {
/// # use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
///
/// // Generate a cert with secrets.
/// let (cert_with_secrets, _) =
///     CertBuilder::general_purpose(None, Some("alice@example.org"))
///     .generate()?;
///
/// // Derive a cert without secrets.
/// let cert_without_secrets =
///     cert_with_secrets.clone().strip_secret_key_material();
///
/// // Both are considered equal.
/// assert!(cert_with_secrets == cert_without_secrets);
///
/// // But not if we compare their TSKs:
/// assert!(cert_with_secrets.as_tsk() != cert_without_secrets.as_tsk());
/// # Ok(()) }
/// ```
///
/// # Examples
///
/// Parse a certificate:
///
/// ```rust
/// use std::convert::TryFrom;
/// use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// # use openpgp::parse::{Parse, PacketParserResult, PacketParser};
/// use openpgp::Cert;
///
/// # fn main() -> Result<()> {
/// #     let ppr = PacketParser::from_bytes(&b""[..])?;
/// match Cert::try_from(ppr) {
///     Ok(cert) => {
///         println!("Key: {}", cert.fingerprint());
///         for uid in cert.userids() {
///             println!("User ID: {}", uid.userid());
///         }
///     }
///     Err(err) => {
///         eprintln!("Error parsing Cert: {}", err);
///     }
/// }
///
/// #     Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Cert {
    primary: PrimaryKeyBundle<key::PublicParts>,

    userids: UserIDBundles,
    user_attributes: UserAttributeBundles,
    subkeys: SubkeyBundles<key::PublicParts>,

    // Unknown components, e.g., some UserAttribute++ packet from the
    // future.
    unknowns: UnknownBundles,
    // Signatures that we couldn't find a place for.
    bad: Vec<packet::Signature>,
}
assert_send_and_sync!(Cert);

impl std::str::FromStr for Cert {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::from_bytes(s.as_bytes())
    }
}

impl<'a> Parse<'a, Cert> for Cert {
    /// Returns the first Cert encountered in the reader.
    fn from_reader<R: io::Read + Send + Sync>(reader: R) -> Result<Self> {
        Cert::try_from(PacketParser::from_reader(reader)?)
    }

    /// Returns the first Cert encountered in the file.
    fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Cert::try_from(PacketParser::from_file(path)?)
    }

    /// Returns the first Cert found in `buf`.
    ///
    /// `buf` must be an OpenPGP-encoded message.
    fn from_bytes<D: AsRef<[u8]> + ?Sized + Send + Sync>(data: &'a D) -> Result<Self> {
        Cert::try_from(PacketParser::from_bytes(data)?)
    }
}

impl Cert {
    /// Returns the primary key.
    ///
    /// Unlike getting the certificate's primary key using the
    /// [`Cert::keys`] method, this method does not erase the key's
    /// role.
    ///
    /// A key's secret key material may be protected with a password.
    /// In such cases, it needs to be decrypted before it can be used
    /// to decrypt data or generate a signature.  Refer to
    /// [`Key::decrypt_secret`] for details.
    ///
    /// [`Cert::keys`]: Cert::keys()
    /// [`Key::decrypt_secret`]: crate::packet::Key::decrypt_secret()
    ///
    /// # Examples
    ///
    /// The first key returned by [`Cert::keys`] is the primary key,
    /// but its role has been erased:
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) = CertBuilder::new()
    /// #     .add_userid("Alice")
    /// #     .add_signing_subkey()
    /// #     .add_transport_encryption_subkey()
    /// #     .generate()?;
    /// assert_eq!(cert.primary_key().key().role_as_unspecified(),
    ///            cert.keys().nth(0).unwrap().key());
    /// #     Ok(())
    /// # }
    /// ```
    pub fn primary_key(&self) -> PrimaryKeyAmalgamation<key::PublicParts>
    {
        PrimaryKeyAmalgamation::new(self)
    }

    /// Returns the certificate's revocation status.
    ///
    /// Normally, methods that take a policy and a reference time are
    /// only provided by [`ValidCert`].  This method is provided here
    /// because there are two revocation criteria, and one of them is
    /// independent of the reference time.  That is, even if it is not
    /// possible to turn a `Cert` into a `ValidCert` at time `t`, it
    /// may still be considered revoked at time `t`.
    ///
    ///
    /// A certificate is considered revoked at time `t` if:
    ///
    ///   - There is a valid and live revocation at time `t` that is
    ///     newer than all valid and live self signatures at time `t`,
    ///     or
    ///
    ///   - There is a valid [hard revocation] (even if it is not live
    ///     at time `t`, and even if there is a newer self signature).
    ///
    /// [hard revocation]: crate::types::RevocationType::Hard
    ///
    /// Note: certificates and subkeys have different revocation
    /// criteria from [User IDs] and [User Attributes].
    ///
    //  Pending https://github.com/rust-lang/rust/issues/85960, should be
    //  [User IDs]: bundle::ComponentBundle<UserID>::revocation_status
    //  [User Attributes]: bundle::ComponentBundle<UserAttribute>::revocation_status
    /// [User IDs]: bundle::ComponentBundle#method.revocation_status-1
    /// [User Attributes]: bundle::ComponentBundle#method.revocation_status-2
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::types::RevocationStatus;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, rev) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///     .generate()?;
    ///
    /// assert_eq!(cert.revocation_status(p, None), RevocationStatus::NotAsFarAsWeKnow);
    ///
    /// // Merge the revocation certificate.  `cert` is now considered
    /// // to be revoked.
    /// let cert = cert.insert_packets(rev.clone())?;
    /// assert_eq!(cert.revocation_status(p, None),
    ///            RevocationStatus::Revoked(vec![&rev.into()]));
    /// #     Ok(())
    /// # }
    /// ```
    pub fn revocation_status<T>(&self, policy: &dyn Policy, t: T) -> RevocationStatus
        where T: Into<Option<time::SystemTime>>
    {
        let t = t.into();
        // Both a primary key signature and the primary userid's
        // binding signature can override a soft revocation.  Compute
        // the most recent one.
        let vkao = self.primary_key().with_policy(policy, t).ok();
        let mut sig = vkao.as_ref().map(|vka| vka.binding_signature());
        if let Some(direct) = vkao.as_ref()
            .and_then(|vka| vka.direct_key_signature().ok())
        {
            match (direct.signature_creation_time(),
                   sig.and_then(|s| s.signature_creation_time())) {
                (Some(ds), Some(bs)) if ds > bs =>
                    sig = Some(direct),
                _ => ()
            }
        }
        self.primary_key().bundle()._revocation_status(policy, t, true, sig)
    }

    /// Generates a revocation certificate.
    ///
    /// This is a convenience function around
    /// [`CertRevocationBuilder`] to generate a revocation
    /// certificate.  To use the revocation certificate, merge it into
    /// the certificate using [`Cert::insert_packets`].
    ///
    ///
    /// If you want to revoke an individual component, use
    /// [`SubkeyRevocationBuilder`], [`UserIDRevocationBuilder`], or
    /// [`UserAttributeRevocationBuilder`], as appropriate.
    ///
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::types::{ReasonForRevocation, RevocationStatus, SignatureType};
    /// use openpgp::cert::prelude::*;
    /// use openpgp::crypto::KeyPair;
    /// use openpgp::parse::Parse;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, rev) = CertBuilder::new()
    ///     .set_cipher_suite(CipherSuite::Cv25519)
    ///     .generate()?;
    ///
    /// // A new certificate is not revoked.
    /// assert_eq!(cert.revocation_status(p, None),
    ///            RevocationStatus::NotAsFarAsWeKnow);
    ///
    /// // The default revocation certificate is a generic
    /// // revocation.
    /// assert_eq!(rev.reason_for_revocation().unwrap().0,
    ///            ReasonForRevocation::Unspecified);
    ///
    /// // Create a revocation to explain what *really* happened.
    /// let mut keypair = cert.primary_key()
    ///     .key().clone().parts_into_secret()?.into_keypair()?;
    /// let rev = cert.revoke(&mut keypair,
    ///                       ReasonForRevocation::KeyCompromised,
    ///                       b"It was the maid :/")?;
    /// let cert = cert.insert_packets(rev)?;
    /// if let RevocationStatus::Revoked(revs) = cert.revocation_status(p, None) {
    ///     assert_eq!(revs.len(), 1);
    ///     let rev = revs[0];
    ///
    ///     assert_eq!(rev.typ(), SignatureType::KeyRevocation);
    ///     assert_eq!(rev.reason_for_revocation(),
    ///                Some((ReasonForRevocation::KeyCompromised,
    ///                      "It was the maid :/".as_bytes())));
    /// } else {
    ///     unreachable!()
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn revoke(&self, primary_signer: &mut dyn Signer,
                  code: ReasonForRevocation, reason: &[u8])
        -> Result<Signature>
    {
        CertRevocationBuilder::new()
            .set_reason_for_revocation(code, reason)?
            .build(primary_signer, self, None)
    }

    /// Sets the key to expire in delta seconds.
    ///
    /// Note: the time is relative to the key's creation time, not the
    /// current time!
    ///
    /// This function exists to facilitate testing, which is why it is
    /// not exported.
    #[cfg(test)]
    fn set_validity_period_as_of(self, policy: &dyn Policy,
                                 primary_signer: &mut dyn Signer,
                                 expiration: Option<time::Duration>,
                                 now: time::SystemTime)
        -> Result<Cert>
    {
        let primary = self.primary_key().with_policy(policy, now)?;
        let sigs = primary.set_validity_period_as_of(primary_signer,
                                                     expiration,
                                                     now)?;
        self.insert_packets(sigs)
    }

    /// Sets the certificate to expire at the specified time.
    ///
    /// If no time (`None`) is specified, then the certificate is set
    /// to not expire.
    ///
    /// This function creates new binding signatures that cause the
    /// certificate to expire at the specified time.  Specifically, it
    /// updates the current binding signature on each of the valid,
    /// non-revoked User IDs, and the direct key signature, if any.
    /// This is necessary, because the primary User ID is first
    /// consulted when determining the certificate's expiration time,
    /// and certificates can be distributed with a possibly empty
    /// subset of User IDs.
    ///
    /// A policy is needed, because the expiration is updated by
    /// updating the current binding signatures.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::time;
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::crypto::KeyPair;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// # let t0 = time::SystemTime::now() - time::Duration::from_secs(1);
    /// # let (cert, _) = CertBuilder::new()
    /// #     .set_cipher_suite(CipherSuite::Cv25519)
    /// #     .set_creation_time(t0)
    /// #     .generate()?;
    /// // The certificate is alive (not expired).
    /// assert!(cert.with_policy(p, None)?.alive().is_ok());
    ///
    /// // Make cert expire now.
    /// let mut keypair = cert.primary_key()
    ///     .key().clone().parts_into_secret()?.into_keypair()?;
    /// let sigs = cert.set_expiration_time(p, None, &mut keypair,
    ///                                     Some(time::SystemTime::now()))?;
    ///
    /// let cert = cert.insert_packets(sigs)?;
    /// assert!(cert.with_policy(p, None)?.alive().is_err());
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_expiration_time<T>(&self, policy: &dyn Policy, t: T,
                                  primary_signer: &mut dyn Signer,
                                  expiration: Option<time::SystemTime>)
        -> Result<Vec<Signature>>
        where T: Into<Option<time::SystemTime>>,
    {
        let primary = self.primary_key().with_policy(policy, t.into())?;
        primary.set_expiration_time(primary_signer, expiration)
    }

    /// Returns the primary User ID at the reference time, if any.
    fn primary_userid_relaxed<'a, T>(&'a self, policy: &'a dyn Policy, t: T,
                                     valid_cert: bool)
        -> Result<ValidUserIDAmalgamation<'a>>
        where T: Into<Option<std::time::SystemTime>>
    {
        let t = t.into().unwrap_or_else(crate::now);
        ValidComponentAmalgamation::primary(self, self.userids.iter(),
                                            policy, t, valid_cert)
    }

    /// Returns an iterator over the certificate's User IDs.
    ///
    /// **Note:** This returns all User IDs, even those without a
    /// binding signature.  This is not what you want, unless you are
    /// doing a low-level inspection of the certificate.  Use
    /// [`ValidCert::userids`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::prelude::*;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, rev) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// println!("{}'s User IDs:", cert.fingerprint());
    /// for ua in cert.userids() {
    ///     println!("  {}", String::from_utf8_lossy(ua.value()));
    /// }
    /// # // Add a User ID without a binding signature and make sure
    /// # // it is still returned.
    /// # let userid = UserID::from("alice@example.net");
    /// # let cert = cert.insert_packets(userid)?;
    /// # assert_eq!(cert.userids().count(), 2);
    /// #     Ok(())
    /// # }
    /// ```
    pub fn userids(&self) -> UserIDAmalgamationIter {
        ComponentAmalgamationIter::new(self, self.userids.iter())
    }

    /// Returns an iterator over the certificate's User Attributes.
    ///
    /// **Note:** This returns all User Attributes, even those without
    /// a binding signature.  This is not what you want, unless you
    /// are doing a low-level inspection of the certificate.  Use
    /// [`ValidCert::user_attributes`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, rev) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// println!("{}'s has {} User Attributes.",
    ///          cert.fingerprint(),
    ///          cert.user_attributes().count());
    /// # assert_eq!(cert.user_attributes().count(), 0);
    /// #     Ok(())
    /// # }
    /// ```
    pub fn user_attributes(&self) -> UserAttributeAmalgamationIter {
        ComponentAmalgamationIter::new(self, self.user_attributes.iter())
    }

    /// Returns an iterator over the certificate's keys.
    ///
    /// That is, this returns an iterator over the primary key and any
    /// subkeys.
    ///
    /// **Note:** This returns all keys, even those without a binding
    /// signature.  This is not what you want, unless you are doing a
    /// low-level inspection of the certificate.  Use
    /// [`ValidCert::keys`] instead.
    ///
    /// By necessity, this function erases the returned keys' roles.
    /// If you are only interested in the primary key, use
    /// [`Cert::primary_key`].  If you are only interested in the
    /// subkeys, use [`KeyAmalgamationIter::subkeys`].  These
    /// functions preserve the keys' role in the type system.
    ///
    /// A key's secret key material may be protected with a
    /// password.  In such cases, it needs to be decrypted before it
    /// can be used to decrypt data or generate a signature.  Refer to
    /// [`Key::decrypt_secret`] for details.
    ///
    /// [`Cert::primary_key`]: Cert::primary_key()
    /// [`KeyAmalgamationIter::subkeys`]: amalgamation::key::KeyAmalgamationIter::subkeys()
    /// [`Key::decrypt_secret`]: crate::packet::Key::decrypt_secret()
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::Tag;
    /// # use std::convert::TryInto;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) = CertBuilder::new()
    /// #     .add_userid("Alice")
    /// #     .add_signing_subkey()
    /// #     .add_transport_encryption_subkey()
    /// #     .generate()?;
    /// println!("{}'s has {} keys.",
    ///          cert.fingerprint(),
    ///          cert.keys().count());
    /// # assert_eq!(cert.keys().count(), 1 + 2);
    /// #
    /// # // Make sure that we keep all keys even if they don't have
    /// # // any self signatures.
    /// # let packets = cert.into_packets()
    /// #     .filter(|p| p.tag() != Tag::Signature)
    /// #     .collect::<Vec<_>>();
    /// # let cert : Cert = packets.try_into()?;
    /// # assert_eq!(cert.keys().count(), 1 + 2);
    /// #
    /// #     Ok(())
    /// # }
    /// ```
    pub fn keys(&self) -> KeyAmalgamationIter<key::PublicParts, key::UnspecifiedRole>
    {
        KeyAmalgamationIter::new(self)
    }

    /// Returns an iterator over the certificate's subkeys.
    pub(crate) fn subkeys(&self) -> ComponentAmalgamationIter<Key<key::PublicParts,
                                                      key::SubordinateRole>>
    {
        ComponentAmalgamationIter::new(self, self.subkeys.iter())
    }

    /// Returns an iterator over the certificate's unknown components.
    ///
    /// This function returns all unknown components even those
    /// without a binding signature.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::packet::prelude::*;
    /// # use openpgp::cert::prelude::*;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// # let tag = Tag::Private(61);
    /// # let unknown
    /// #     = Unknown::new(tag, openpgp::Error::UnsupportedPacketType(tag).into());
    /// # let cert = cert.insert_packets(unknown)?;
    /// println!("{}'s has {} unknown components.",
    ///          cert.fingerprint(),
    ///          cert.unknowns().count());
    /// for ua in cert.unknowns() {
    ///     println!("  Unknown component with tag {} ({}), error: {}",
    ///              ua.tag(), u8::from(ua.tag()), ua.error());
    /// }
    /// # assert_eq!(cert.unknowns().count(), 1);
    /// # assert_eq!(cert.unknowns().nth(0).unwrap().tag(), tag);
    /// # Ok(())
    /// # }
    /// ```
    pub fn unknowns(&self) -> UnknownComponentAmalgamationIter {
        ComponentAmalgamationIter::new(self, self.unknowns.iter())
    }

    /// Returns a slice containing the bad signatures.
    ///
    /// Bad signatures are signatures and revocations that we could
    /// not associate with one of the certificate's components.
    ///
    /// For self signatures and self revocations, we check that the
    /// signature is correct.  For third-party signatures and
    /// third-party revocations, we only check that the [digest
    /// prefix] is correct, because third-party keys are not
    /// available.  Checking the digest prefix is *not* an integrity
    /// check; third party-signatures and third-party revocations may
    /// be invalid and must still be checked for validity before use.
    ///
    /// [digest prefix]: packet::signature::Signature4::digest_prefix()
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, rev) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// println!("{}'s has {} bad signatures.",
    ///          cert.fingerprint(),
    ///          cert.bad_signatures().count());
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// #     Ok(())
    /// # }
    /// ```
    pub fn bad_signatures(&self)
                          -> impl Iterator<Item = &Signature> + Send + Sync {
        self.bad.iter()
    }

    /// Returns a list of any designated revokers for this certificate.
    ///
    /// This function returns the designated revokers listed on the
    /// primary key's binding signatures and the certificate's direct
    /// key signatures.
    ///
    /// Note: the returned list is deduplicated.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::RevocationKey;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (alice, _) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///     .generate()?;
    /// // Make Alice a designated revoker for Bob.
    /// let (bob, _) =
    ///     CertBuilder::general_purpose(None, Some("bob@example.org"))
    ///     .set_revocation_keys(vec![(&alice).into()])
    ///     .generate()?;
    ///
    /// // Make sure Alice is listed as a designated revoker for Bob.
    /// assert_eq!(bob.revocation_keys(p).collect::<Vec<&RevocationKey>>(),
    ///            vec![&(&alice).into()]);
    /// # Ok(()) }
    /// ```
    pub fn revocation_keys<'a>(&'a self, policy: &dyn Policy)
        -> Box<dyn Iterator<Item = &'a RevocationKey> + 'a>
    {
        let mut keys = std::collections::HashSet::new();

        let pk_sec = self.primary_key().hash_algo_security();

        // All user ids.
        self.userids()
            .flat_map(|ua| {
                // All valid self-signatures.
                let sec = ua.hash_algo_security;
                ua.self_signatures()
                    .filter(move |sig| {
                        policy.signature(sig, sec).is_ok()
                   })
            })
            // All direct-key signatures.
            .chain(self.primary_key()
                   .self_signatures()
                   .filter(|sig| {
                       policy.signature(sig, pk_sec).is_ok()
                   }))
            .flat_map(|sig| sig.revocation_keys())
            .for_each(|rk| { keys.insert(rk); });

        Box::new(keys.into_iter())
    }

    /// Converts the certificate into an iterator over a sequence of
    /// packets.
    ///
    /// **WARNING**: When serializing a `Cert`, any secret key
    /// material is dropped.  In order to serialize the secret key
    /// material, it is first necessary to convert the `Cert` into a
    /// [`TSK`] and serialize that.  This behavior makes it harder to
    /// accidentally leak secret key material.  *This function is
    /// different.* If a key contains secret key material, it is
    /// exported as a [`SecretKey`] or [`SecretSubkey`], as
    /// appropriate.  This means that **if you serialize the resulting
    /// packets, the secret key material will be serialized too**.
    ///
    /// [`TSK`]: crate::serialize::TSK
    /// [`SecretKey`]: Packet::SecretKey
    /// [`SecretSubkey`]: Packet::SecretSubkey
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) =
    /// #       CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #       .generate()?;
    /// println!("Cert contains {} packets",
    ///          cert.into_packets().count());
    /// #     Ok(())
    /// # }
    /// ```
    pub fn into_packets(self) -> impl Iterator<Item=Packet> + Send + Sync {
        fn rewrite(mut p: impl Iterator<Item=Packet> + Send + Sync)
            -> impl Iterator<Item=Packet> + Send + Sync
        {
            let k: Packet = match p.next().unwrap() {
                Packet::PublicKey(k) => {
                    if k.has_secret() {
                        Packet::SecretKey(k.parts_into_secret().unwrap())
                    } else {
                        Packet::PublicKey(k)
                    }
                }
                Packet::PublicSubkey(k) => {
                    if k.has_secret() {
                        Packet::SecretSubkey(k.parts_into_secret().unwrap())
                    } else {
                        Packet::PublicSubkey(k)
                    }
                }
                _ => unreachable!(),
            };

            std::iter::once(k).chain(p)
        }

        rewrite(self.primary.into_packets())
            .chain(self.userids.into_iter().flat_map(|b| b.into_packets()))
            .chain(self.user_attributes.into_iter().flat_map(|b| b.into_packets()))
            .chain(self.subkeys.into_iter().flat_map(|b| rewrite(b.into_packets())))
            .chain(self.unknowns.into_iter().flat_map(|b| b.into_packets()))
            .chain(self.bad.into_iter().map(|s| s.into()))
    }

    /// Returns the first certificate found in the sequence of packets.
    ///
    /// If the sequence of packets does not start with a certificate
    /// (specifically, if it does not start with a primary key
    /// packet), then this fails.
    ///
    /// If the sequence contains multiple certificates (i.e., it is a
    /// keyring), or the certificate is followed by an invalid packet
    /// this function will fail.  To parse keyrings, use
    /// [`CertParser`] instead of this function.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::PacketPile;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let (cert, rev) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///     .generate()?;
    ///
    /// // We should be able to turn a certificate into a PacketPile
    /// // and back.
    /// assert!(Cert::from_packets(cert.into_packets()).is_ok());
    ///
    /// // But a revocation certificate is not a certificate, so this
    /// // will fail.
    /// let p : Vec<Packet> = vec![rev.into()];
    /// assert!(Cert::from_packets(p.into_iter()).is_err());
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_packets(p: impl Iterator<Item=Packet> + Send + Sync) -> Result<Self> {
        let mut i = parser::CertParser::from_iter(p);
        if let Some(cert_result) = i.next() {
            if i.next().is_some() {
                Err(Error::MalformedCert(
                    "Additional packets found, is this a keyring?".into()
                ).into())
            } else {
                cert_result
            }
        } else {
            Err(Error::MalformedCert("No data".into()).into())
        }
    }

    /// Converts the certificate into a `PacketPile`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::PacketPile;
    /// # use openpgp::cert::prelude::*;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) =
    /// #       CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #       .generate()?;
    /// let pp = cert.into_packet_pile();
    /// # let _ : PacketPile = pp;
    /// #     Ok(())
    /// # }
    /// ```
    pub fn into_packet_pile(self) -> PacketPile {
        self.into()
    }

    /// Sorts and deduplicates all components and all signatures of
    /// all components.
    fn sort_and_dedup(&mut self) {
        self.primary.sort_and_dedup();

        self.bad.sort_by(Signature::normalized_cmp);
        self.bad.dedup_by(|a, b| a.normalized_eq(b));
        // Order bad signatures so that the most recent one comes
        // first.
        self.bad.sort_by(sig_cmp);

        self.userids.sort_and_dedup(UserID::cmp, |_, _| {});
        self.user_attributes.sort_and_dedup(UserAttribute::cmp, |_, _| {});
        // XXX: If we have two keys with the same public parts and
        // different non-empty secret parts, then the one that comes
        // first will be dropped, the one that comes later will be
        // kept.
        //
        // This can happen if:
        //
        //   - One is corrupted
        //   - There are two versions that are encrypted differently
        //
        // If the order of the keys is unpredictable, this effect is
        // unpredictable!  However, if we merge two canonicalized
        // certs with Cert::merge_public_and_secret, then we know the
        // order: the version in `self` comes first, the version in
        // `other` comes last.
        self.subkeys.sort_and_dedup(Key::public_cmp,
            |a, b| {
                // Recall: if a and b are equal, a will be dropped.
                // Also, the elements are given in the opposite order
                // from their order in the vector.
                //
                // Prefer the secret in `a`, i.e. the "later" one.
                if a.has_secret() {
                    std::mem::swap(a, b);
                }
            });

        self.unknowns.sort_and_dedup(Unknown::best_effort_cmp, |_, _| {});
    }

    fn canonicalize(mut self) -> Self {
        tracer!(TRACE, "canonicalize", 0);

        // Before we do anything, we'll order and deduplicate the
        // components.  If two components are the same, they will be
        // merged, and their signatures will also be deduplicated.
        // This improves the performance considerably when we update a
        // certificate, because the certificates will be most likely
        // almost identical, and we avoid about half of the signature
        // verifications.
        self.sort_and_dedup();

        // Now we verify the self signatures.  There are a few things
        // that we need to be aware of:
        //
        //  - Signatures may be invalid.  These should be dropped.
        //
        //  - Signatures may be out of order.  These should be
        //    reordered so that we have the latest self signature and
        //    we don't drop a userid or subkey that is actually
        //    valid.

        // We collect bad signatures here in self.bad.  Below, we'll
        // test whether they are just out of order by checking them
        // against all userids and subkeys.  Furthermore, this may be
        // a partial Cert that is merged into an older copy.

        // desc: a description of the component
        // binding: the binding to check
        // sigs: a vector of sigs in $binding to check
        // verify_method: the method to call on a signature to verify it
        // verify_args: additional arguments to pass to verify_method
        macro_rules! check {
            ($desc:expr, $binding:expr, $sigs:ident,
             $verify_method:ident, $($verify_args:expr),*) => ({
                t!("check!({}, {}, {:?}, {}, ...)",
                   $desc, stringify!($binding), $binding.$sigs,
                   stringify!($verify_method));
                for mut sig in mem::take(&mut $binding.$sigs).into_iter() {
                     match sig.$verify_method(self.primary.key(),
                                              self.primary.key(),
                                              $($verify_args),*) {
                         Ok(()) => $binding.$sigs.push(sig),
                         Err(err) => {
                             t!("Sig {:02X}{:02X}, type = {} \
                                 doesn't belong to {}: {:?}",
                                sig.digest_prefix()[0], sig.digest_prefix()[1],
                                sig.typ(), $desc, err);

                             self.bad.push(sig);
                         }
                    }
                }
            });
            ($desc:expr, $binding:expr, $sigs:ident,
             $verify_method:ident) => ({
                check!($desc, $binding, $sigs, $verify_method,)
            });
        }

        // The same as check!, but for third party signatures.  If we
        // do have the key that made the signature, we can verify it
        // like in check!.  Otherwise, we use the hash prefix as
        // heuristic approximating the verification.
        macro_rules! check_3rd_party {
            ($desc:expr,            // a description of the component
             $binding:expr,         // the binding to check
             $sigs:ident,           // a vector of sigs in $binding to check
             $lookup_fn:expr,       // a function to lookup keys
             $verify_method:ident,  // the method to call to verify it
             $hash_method:ident,    // the method to call to compute the hash
             $($verify_args:expr),* // additional arguments to pass to the above
            ) => ({
                t!("check_3rd_party!({}, {}, {:?}, {}, {}, ...)",
                   $desc, stringify!($binding), $binding.$sigs,
                   stringify!($verify_method), stringify!($hash_method));
                for mut sig in mem::take(&mut $binding.$sigs) {
                    // Use hash prefix as heuristic.
                    let key = self.primary.key();
                    match sig.hash_algo().context().and_then(|mut ctx| {
                        sig.$hash_method(&mut ctx, key, $($verify_args),*);
                        ctx.into_digest()
                    }) {
                      Ok(hash) => {
                        if &sig.digest_prefix()[..] == &hash[..2] {
                            // See if we can get the key for a
                            // positive verification.
                            if let Some(key) = $lookup_fn(&sig) {
                                if let Ok(()) = sig.$verify_method(
                                    &key, self.primary.key(), $($verify_args),*)
                                {
                                    $binding.$sigs.push(sig);
                                } else {
                                    t!("Sig {:02X}{:02X}, type = {} \
                                        doesn't belong to {}",
                                       sig.digest_prefix()[0],
                                       sig.digest_prefix()[1],
                                       sig.typ(), $desc);

                                    self.bad.push(sig);
                                }
                            } else {
                                // No key, we need to trust our heuristic.
                                $binding.$sigs.push(sig);
                            }
                        } else {
                            t!("Sig {:02X}{:02X}, type = {} \
                                doesn't belong to {} (computed hash's prefix: {:02X}{:02X})",
                               sig.digest_prefix()[0], sig.digest_prefix()[1],
                               sig.typ(), $desc,
                               hash[0], hash[1]);

                            self.bad.push(sig);
                        }
                      },
                      Err(e) => {
                        // Hashing failed, we likely don't support
                        // the hash algorithm.
                        t!("Sig {:02X}{:02X}, type = {}: \
                            Hashing failed: {}",
                           sig.digest_prefix()[0], sig.digest_prefix()[1],
                           sig.typ(), e);

                        self.bad.push(sig);
                      },
                    }
                }
            });
            ($desc:expr, $binding:expr, $sigs:ident, $lookup_fn:expr,
             $verify_method:ident, $hash_method:ident) => ({
                 check_3rd_party!($desc, $binding, $sigs, $lookup_fn,
                                  $verify_method, $hash_method, )
            });
        }

        // Placeholder lookup function.
        fn lookup_fn(_: &Signature)
                     -> Option<Key<key::PublicParts, key::UnspecifiedRole>> {
            None
        }

        check!("primary key",
               self.primary, self_signatures, verify_direct_key);
        check!("primary key",
               self.primary, self_revocations, verify_primary_key_revocation);
        check_3rd_party!("primary key",
                         self.primary, certifications, lookup_fn,
                         verify_direct_key, hash_direct_key);
        check_3rd_party!("primary key",
                         self.primary, other_revocations, lookup_fn,
                         verify_primary_key_revocation, hash_direct_key);

        for ua in self.userids.iter_mut() {
            check!(format!("userid \"{}\"",
                           String::from_utf8_lossy(ua.userid().value())),
                   ua, self_signatures, verify_userid_binding,
                   ua.userid());
            check!(format!("userid \"{}\"",
                           String::from_utf8_lossy(ua.userid().value())),
                   ua, self_revocations, verify_userid_revocation,
                   ua.userid());
            check_3rd_party!(
                format!("userid \"{}\"",
                        String::from_utf8_lossy(ua.userid().value())),
                ua, certifications, lookup_fn,
                verify_userid_binding, hash_userid_binding,
                ua.userid());
            check_3rd_party!(
                format!("userid \"{}\"",
                        String::from_utf8_lossy(ua.userid().value())),
                ua, other_revocations, lookup_fn,
                verify_userid_revocation, hash_userid_binding,
                ua.userid());
        }

        for binding in self.user_attributes.iter_mut() {
            check!("user attribute",
                   binding, self_signatures, verify_user_attribute_binding,
                   binding.user_attribute());
            check!("user attribute",
                   binding, self_revocations, verify_user_attribute_revocation,
                   binding.user_attribute());
            check_3rd_party!(
                "user attribute",
                binding, certifications, lookup_fn,
                verify_user_attribute_binding, hash_user_attribute_binding,
                binding.user_attribute());
            check_3rd_party!(
                "user attribute",
                binding, other_revocations, lookup_fn,
                verify_user_attribute_revocation, hash_user_attribute_binding,
                binding.user_attribute());
        }

        for binding in self.subkeys.iter_mut() {
            check!(format!("subkey {}", binding.key().keyid()),
                   binding, self_signatures, verify_subkey_binding,
                   binding.key());
            check!(format!("subkey {}", binding.key().keyid()),
                   binding, self_revocations, verify_subkey_revocation,
                   binding.key());
            check_3rd_party!(
                format!("subkey {}", binding.key().keyid()),
                binding, certifications, lookup_fn,
                verify_subkey_binding, hash_subkey_binding,
                binding.key());
            check_3rd_party!(
                format!("subkey {}", binding.key().keyid()),
                binding, other_revocations, lookup_fn,
                verify_subkey_revocation, hash_subkey_binding,
                binding.key());
        }

        // See if the signatures that didn't validate are just out of
        // place.
        let mut bad_sigs: Vec<(Option<usize>, Signature)> =
            std::mem::take(&mut self.bad).into_iter()
            .map(|sig| {
                t!("We're going to reconsider bad signature {:?}", sig);
                (None, sig)
            })
            .collect();

        // Do the same for signatures on unknown components, but
        // remember where we took them from.
        for (i, c) in self.unknowns.iter_mut().enumerate() {
            for sig in
                std::mem::take(&mut c.self_signatures).into_iter()
                .chain(
                    std::mem::take(&mut c.certifications).into_iter())
                .chain(
                    std::mem::take(&mut c.attestations).into_iter())
                .chain(
                    std::mem::take(&mut c.self_revocations).into_iter())
                .chain(
                    std::mem::take(&mut c.other_revocations).into_iter())
            {
                t!("We're going to reconsider {:?} on unknown component #{}",
                   sig, i);
                bad_sigs.push((Some(i), sig));
            }
        }

        let primary_fp: KeyHandle = self.key_handle();
        let primary_keyid = KeyHandle::KeyID(primary_fp.clone().into());

        'outer: for (unknown_idx, mut sig) in bad_sigs {
            // Did we find a new place for sig?
            let mut found_component = false;

            // Is this signature a self-signature?
            let issuers =
                sig.get_issuers();
            let is_selfsig =
                issuers.is_empty()
                || issuers.contains(&primary_fp)
                || issuers.contains(&primary_keyid);

            macro_rules! check_one {
                ($desc:expr, $sigs:expr, $sig:expr,
                 $verify_method:ident, $($verify_args:expr),*) => ({
                   if is_selfsig {
                     t!("check_one!({}, {:?}, {:?}, {}, ...)",
                        $desc, $sigs, $sig,
                        stringify!($verify_method));
                     match $sig.$verify_method(self.primary.key(),
                                               self.primary.key(),
                                               $($verify_args),*)
                     {
                       Ok(()) => {
                         t!("Sig {:02X}{:02X}, {:?} \
                             was out of place.  Belongs to {}.",
                            $sig.digest_prefix()[0],
                            $sig.digest_prefix()[1],
                            $sig.typ(), $desc);

                         $sigs.push($sig);
                         continue 'outer;
                       },
                       Err(err) => {
                         t!("Sig {:02X}{:02X}, type = {} \
                             doesn't belong to {}: {:?}",
                            sig.digest_prefix()[0], sig.digest_prefix()[1],
                            sig.typ(), $desc, err);
                       },
                     }
                   }
                 });
                ($desc:expr, $sigs:expr, $sig:expr,
                 $verify_method:ident) => ({
                    check_one!($desc, $sigs, $sig, $verify_method,)
                });
            }

            // The same as check_one!, but for third party signatures.
            // If we do have the key that made the signature, we can
            // verify it like in check!.  Otherwise, we use the hash
            // prefix as heuristic approximating the verification.
            macro_rules! check_one_3rd_party {
                ($desc:expr,            // a description of the component
                 $sigs:expr,            // where to put $sig if successful
                 $sig:ident,            // the signature to check
                 $lookup_fn:expr,       // a function to lookup keys
                 $verify_method:ident,  // the method to verify it
                 $hash_method:ident,    // the method to compute the hash
                 $($verify_args:expr),* // additional arguments for the above
                ) => ({
                  if ! is_selfsig {
                    t!("check_one_3rd_party!({}, {}, {:?}, {}, {}, ...)",
                       $desc, stringify!($sigs), $sig,
                       stringify!($verify_method), stringify!($hash_method));
                    if let Some(key) = $lookup_fn(&sig) {
                        match sig.$verify_method(&key,
                                                 self.primary.key(),
                                                 $($verify_args),*)
                        {
                          Ok(()) => {
                            t!("Sig {:02X}{:02X}, {:?} \
                                was out of place.  Belongs to {}.",
                               $sig.digest_prefix()[0],
                               $sig.digest_prefix()[1],
                               $sig.typ(), $desc);

                            $sigs.push($sig);
                            continue 'outer;
                          },
                          Err(err) => {
                            t!("Sig {:02X}{:02X}, type = {} \
                                doesn't belong to {}: {:?}",
                               sig.digest_prefix()[0], sig.digest_prefix()[1],
                               sig.typ(), $desc, err);
                          },
                       }
                    } else {
                        // Use hash prefix as heuristic.
                        let key = self.primary.key();
                        if let Ok(hash) = sig.hash_algo().context()
                            .and_then(|mut ctx| {
                                sig.$hash_method(&mut ctx, key,
                                                 $($verify_args),*);
                                ctx.into_digest()
                            })
                        {
                            if &sig.digest_prefix()[..] == &hash[..2] {
                                t!("Sig {:02X}{:02X}, {:?} \
                                    was out of place.  Likely belongs to {}.",
                                   $sig.digest_prefix()[0],
                                   $sig.digest_prefix()[1],
                                   $sig.typ(), $desc);

                                $sigs.push($sig.clone());
                                // The cost of missing a revocation
                                // certificate merely because we put
                                // it into the wrong place seem to
                                // outweigh the cost of duplicating
                                // it.
                                t!("Will keep trying to match this sig to \
                                    other components (found before? {:?})...",
                                   found_component);
                                found_component = true;
                            } else {
                                t!("Sig {:02X}{:02X}, {:?} \
                                    does not belong to {}: \
                                    hash prefix mismatch",
                                   $sig.digest_prefix()[0],
                                   $sig.digest_prefix()[1],
                                   $sig.typ(), $desc);
                            }
                        } else {
                            t!("Sig {:02X}{:02X}, type = {} \
                                doesn't use a supported hash algorithm: \
                                {:?} unsupported",
                               sig.digest_prefix()[0], sig.digest_prefix()[1],
                               sig.typ(), sig.hash_algo());
                        }
                    }
                  }
                });
                ($desc:expr, $sigs:expr, $sig:ident, $lookup_fn:expr,
                 $verify_method:ident, $hash_method:ident) => ({
                     check_one_3rd_party!($desc, $sigs, $sig, $lookup_fn,
                                          $verify_method, $hash_method, )
                 });
            }

            use SignatureType::*;
            match sig.typ() {
                DirectKey => {
                    check_one!("primary key", self.primary.self_signatures,
                               sig, verify_direct_key);
                    check_one_3rd_party!(
                        "primary key", self.primary.certifications, sig,
                        lookup_fn,
                        verify_direct_key, hash_direct_key);
                },

                KeyRevocation => {
                    check_one!("primary key", self.primary.self_revocations,
                               sig, verify_primary_key_revocation);
                    check_one_3rd_party!(
                        "primary key", self.primary.other_revocations, sig,
                        lookup_fn, verify_primary_key_revocation,
                        hash_direct_key);
                },

                GenericCertification | PersonaCertification
                    | CasualCertification | PositiveCertification =>
                {
                    for binding in self.userids.iter_mut() {
                        check_one!(format!("userid \"{}\"",
                                           String::from_utf8_lossy(
                                               binding.userid().value())),
                                   binding.self_signatures, sig,
                                   verify_userid_binding, binding.userid());
                        check_one_3rd_party!(
                            format!("userid \"{}\"",
                                    String::from_utf8_lossy(
                                        binding.userid().value())),
                            binding.certifications, sig, lookup_fn,
                            verify_userid_binding, hash_userid_binding,
                            binding.userid());
                    }

                    for binding in self.user_attributes.iter_mut() {
                        check_one!("user attribute",
                                   binding.self_signatures, sig,
                                   verify_user_attribute_binding,
                                   binding.user_attribute());
                        check_one_3rd_party!(
                            "user attribute",
                            binding.certifications, sig, lookup_fn,
                            verify_user_attribute_binding,
                            hash_user_attribute_binding,
                            binding.user_attribute());
                    }
                },

                crate::types::SignatureType::AttestationKey => {
                    for binding in self.userids.iter_mut() {
                        check_one!(format!("userid \"{}\"",
                                           String::from_utf8_lossy(
                                               binding.userid().value())),
                                   binding.attestations, sig,
                                   verify_userid_attestation, binding.userid());
                    }

                    for binding in self.user_attributes.iter_mut() {
                        check_one!("user attribute",
                                   binding.attestations, sig,
                                   verify_user_attribute_attestation,
                                   binding.user_attribute());
                    }
                },

                CertificationRevocation => {
                    for binding in self.userids.iter_mut() {
                        check_one!(format!("userid \"{}\"",
                                           String::from_utf8_lossy(
                                               binding.userid().value())),
                                   binding.self_revocations, sig,
                                   verify_userid_revocation,
                                   binding.userid());
                        check_one_3rd_party!(
                            format!("userid \"{}\"",
                                    String::from_utf8_lossy(
                                        binding.userid().value())),
                            binding.other_revocations, sig, lookup_fn,
                            verify_userid_revocation, hash_userid_binding,
                            binding.userid());
                    }

                    for binding in self.user_attributes.iter_mut() {
                        check_one!("user attribute",
                                   binding.self_revocations, sig,
                                   verify_user_attribute_revocation,
                                   binding.user_attribute());
                        check_one_3rd_party!(
                            "user attribute",
                            binding.other_revocations, sig, lookup_fn,
                            verify_user_attribute_revocation,
                            hash_user_attribute_binding,
                            binding.user_attribute());
                    }
                },

                SubkeyBinding => {
                    for binding in self.subkeys.iter_mut() {
                        check_one!(format!("subkey {}", binding.key().keyid()),
                                   binding.self_signatures, sig,
                                   verify_subkey_binding, binding.key());
                        check_one_3rd_party!(
                            format!("subkey {}", binding.key().keyid()),
                            binding.certifications, sig, lookup_fn,
                            verify_subkey_binding, hash_subkey_binding,
                            binding.key());
                    }
                },

                SubkeyRevocation => {
                    for binding in self.subkeys.iter_mut() {
                        check_one!(format!("subkey {}", binding.key().keyid()),
                                   binding.self_revocations, sig,
                                   verify_subkey_revocation, binding.key());
                        check_one_3rd_party!(
                            format!("subkey {}", binding.key().keyid()),
                            binding.other_revocations, sig, lookup_fn,
                            verify_subkey_revocation, hash_subkey_binding,
                            binding.key());
                    }
                },

                typ => {
                    t!("Odd signature type: {:?}", typ);
                },
            }

            if found_component {
                continue;
            }

            // Keep them for later.
            t!("{} {:02X}{:02X}, {:?} doesn't belong \
                to any known component or is bad.",
               if is_selfsig { "Self-sig" } else { "3rd-party-sig" },
               sig.digest_prefix()[0], sig.digest_prefix()[1],
               sig.typ());

            if let Some(i) = unknown_idx {
                self.unknowns[i].certifications.push(sig);
            } else {
                self.bad.push(sig);
            }
        }

        if !self.bad.is_empty() {
            t!("{}: ignoring {} bad self signatures",
               self.keyid(), self.bad.len());
        }

        // Split signatures on unknown components.
        let primary_fp: KeyHandle = self.key_handle();
        let primary_keyid = KeyHandle::KeyID(primary_fp.clone().into());
        for c in self.unknowns.iter_mut() {
            parser::split_sigs(&primary_fp, &primary_keyid, c);
        }

        // Sort again.  We may have moved signatures to the right
        // component, and we need to ensure they are in the right spot
        // (i.e. newest first).
        self.sort_and_dedup();

        // XXX: Check if the sigs in other_sigs issuer are actually
        // designated revokers for this key (listed in a "Revocation
        // Key" subpacket in *any* non-revoked self signature).  Only
        // if that is the case should a sig be considered a potential
        // revocation.  (This applies to
        // self.primary_other_revocations as well as
        // self.userids().other_revocations, etc.)  If not, put the
        // sig on the bad list.
        //
        // Note: just because the Cert doesn't indicate that a key is a
        // designed revoker doesn't mean that it isn't---we might just
        // be missing the signature.  In other words, this is a policy
        // decision, but given how easy it could be to create rogue
        // revocations, is probably the better to reject such
        // signatures than to keep them around and have many keys
        // being shown as "potentially revoked".

        // XXX Do some more canonicalization.

        self
    }

    /// Returns the certificate's fingerprint as a `KeyHandle`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::KeyHandle;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// #
    /// println!("{}", cert.key_handle());
    ///
    /// // This always returns a fingerprint.
    /// match cert.key_handle() {
    ///     KeyHandle::Fingerprint(_) => (),
    ///     KeyHandle::KeyID(_) => unreachable!(),
    /// }
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn key_handle(&self) -> KeyHandle {
        self.primary.key().key_handle()
    }

    /// Returns the certificate's fingerprint.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// #
    /// println!("{}", cert.fingerprint());
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn fingerprint(&self) -> Fingerprint {
        self.primary.key().fingerprint()
    }

    /// Returns the certificate's Key ID.
    ///
    /// As a general rule of thumb, you should prefer the fingerprint
    /// as it is possible to create keys with a colliding Key ID using
    /// a [birthday attack].
    ///
    /// [birthday attack]: https://nullprogram.com/blog/2019/07/22/
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// #
    /// println!("{}", cert.keyid());
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn keyid(&self) -> KeyID {
        self.primary.key().keyid()
    }

    /// Merges `other` into `self`, ignoring secret key material in
    /// `other`.
    ///
    /// If `other` is a different certificate, then an error is
    /// returned.
    ///
    /// This routine merges duplicate packets.  This is different from
    /// [`Cert::insert_packets`], which prefers keys in the packets that
    /// are being merged into the certificate.
    ///
    /// [`Cert::insert_packets`]: Cert::insert_packets()
    ///
    /// This function is appropriate to merge certificate material
    /// from untrusted sources like keyservers.  If `other` contains
    /// secret key material, it is ignored.  See
    /// [`Cert::merge_public_and_secret`] on how to merge certificates
    /// containing secret key material from trusted sources.
    ///
    /// [`Cert::merge_public_and_secret`]: Cert::merge_public_and_secret()
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (local, _) =
    /// #       CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #       .generate()?;
    /// # let keyserver = local.clone();
    /// // Merge the local version with the version from the keyserver.
    /// let cert = local.merge_public(keyserver)?;
    /// # let _ = cert;
    /// # Ok(()) }
    /// ```
    pub fn merge_public(self, other: Cert) -> Result<Self> {
        // Strip all secrets from `other`.
        let other_public = other.strip_secret_key_material();
        // Then merge it.
        self.merge_public_and_secret(other_public)
    }

    /// Merges `other` into `self`, including secret key material.
    ///
    /// If `other` is a different certificate, then an error is
    /// returned.
    ///
    /// This routine merges duplicate packets.  If the same (sub)key
    /// is present in both `self` and `other`, then
    ///
    ///   - if both keys have secret key material, then the version in
    ///     `other` is preferred,
    ///
    ///   - if only one key has secret key material, then this copy is
    ///     preferred.
    ///
    /// This is different from [`Cert::insert_packets`], which
    /// unconditionally prefers keys in the packets that are being
    /// merged into the certificate.
    ///
    /// It is important to only merge key material from trusted
    /// sources using this function, because it may be used to import
    /// secret key material.  Secret key material is not authenticated
    /// by OpenPGP, and there are plausible attack scenarios where a
    /// malicious actor injects secret key material.
    ///
    /// To merge only public key material, which is always safe, use
    /// [`Cert::merge_public`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (local, _) =
    /// #       CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #       .generate()?;
    /// # let other_device = local.clone();
    /// // Merge the local version with the version from your other device.
    /// let cert = local.merge_public_and_secret(other_device)?;
    /// # let _ = cert;
    /// # Ok(()) }
    /// ```
    pub fn merge_public_and_secret(mut self, mut other: Cert) -> Result<Self> {
        if self.fingerprint() != other.fingerprint() {
            // The primary key is not the same.  There is nothing to
            // do.
            return Err(Error::InvalidArgument(
                "Primary key mismatch".into()).into());
        }

        // Prefer the secret in `other`.
        if other.primary.key().has_secret() {
            std::mem::swap(self.primary.key_mut(), other.primary.key_mut());
        }

        self.primary.self_signatures.append(
            &mut other.primary.self_signatures);
        self.primary.attestations.append(
            &mut other.primary.attestations);
        self.primary.certifications.append(
            &mut other.primary.certifications);
        self.primary.self_revocations.append(
            &mut other.primary.self_revocations);
        self.primary.other_revocations.append(
            &mut other.primary.other_revocations);

        self.userids.append(&mut other.userids);
        self.user_attributes.append(&mut other.user_attributes);
        self.subkeys.append(&mut other.subkeys);
        self.bad.append(&mut other.bad);

        Ok(self.canonicalize())
    }

    // Returns whether the specified packet is a valid start of a
    // certificate.
    fn valid_start<T>(tag: T) -> Result<()>
        where T: Into<Tag>
    {
        let tag = tag.into();
        match tag {
            Tag::SecretKey | Tag::PublicKey => Ok(()),
            _ => Err(Error::MalformedCert(
                format!("A certificate does not start with a {}",
                        tag)).into()),
        }
    }

    // Returns whether the specified packet can occur in a
    // certificate.
    //
    // This function rejects all packets that are known to not belong
    // in a certificate.  It conservatively accepts unknown packets
    // based on the assumption that they are some new component type
    // from the future.
    fn valid_packet<T>(tag: T) -> Result<()>
        where T: Into<Tag>
    {
        let tag = tag.into();
        match tag {
            // Packets that definitely don't belong in a certificate.
            Tag::Reserved
                | Tag::PKESK
                | Tag::SKESK
                | Tag::OnePassSig
                | Tag::CompressedData
                | Tag::SED
                | Tag::Literal
                | Tag::SEIP
                | Tag::MDC
                | Tag::AED =>
            {
                Err(Error::MalformedCert(
                    format!("A certificate cannot not include a {}",
                            tag)).into())
            }
            // The rest either definitely belong in a certificate or
            // are unknown (and conservatively accepted for future
            // compatibility).
            _ => Ok(()),
        }
    }

    /// Adds packets to the certificate.
    ///
    /// This function turns the certificate into a sequence of
    /// packets, appends the packets to the end of it, and
    /// canonicalizes the result.  [Known packets that don't belong in
    /// a TPK or TSK] cause this function to return an error.  Unknown
    /// packets are retained and added to the list of [unknown
    /// components].  The goal is to provide some future
    /// compatibility.
    ///
    /// If a key is merged that already exists in the certificate, it
    /// replaces the existing key.  This way, secret key material can
    /// be added, removed, encrypted, or decrypted.
    ///
    /// Similarly, if a signature is merged that already exists in the
    /// certificate, it replaces the existing signature.  This way,
    /// the unhashed subpacket area can be updated.
    ///
    /// On success, this function returns the certificate with the
    /// packets merged in, and a boolean indicating whether the
    /// certificate actually changed.  Changed here means that at
    /// least one new packet was added, or an existing packet was
    /// updated.  Alternatively, changed means that the serialized
    /// form has changed.
    ///
    /// [Known packets that don't belong in a TPK or TSK]: https://tools.ietf.org/html/rfc4880#section-11
    /// [unknown components]: Cert::unknowns()
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::serialize::Serialize;
    /// use openpgp::parse::Parse;
    /// use openpgp::types::DataFormat;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// // Create a new key.
    /// let (cert, rev) =
    ///       CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///       .generate()?;
    /// assert!(cert.is_tsk());
    ///
    ///
    /// // Merging in the certificate doesn't change it.
    /// let identical_cert = cert.clone();
    /// let (cert, changed) =
    ///     cert.insert_packets2(identical_cert.into_packets())?;
    /// assert!(! changed);
    ///
    ///
    /// // Merge in the revocation certificate.
    /// assert_eq!(cert.primary_key().self_revocations().count(), 0);
    /// let (cert, changed) = cert.insert_packets2(rev)?;
    /// assert!(changed);
    /// assert_eq!(cert.primary_key().self_revocations().count(), 1);
    ///
    ///
    /// // Add an unknown packet.
    /// let tag = Tag::Private(61.into());
    /// let unknown = Unknown::new(tag,
    ///     openpgp::Error::UnsupportedPacketType(tag).into());
    ///
    /// // It shows up as an unknown component.
    /// let (cert, changed) = cert.insert_packets2(unknown)?;
    /// assert!(changed);
    /// assert_eq!(cert.unknowns().count(), 1);
    /// for p in cert.unknowns() {
    ///     assert_eq!(p.tag(), tag);
    /// }
    ///
    ///
    /// // Try and merge a literal data packet.
    /// let mut lit = Literal::new(DataFormat::Text);
    /// lit.set_body(b"test".to_vec());
    ///
    /// // Merging packets that are known to not belong to a
    /// // certificate result in an error.
    /// assert!(cert.insert_packets(lit).is_err());
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// Remove secret key material:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// // Create a new key.
    /// let (cert, _) =
    ///       CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///       .generate()?;
    /// assert!(cert.is_tsk());
    ///
    /// // We just created the key, so all of the keys have secret key
    /// // material.
    /// let mut pk = cert.primary_key().key().clone();
    ///
    /// // Split off the secret key material.
    /// let (pk, sk) = pk.take_secret();
    /// assert!(sk.is_some());
    /// assert!(! pk.has_secret());
    ///
    /// // Merge in the public key.  Recall: the packets that are
    /// // being merged into the certificate take precedence.
    /// let (cert, changed) = cert.insert_packets2(pk)?;
    /// assert!(changed);
    ///
    /// // The secret key material is stripped.
    /// assert!(! cert.primary_key().has_secret());
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// Update a binding signature's unhashed subpacket area:
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::packet::signature::subpacket::*;
    ///
    /// // Create a new key.
    /// let (cert, _) =
    ///       CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///       .generate()?;
    /// assert_eq!(cert.userids().nth(0).unwrap().self_signatures().count(), 1);
    ///
    /// // Grab the binding signature so that we can modify it.
    /// let mut sig =
    ///     cert.userids().nth(0).unwrap().self_signatures().nth(0)
    ///     .unwrap().clone();
    ///
    /// // Add a notation subpacket.  Note that the information is not
    /// // authenticated, therefore it may only be trusted if the
    /// // certificate with the signature is placed in a trusted store.
    /// let notation = NotationData::new("retrieved-from@example.org",
    ///                                  "generated-locally",
    ///                                  NotationDataFlags::empty()
    ///                                      .set_human_readable());
    /// sig.unhashed_area_mut().add(
    ///     Subpacket::new(SubpacketValue::NotationData(notation), false)?)?;
    ///
    /// // Merge in the signature.  Recall: the packets that are
    /// // being merged into the certificate take precedence.
    /// let (cert, changed) = cert.insert_packets2(sig)?;
    /// assert!(changed);
    ///
    /// // The old binding signature is replaced.
    /// assert_eq!(cert.userids().nth(0).unwrap().self_signatures().count(), 1);
    /// assert_eq!(cert.userids().nth(0).unwrap().self_signatures().nth(0)
    ///                .unwrap()
    ///                .unhashed_area()
    ///                .subpackets(SubpacketTag::NotationData).count(), 1);
    /// # Ok(()) }
    /// ```
    pub fn insert_packets2<I>(self, packets: I)
        -> Result<(Self, bool)>
        where I: IntoIterator,
              I::Item: Into<Packet>,
    {
        self.insert_packets_merge(packets, |_old, new| Ok(new))
    }

    /// Adds packets to the certificate with an explicit merge policy.
    ///
    /// Like [`Cert::insert_packets2`], but also takes a function that
    /// will be called on inserts and replacements that can be used to
    /// log changes to the certificate, and to influence how packets
    /// are merged.  The merge function takes two parameters, an
    /// optional existing packet, and the packet to be merged in.
    ///
    /// If a new packet is inserted, there is no packet currently in
    /// the certificate.  Hence, the first parameter to the merge
    /// function is `None`.
    ///
    /// If an existing packet is updated, there is a packet currently
    /// in the certificate that matches the given packet.  Hence, the
    /// first parameter to the merge function is
    /// `Some(existing_packet)`.
    ///
    /// Both packets given to the merge function are considered equal
    /// when considering the normalized form (only comparing public
    /// key parameters and ignoring unhashed signature subpackets, see
    /// [`Packet::normalized_hash`]).  It must return a packet that
    /// equals the input packet.  In practice that means that the
    /// merge function returns either the old packet, the new packet,
    /// or a combination of both packets.  If the merge function
    /// returns a different packet, this function returns
    /// `Error::InvalidOperation`.
    ///
    /// If the merge function returns the existing packet, this
    /// function will still consider this as a change to the
    /// certificate.  In other words, it may return that the
    /// certificate has changed even if the serialized representation
    /// has not changed.
    ///
    /// # Examples
    ///
    /// In the first example, we give an explicit merge function that
    /// just returns the new packet.  This policy prefers the new
    /// packet.  This is the policy used by [`Cert::insert_packets2`].
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::crypto::Password;
    /// use openpgp::cert::prelude::CertBuilder;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p0 = Password::from("old password");
    /// let p1 = Password::from("new password");
    ///
    /// // Create a new key.
    /// let (cert, rev) =
    ///       CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///       .set_password(Some(p0.clone()))
    ///       .generate()?;
    /// assert!(cert.is_tsk());
    ///
    /// // Change the password for the primary key.
    /// let pk = cert.primary_key().key().clone().parts_into_secret()?
    ///     .decrypt_secret(&p0)?
    ///     .encrypt_secret(&p1)?;
    ///
    /// // Merge it back in, with a policy projecting to the new packet.
    /// let (cert, changed) =
    ///     cert.insert_packets_merge(pk, |_old, new| Ok(new))?;
    /// assert!(changed);
    ///
    /// // Make sure we can still decrypt the primary key using the
    /// // new password.
    /// assert!(cert.primary_key().key().clone().parts_into_secret()?
    ///         .decrypt_secret(&p1).is_ok());
    /// # Ok(()) }
    /// ```
    ///
    /// In the second example, we give an explicit merge function that
    /// returns the old packet if given, falling back to the new
    /// packet, if not.  This policy prefers the existing packets.
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::crypto::Password;
    /// use openpgp::cert::prelude::CertBuilder;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p0 = Password::from("old password");
    /// let p1 = Password::from("new password");
    ///
    /// // Create a new key.
    /// let (cert, rev) =
    ///       CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///       .set_password(Some(p0.clone()))
    ///       .generate()?;
    /// assert!(cert.is_tsk());
    ///
    /// // Change the password for the primary key.
    /// let pk = cert.primary_key().key().clone().parts_into_secret()?
    ///     .decrypt_secret(&p0)?
    ///     .encrypt_secret(&p1)?;
    ///
    /// // Merge it back in, with a policy preferring to the old packet.
    /// let (cert, changed) =
    ///     cert.insert_packets_merge(pk, |old, new| Ok(old.unwrap_or(new)))?;
    /// assert!(changed); // Overestimates changes.
    ///
    /// // Make sure we can still decrypt the primary key using the
    /// // old password.
    /// assert!(cert.primary_key().key().clone().parts_into_secret()?
    ///         .decrypt_secret(&p0).is_ok());
    /// # Ok(()) }
    /// ```
    pub fn insert_packets_merge<P, I>(self, packets: P, merge: I)
        -> Result<(Self, bool)>
        where P: IntoIterator,
              P::Item: Into<Packet>,
              I: FnMut(Option<Packet>, Packet) -> Result<Packet>,
    {
        self.insert_packets_(&mut packets.into_iter().map(Into::into),
                             Box::new(merge))
    }

    /// Adds packets to the certificate with an explicit merge policy.
    ///
    /// This implements all the Cert::insert_packets* functions.  Its
    /// arguments `packets` and `merge` use dynamic dispatch so that
    /// we avoid the cost of monomorphization.
    fn insert_packets_<'a>(self,
                           packets: &mut dyn Iterator<Item = Packet>,
                           mut merge: Box<dyn FnMut(Option<Packet>, Packet)
                                                    -> Result<Packet> + 'a>)
        -> Result<(Self, bool)>
    {
        let mut changed = false;
        let mut combined = self.into_packets().collect::<Vec<_>>();

        // Hashes a packet ignoring the unhashed subpacket area and
        // any secret key material.
        let hash_packet = |p: &Packet| -> u64 {
            let mut hasher = DefaultHasher::new();
            p.normalized_hash(&mut hasher);
            hasher.finish()
        };

        // BTreeMap of (hash) -> Vec<index in combined>.
        //
        // We don't use a HashMap, because the key would be a
        // reference to the packets in combined, which would prevent
        // us from modifying combined.
        //
        // Note: we really don't want to dedup components now, because
        // we want to keep signatures immediately after their
        // components.
        let mut packet_map: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
        for (i, p) in combined.iter().enumerate() {
            match packet_map.entry(hash_packet(p)) {
                Entry::Occupied(mut oe) => {
                    oe.get_mut().push(i)
                }
                Entry::Vacant(ve) => {
                    ve.insert(vec![ i ]);
                }
            }
        }

        enum Action {
            Drop,
            Overwrite(usize),
            Insert,
        }
        use Action::*;

        // Now we merge in the new packets.
        for p in packets {
            Cert::valid_packet(&p)?;

            let hash = hash_packet(&p);
            let mut action = Insert;
            if let Some(combined_i) = packet_map.get(&hash) {
                for i in combined_i {
                    let i: usize = *i;
                    let (same, identical) = match (&p, &combined[i]) {
                        // For keys, only compare the public bits.  If
                        // they match, then we keep whatever is in the
                        // new key.
                        (Packet::PublicKey(a), Packet::PublicKey(b)) =>
                            (a.public_cmp(b) == Ordering::Equal,
                             a == b),
                        (Packet::SecretKey(a), Packet::SecretKey(b)) =>
                            (a.public_cmp(b) == Ordering::Equal,
                             a == b),
                        (Packet::PublicKey(a), Packet::SecretKey(b)) =>
                            (a.public_cmp(b) == Ordering::Equal,
                             false),
                        (Packet::SecretKey(a), Packet::PublicKey(b)) =>
                            (a.public_cmp(b) == Ordering::Equal,
                             false),

                        (Packet::PublicSubkey(a), Packet::PublicSubkey(b)) =>
                            (a.public_cmp(b) == Ordering::Equal,
                             a == b),
                        (Packet::SecretSubkey(a), Packet::SecretSubkey(b)) =>
                            (a.public_cmp(b) == Ordering::Equal,
                             a == b),
                        (Packet::PublicSubkey(a), Packet::SecretSubkey(b)) =>
                            (a.public_cmp(b) == Ordering::Equal,
                             false),
                        (Packet::SecretSubkey(a), Packet::PublicSubkey(b)) =>
                            (a.public_cmp(b) == Ordering::Equal,
                             false),

                        // For signatures, don't compare the unhashed
                        // subpacket areas.  If it's the same
                        // signature, then we keep what is the new
                        // signature's unhashed subpacket area.
                        (Packet::Signature(a), Packet::Signature(b)) =>
                            (a.normalized_eq(b),
                             a == b),

                        (a, b) => {
                            let identical = a == b;
                            (identical, identical)
                        }
                    };

                    if same {
                        if identical {
                            action = Drop;
                        } else {
                            action = Overwrite(i);
                        }
                        break;
                    }
                }
            }

            match action {
                Drop => (),
                Overwrite(i) => {
                    // Existing packet.
                    let existing =
                        std::mem::replace(&mut combined[i],
                                          Packet::Marker(Default::default()));
                    let merged = merge(Some(existing), p)?;
                    let merged_hash = hash_packet(&merged);
                    if hash != merged_hash {
                        return Err(Error::InvalidOperation(
                            format!("merge function changed packet hash \
                                     (expected: {}, got: {})",
                                    hash, merged_hash)).into());
                    }

                    combined[i] = merged;
                    changed = true;
                },
                Insert => {
                    // New packet.
                    let merged = merge(None, p)?;
                    let merged_hash = hash_packet(&merged);
                    if hash != merged_hash {
                        return Err(Error::InvalidOperation(
                            format!("merge function changed packet hash \
                                     (expected: {}, got: {})",
                                    hash, merged_hash)).into());
                    }

                    // Add it to combined.
                    combined.push(merged);
                    changed = true;

                    // Because the caller might insert the same packet
                    // multiple times, we need to also add it to
                    // packet_map.
                    let i = combined.len() - 1;
                    match packet_map.entry(hash) {
                        Entry::Occupied(mut oe) => {
                            oe.get_mut().push(i)
                        }
                        Entry::Vacant(ve) => {
                            ve.insert(vec![ i ]);
                        }
                    }
                }
            }
        }

        Cert::try_from(combined).map(|cert| (cert, changed))
    }

    /// Adds packets to the certificate.
    ///
    /// Like [`Cert::insert_packets2`], but does not return whether
    /// the certificate changed.
    pub fn insert_packets<I>(self, packets: I)
        -> Result<Self>
        where I: IntoIterator,
              I::Item: Into<Packet>,
    {
        self.insert_packets2(packets).map(|(cert, _)| cert)
    }

    /// Returns whether at least one of the keys includes secret
    /// key material.
    ///
    /// This returns true if either the primary key or at least one of
    /// the subkeys includes secret key material.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::serialize::Serialize;
    /// use openpgp::parse::Parse;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// // Create a new key.
    /// let (cert, _) =
    ///       CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///       .generate()?;
    /// assert!(cert.is_tsk());
    ///
    /// // If we serialize the certificate, the secret key material is
    /// // stripped, unless we first convert it to a TSK.
    ///
    /// let mut buffer = Vec::new();
    /// cert.as_tsk().serialize(&mut buffer);
    /// let cert = Cert::from_bytes(&buffer)?;
    /// assert!(cert.is_tsk());
    ///
    /// // Now round trip it without first converting it to a TSK.  This
    /// // drops the secret key material.
    /// let mut buffer = Vec::new();
    /// cert.serialize(&mut buffer);
    /// let cert = Cert::from_bytes(&buffer)?;
    /// assert!(!cert.is_tsk());
    /// #     Ok(())
    /// # }
    /// ```
    pub fn is_tsk(&self) -> bool {
        if self.primary_key().has_secret() {
            return true;
        }
        self.subkeys().any(|sk| {
            sk.key().has_secret()
        })
    }

    /// Strips any secret key material.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> openpgp::Result<()> {
    ///
    /// // Create a new key.
    /// let (cert, _) =
    ///       CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///       .generate()?;
    /// assert!(cert.is_tsk());
    ///
    /// let cert = cert.strip_secret_key_material();
    /// assert!(! cert.is_tsk());
    /// #     Ok(())
    /// # }
    /// ```
    pub fn strip_secret_key_material(mut self) -> Cert {
        let (pk, _sk) = self.primary.component.take_secret();
        self.primary.component = pk;

        let subkeys = self.subkeys.into_iter()
            .map(|mut kb| {
                let (pk, _sk) = kb.component.take_secret();
                kb.component = pk;
                kb
            })
            .collect::<Vec<_>>();
        self.subkeys = ComponentBundles { bundles: subkeys, };

        self
    }

    /// Retains only the userids specified by the predicate.
    ///
    /// Removes all the userids for which the given predicate returns
    /// false.
    ///
    /// # Warning
    ///
    /// Because userid binding signatures are traditionally used to
    /// provide additional information like the certificate holder's
    /// algorithm preferences (see [`Preferences`]) and primary key
    /// flags (see [`ValidKeyAmalgamation::key_flags`]).  Removing a
    /// userid may inadvertently change this information.
    ///
    ///   [`ValidKeyAmalgamation::key_flags`]: amalgamation::key::ValidKeyAmalgamation::key_flags()
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    ///
    /// // Create a new key.
    /// let (cert, _) =
    ///       CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///       .add_userid("Alice Lovelace <alice@lovelace.name>")
    ///       .generate()?;
    /// assert_eq!(cert.userids().count(), 2);
    ///
    /// let cert = cert.retain_userids(|ua| {
    ///     if let Ok(Some(address)) = ua.email() {
    ///         address == "alice@example.org" // Only keep this one.
    ///     } else {
    ///         false                          // Drop malformed userids.
    ///     }
    /// });
    /// assert_eq!(cert.userids().count(), 1);
    /// assert_eq!(cert.userids().nth(0).unwrap().email()?.unwrap(),
    ///            "alice@example.org");
    /// # Ok(()) }
    /// ```
    pub fn retain_userids<P>(mut self, mut predicate: P) -> Cert
        where P: FnMut(UserIDAmalgamation) -> bool,
    {
        let mut keep = vec![false; self.userids.len()];
        for (i, a) in self.userids().enumerate() {
            keep[i] = predicate(a);
        }
        // Note: Vec::retain visits the elements in the original
        // order.
        let mut keep = keep.iter();
        self.userids.retain(|_| *keep.next().unwrap());
        self
    }

    /// Retains only the user attributes specified by the predicate.
    ///
    /// Removes all the user attributes for which the given predicate
    /// returns false.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    ///
    /// // Create a new key.
    /// let (cert, _) =
    ///       CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///       // Add nonsensical user attribute.
    ///       .add_user_attribute(vec![0, 1, 2])
    ///       .generate()?;
    /// assert_eq!(cert.user_attributes().count(), 1);
    ///
    /// // Strip all user attributes
    /// let cert = cert.retain_user_attributes(|_| false);
    /// assert_eq!(cert.user_attributes().count(), 0);
    /// # Ok(()) }
    /// ```
    pub fn retain_user_attributes<P>(mut self, mut predicate: P) -> Cert
        where P: FnMut(UserAttributeAmalgamation) -> bool,
    {
        let mut keep = vec![false; self.user_attributes.len()];
        for (i, a) in self.user_attributes().enumerate() {
            keep[i] = predicate(a);
        }
        // Note: Vec::retain visits the elements in the original
        // order.
        let mut keep = keep.iter();
        self.user_attributes.retain(|_| *keep.next().unwrap());
        self
    }

    /// Retains only the subkeys specified by the predicate.
    ///
    /// Removes all the subkeys for which the given predicate returns
    /// false.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::cert::prelude::*;
    ///
    /// // Create a new key.
    /// let (cert, _) =
    ///       CertBuilder::new()
    ///       .add_userid("Alice Lovelace <alice@lovelace.name>")
    ///       .add_transport_encryption_subkey()
    ///       .add_storage_encryption_subkey()
    ///       .generate()?;
    /// assert_eq!(cert.keys().subkeys().count(), 2);
    ///
    /// // Retain only the transport encryption subkey.  For that, we
    /// // need to examine the key flags, therefore we need to turn
    /// // the `KeyAmalgamation` into a `ValidKeyAmalgamation` under a
    /// // policy.
    /// let p = &StandardPolicy::new();
    /// let cert = cert.retain_subkeys(|ka| {
    ///     if let Ok(vka) = ka.with_policy(p, None) {
    ///         vka.key_flags().map(|flags| flags.for_transport_encryption())
    ///             .unwrap_or(false)      // Keep transport encryption keys.
    ///     } else {
    ///         false                      // Drop unbound keys.
    ///     }
    /// });
    /// assert_eq!(cert.keys().subkeys().count(), 1);
    /// assert!(cert.with_policy(p, None)?.keys().subkeys().nth(0).unwrap()
    ///             .key_flags().unwrap().for_transport_encryption());
    /// # Ok(()) }
    /// ```
    pub fn retain_subkeys<P>(mut self, mut predicate: P) -> Cert
        where P: FnMut(SubordinateKeyAmalgamation<crate::packet::key::PublicParts>) -> bool,
    {
        let mut keep = vec![false; self.subkeys.len()];
        for (i, a) in self.keys().subkeys().enumerate() {
            keep[i] = predicate(a);
        }
        // Note: Vec::retain visits the elements in the original
        // order.
        let mut keep = keep.iter();
        self.subkeys.retain(|_| *keep.next().unwrap());
        self
    }

    /// Associates a policy and a reference time with the certificate.
    ///
    /// This is used to turn a `Cert` into a
    /// [`ValidCert`].  (See also [`ValidateAmalgamation`],
    /// which does the same for component amalgamations.)
    ///
    /// A certificate is considered valid if:
    ///
    ///   - It has a self signature that is live at time `t`.
    ///
    ///   - The policy considers it acceptable.
    ///
    /// This doesn't say anything about whether the certificate itself
    /// is alive (see [`ValidCert::alive`]) or revoked (see
    /// [`ValidCert::revocation_status`]).
    ///
    /// [`ValidateAmalgamation`]: amalgamation::ValidateAmalgamation
    /// [`ValidCert::alive`]: ValidCert::alive()
    /// [`ValidCert::revocation_status`]: ValidCert::revocation_status()
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// let vc = cert.with_policy(p, None)?;
    /// # assert!(std::ptr::eq(vc.policy(), p));
    /// #     Ok(())
    /// # }
    /// ```
    pub fn with_policy<'a, T>(&'a self, policy: &'a dyn Policy, time: T)
                              -> Result<ValidCert<'a>>
        where T: Into<Option<time::SystemTime>>,
    {
        let time = time.into().unwrap_or_else(crate::now);
        self.primary_key().with_policy(policy, time)?;

        Ok(ValidCert {
            cert: self,
            policy,
            time,
        })
    }
}

impl TryFrom<PacketParserResult<'_>> for Cert {
    type Error = anyhow::Error;

    /// Returns the Cert found in the packet stream.
    ///
    /// If the sequence contains multiple certificates (i.e., it is a
    /// keyring), or the certificate is followed by an invalid packet
    /// this function will fail.  To parse keyrings, use
    /// [`CertParser`] instead of this function.
    fn try_from(ppr: PacketParserResult) -> Result<Self> {
        let mut parser = parser::CertParser::from(ppr);
        if let Some(cert_result) = parser.next() {
            if parser.next().is_some() {
                Err(Error::MalformedCert(
                    "Additional packets found, is this a keyring?".into()
                ).into())
            } else {
                cert_result
            }
        } else {
            Err(Error::MalformedCert("No data".into()).into())
        }
    }
}

impl TryFrom<Vec<Packet>> for Cert {
    type Error = anyhow::Error;

    fn try_from(p: Vec<Packet>) -> Result<Self> {
        Cert::from_packets(p.into_iter())
    }
}

impl TryFrom<Packet> for Cert {
    type Error = anyhow::Error;

    fn try_from(p: Packet) -> Result<Self> {
        Cert::from_packets(std::iter::once(p))
    }
}

impl TryFrom<PacketPile> for Cert {
    type Error = anyhow::Error;

    /// Returns the certificate found in the `PacketPile`.
    ///
    /// If the [`PacketPile`] does not start with a certificate
    /// (specifically, if it does not start with a primary key
    /// packet), then this fails.
    ///
    /// If the sequence contains multiple certificates (i.e., it is a
    /// keyring), or the certificate is followed by an invalid packet
    /// this function will fail.  To parse keyrings, use
    /// [`CertParser`] instead of this function.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::PacketPile;
    /// use std::convert::TryFrom;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let (cert, rev) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///     .generate()?;
    ///
    /// // We should be able to turn a certificate into a PacketPile
    /// // and back.
    /// let pp : PacketPile = cert.into();
    /// assert!(Cert::try_from(pp).is_ok());
    ///
    /// // But a revocation certificate is not a certificate, so this
    /// // will fail.
    /// let pp : PacketPile = Packet::from(rev).into();
    /// assert!(Cert::try_from(pp).is_err());
    /// # Ok(())
    /// # }
    /// ```
    fn try_from(p: PacketPile) -> Result<Self> {
        Self::from_packets(p.into_children())
    }
}

impl From<Cert> for Vec<Packet> {
    fn from(cert: Cert) -> Self {
        cert.into_packets().collect::<Vec<_>>()
    }
}

/// An iterator that moves out of a `Cert`.
///
/// This structure is created by the `into_iter` method on [`Cert`]
/// (provided by the [`IntoIterator`] trait).
///
/// [`IntoIterator`]: std::iter::IntoIterator
// We can't use a generic type, and due to the use of closures, we
// can't write down the concrete type.  So, just use a Box.
pub struct IntoIter(Box<dyn Iterator<Item=Packet> + Send + Sync>);
assert_send_and_sync!(IntoIter);

impl Iterator for IntoIter {
    type Item = Packet;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl IntoIterator for Cert
{
    type Item = Packet;
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(Box::new(self.into_packets()))
    }
}

/// A `Cert` plus a `Policy` and a reference time.
///
/// A `ValidCert` combines a [`Cert`] with a [`Policy`] and a
/// reference time.  This allows it to implement methods that require
/// a `Policy` and a reference time without requiring the caller to
/// explicitly pass them in.  Embedding them in the `ValidCert` data
/// structure rather than having the caller pass them in explicitly
/// helps ensure that multipart operations, even those that span
/// multiple functions, use the same `Policy` and reference time.
/// This avoids a subtle class of bugs in which different views of a
/// certificate are unintentionally used.
///
/// A `ValidCert` is typically obtained by transforming a `Cert` using
/// [`Cert::with_policy`].
///
/// A `ValidCert` is guaranteed to have a valid and live binding
/// signature at the specified reference time.  Note: this only means
/// that the binding signature is live; it says nothing about whether
/// the certificate or any component is live.  If you care about those
/// things, then you need to check them separately.
///
/// [`Policy`]: crate::policy::Policy
/// [`Cert::with_policy`]: Cert::with_policy()
///
/// # Examples
///
/// ```
/// use sequoia_openpgp as openpgp;
/// # use openpgp::cert::prelude::*;
/// use openpgp::policy::StandardPolicy;
///
/// # fn main() -> openpgp::Result<()> {
/// let p = &StandardPolicy::new();
///
/// # let (cert, _) = CertBuilder::new()
/// #     .add_userid("Alice")
/// #     .add_signing_subkey()
/// #     .add_transport_encryption_subkey()
/// #     .generate()?;
/// let vc = cert.with_policy(p, None)?;
/// # assert!(std::ptr::eq(vc.policy(), p));
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
pub struct ValidCert<'a> {
    cert: &'a Cert,
    policy: &'a dyn Policy,
    // The reference time.
    time: time::SystemTime,
}
assert_send_and_sync!(ValidCert<'_>);

impl<'a> std::ops::Deref for ValidCert<'a> {
    type Target = Cert;

    fn deref(&self) -> &Self::Target {
        self.cert
    }
}

impl<'a> fmt::Display for ValidCert<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.fingerprint())
    }
}

impl<'a> ValidCert<'a> {
    /// Returns the underlying certificate.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// # let (cert, _) = CertBuilder::new()
    /// #     .add_userid("Alice")
    /// #     .add_signing_subkey()
    /// #     .add_transport_encryption_subkey()
    /// #     .generate()?;
    /// let vc = cert.with_policy(p, None)?;
    /// assert!(std::ptr::eq(vc.cert(), &cert));
    /// # assert!(std::ptr::eq(vc.policy(), p));
    /// # Ok(()) }
    /// ```
    pub fn cert(&self) -> &'a Cert {
        self.cert
    }

    /// Returns the associated reference time.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::time::{SystemTime, Duration, UNIX_EPOCH};
    /// #
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let t = UNIX_EPOCH + Duration::from_secs(1307732220);
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .set_creation_time(t)
    /// #         .generate()?;
    /// let vc = cert.with_policy(p, t)?;
    /// assert_eq!(vc.time(), t);
    /// #     Ok(())
    /// # }
    /// ```
    pub fn time(&self) -> time::SystemTime {
        self.time
    }

    /// Returns the associated policy.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// let vc = cert.with_policy(p, None)?;
    /// assert!(std::ptr::eq(vc.policy(), p));
    /// #     Ok(())
    /// # }
    /// ```
    pub fn policy(&self) -> &'a dyn Policy {
        self.policy
    }

    /// Changes the associated policy and reference time.
    ///
    /// If `time` is `None`, the current time is used.
    ///
    /// Returns an error if the certificate is not valid for the given
    /// policy at the specified time.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::{StandardPolicy, NullPolicy};
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let sp = &StandardPolicy::new();
    /// let np = &NullPolicy::new();
    ///
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// let vc = cert.with_policy(sp, None)?;
    ///
    /// // ...
    ///
    /// // Now with a different policy.
    /// let vc = vc.with_policy(np, None)?;
    /// #     Ok(())
    /// # }
    /// ```
    pub fn with_policy<T>(self, policy: &'a dyn Policy, time: T)
        -> Result<ValidCert<'a>>
        where T: Into<Option<time::SystemTime>>,
    {
        self.cert.with_policy(policy, time)
    }

    /// Returns the certificate's direct key signature as of the
    /// reference time.
    ///
    /// Subpackets on direct key signatures apply to all components of
    /// the certificate, cf. [Section 5.2.3.3 of RFC 4880].
    ///
    /// [Section 5.2.3.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.3
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use sequoia_openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// # let (cert, _) = CertBuilder::new()
    /// #     .add_userid("Alice")
    /// #     .add_signing_subkey()
    /// #     .add_transport_encryption_subkey()
    /// #     .generate()?;
    /// let vc = cert.with_policy(p, None)?;
    /// println!("{:?}", vc.direct_key_signature());
    /// # assert!(vc.direct_key_signature().is_ok());
    /// # Ok(()) }
    /// ```
    pub fn direct_key_signature(&self) -> Result<&'a Signature>
    {
        self.cert.primary.binding_signature(self.policy(), self.time())
    }

    /// Returns the certificate's revocation status.
    ///
    /// A certificate is considered revoked at time `t` if:
    ///
    ///   - There is a valid and live revocation at time `t` that is
    ///     newer than all valid and live self signatures at time `t`,
    ///     or
    ///
    ///   - There is a valid [hard revocation] (even if it is not live
    ///     at time `t`, and even if there is a newer self signature).
    ///
    /// [hard revocation]: crate::types::RevocationType::Hard
    ///
    /// Note: certificates and subkeys have different revocation
    /// criteria from [User IDs] and [User Attributes].
    ///
    //  Pending https://github.com/rust-lang/rust/issues/85960, should be
    //  [User IDs]: bundle::ComponentBundle<UserID>::revocation_status
    //  [User Attributes]: bundle::ComponentBundle<UserAttribute>::revocation_status
    /// [User IDs]: bundle::ComponentBundle#method.revocation_status-1
    /// [User Attributes]: bundle::ComponentBundle#method.revocation_status-2
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::types::RevocationStatus;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, rev) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///     .generate()?;
    ///
    /// // Not revoked.
    /// assert_eq!(cert.with_policy(p, None)?.revocation_status(),
    ///            RevocationStatus::NotAsFarAsWeKnow);
    ///
    /// // Merge the revocation certificate.  `cert` is now considered
    /// // to be revoked.
    /// let cert = cert.insert_packets(rev.clone())?;
    /// assert_eq!(cert.with_policy(p, None)?.revocation_status(),
    ///            RevocationStatus::Revoked(vec![&rev.into()]));
    /// #     Ok(())
    /// # }
    /// ```
    pub fn revocation_status(&self) -> RevocationStatus<'a> {
        self.cert.revocation_status(self.policy, self.time)
    }

    /// Returns whether or not the certificate is alive at the
    /// reference time.
    ///
    /// A certificate is considered to be alive at time `t` if the
    /// primary key is alive at time `t`.
    ///
    /// A valid certificate's primary key is guaranteed to have [a live
    /// binding signature], however, that does not mean that the
    /// [primary key is necessarily alive].
    ///
    /// [a live binding signature]: amalgamation::ValidateAmalgamation
    /// [primary key is necessarily alive]: amalgamation::key::ValidKeyAmalgamation::alive()
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time;
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let a_second = time::Duration::from_secs(1);
    ///
    /// let creation_time = time::SystemTime::now();
    /// let before_creation = creation_time - a_second;
    /// let validity_period = 60 * a_second;
    /// let expiration_time = creation_time + validity_period;
    /// let before_expiration_time = expiration_time - a_second;
    /// let after_expiration_time = expiration_time + a_second;
    ///
    /// let (cert, _) = CertBuilder::new()
    ///     .add_userid("Alice")
    ///     .set_creation_time(creation_time)
    ///     .set_validity_period(validity_period)
    ///     .generate()?;
    ///
    /// // There is no binding signature before the certificate was created.
    /// assert!(cert.with_policy(p, before_creation).is_err());
    /// assert!(cert.with_policy(p, creation_time)?.alive().is_ok());
    /// assert!(cert.with_policy(p, before_expiration_time)?.alive().is_ok());
    /// // The binding signature is still alive, but the key has expired.
    /// assert!(cert.with_policy(p, expiration_time)?.alive().is_err());
    /// assert!(cert.with_policy(p, after_expiration_time)?.alive().is_err());
    /// # Ok(()) }
    pub fn alive(&self) -> Result<()> {
        self.primary_key().alive()
    }

    /// Returns the certificate's primary key.
    ///
    /// A key's secret key material may be protected with a
    /// password.  In such cases, it needs to be decrypted before it
    /// can be used to decrypt data or generate a signature.  Refer to
    /// [`Key::decrypt_secret`] for details.
    ///
    /// [`Key::decrypt_secret`]: crate::packet::Key::decrypt_secret()
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::policy::StandardPolicy;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let p = &StandardPolicy::new();
    /// # let (cert, _) = CertBuilder::new()
    /// #     .add_userid("Alice")
    /// #     .generate()?;
    /// # let vc = cert.with_policy(p, None)?;
    /// #
    /// let primary = vc.primary_key();
    /// // The certificate's fingerprint *is* the primary key's fingerprint.
    /// assert_eq!(vc.fingerprint(), primary.fingerprint());
    /// # Ok(()) }
    pub fn primary_key(&self)
        -> ValidPrimaryKeyAmalgamation<'a, key::PublicParts>
    {
        self.cert.primary_key().with_policy(self.policy, self.time)
            .expect("A ValidKeyAmalgamation must have a ValidPrimaryKeyAmalgamation")
    }

    /// Returns an iterator over the certificate's valid keys.
    ///
    /// That is, this returns an iterator over the primary key and any
    /// subkeys.
    ///
    /// The iterator always returns the primary key first.  The order
    /// of the subkeys is undefined.
    ///
    /// To only iterate over the certificate's subkeys, call
    /// [`ValidKeyAmalgamationIter::subkeys`] on the returned iterator
    /// instead of skipping the first key: this causes the iterator to
    /// return values with a more accurate type.
    ///
    /// A key's secret key material may be protected with a
    /// password.  In such cases, it needs to be decrypted before it
    /// can be used to decrypt data or generate a signature.  Refer to
    /// [`Key::decrypt_secret`] for details.
    ///
    /// [`ValidKeyAmalgamationIter::subkeys`]: amalgamation::key::ValidKeyAmalgamationIter::subkeys()
    /// [`Key::decrypt_secret`]: crate::packet::Key::decrypt_secret()
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// // Create a key with two subkeys: one for signing and one for
    /// // encrypting data in transit.
    /// let (cert, _) = CertBuilder::new()
    ///     .add_userid("Alice")
    ///     .add_signing_subkey()
    ///     .add_transport_encryption_subkey()
    ///     .generate()?;
    /// // They should all be valid.
    /// assert_eq!(cert.with_policy(p, None)?.keys().count(), 1 + 2);
    /// #     Ok(())
    /// # }
    /// ```
    pub fn keys(&self) -> ValidKeyAmalgamationIter<'a, key::PublicParts, key::UnspecifiedRole> {
        self.cert.keys().with_policy(self.policy, self.time)
    }

    /// Returns the primary User ID at the reference time, if any.
    ///
    /// A certificate may not have a primary User ID if it doesn't
    /// have any valid User IDs.  If a certificate has at least one
    /// valid User ID at time `t`, then it has a primary User ID at
    /// time `t`.
    ///
    /// The primary User ID is determined as follows:
    ///
    ///   - Discard User IDs that are not valid or not alive at time `t`.
    ///
    ///   - Order the remaining User IDs by whether a User ID does not
    ///     have a valid self-revocation (i.e., non-revoked first,
    ///     ignoring third-party revocations).
    ///
    ///   - Break ties by ordering by whether the User ID is [marked
    ///     as being the primary User ID].
    ///
    ///   - Break ties by ordering by the binding signature's creation
    ///     time, most recent first.
    ///
    /// If there are multiple User IDs that are ordered first, then
    /// one is chosen in a deterministic, but undefined manner
    /// (currently, we order the value of the User IDs
    /// lexographically, but you shouldn't rely on this).
    ///
    /// [marked as being the primary User ID]: https://tools.ietf.org/html/rfc4880#section-5.2.3.19
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time;
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let t1 = time::SystemTime::now();
    /// let t2 = t1 + time::Duration::from_secs(1);
    ///
    /// let (cert, _) = CertBuilder::new()
    ///     .set_creation_time(t1)
    ///     .add_userid("Alice")
    ///     .generate()?;
    /// let mut signer = cert
    ///     .primary_key().key().clone().parts_into_secret()?.into_keypair()?;
    ///
    /// // There is only one User ID.  It must be the primary User ID.
    /// let vc = cert.with_policy(p, t1)?;
    /// let alice = vc.primary_userid().unwrap();
    /// assert_eq!(alice.value(), b"Alice");
    /// // By default, the primary User ID flag is set.
    /// assert!(alice.binding_signature().primary_userid().is_some());
    ///
    /// let template: signature::SignatureBuilder
    ///     = alice.binding_signature().clone().into();
    ///
    /// // Add another user id whose creation time is after the
    /// // existing User ID, and doesn't have the User ID set.
    /// let sig = template.clone()
    ///     .set_signature_creation_time(t2)?
    ///     .set_primary_userid(false)?;
    /// let bob: UserID = "Bob".into();
    /// let sig = bob.bind(&mut signer, &cert, sig)?;
    /// let cert = cert.insert_packets(vec![Packet::from(bob), sig.into()])?;
    /// # assert_eq!(cert.userids().count(), 2);
    ///
    /// // Alice should still be the primary User ID, because it has the
    /// // primary User ID flag set.
    /// let alice = cert.with_policy(p, t2)?.primary_userid().unwrap();
    /// assert_eq!(alice.value(), b"Alice");
    ///
    ///
    /// // Add another User ID, whose binding signature's creation
    /// // time is after Alice's and also has the primary User ID flag set.
    /// let sig = template.clone()
    ///    .set_signature_creation_time(t2)?;
    /// let carol: UserID = "Carol".into();
    /// let sig = carol.bind(&mut signer, &cert, sig)?;
    /// let cert = cert.insert_packets(vec![Packet::from(carol), sig.into()])?;
    /// # assert_eq!(cert.userids().count(), 3);
    ///
    /// // It should now be the primary User ID, because it is the
    /// // newest User ID with the primary User ID bit is set.
    /// let carol = cert.with_policy(p, t2)?.primary_userid().unwrap();
    /// assert_eq!(carol.value(), b"Carol");
    /// # Ok(()) }
    pub fn primary_userid(&self) -> Result<ValidUserIDAmalgamation<'a>>
    {
        self.cert.primary_userid_relaxed(self.policy(), self.time(), true)
    }

    /// Returns an iterator over the certificate's valid User IDs.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::time;
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// # let t0 = time::SystemTime::now() - time::Duration::from_secs(10);
    /// # let t1 = t0 + time::Duration::from_secs(1);
    /// # let t2 = t1 + time::Duration::from_secs(1);
    /// # let (cert, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .set_creation_time(t0)
    /// #     .generate()?;
    /// // `cert` was created at t0.  Add a second User ID at t1.
    /// let userid = UserID::from("alice@example.com");
    /// // Use the primary User ID's current binding signature as the
    /// // basis for the new User ID's binding signature.
    /// let template : signature::SignatureBuilder
    ///     = cert.with_policy(p, None)?
    ///           .primary_userid()?
    ///           .binding_signature()
    ///           .clone()
    ///           .into();
    /// let sig = template.set_signature_creation_time(t1)?;
    /// let mut signer = cert
    ///     .primary_key().key().clone().parts_into_secret()?.into_keypair()?;
    /// let binding = userid.bind(&mut signer, &cert, sig)?;
    /// // Merge it.
    /// let cert = cert.insert_packets(
    ///     vec![Packet::from(userid), binding.into()])?;
    ///
    /// // At t0, the new User ID is not yet valid (it doesn't have a
    /// // binding signature that is live at t0).  Thus, it is not
    /// // returned.
    /// let vc = cert.with_policy(p, t0)?;
    /// assert_eq!(vc.userids().count(), 1);
    /// // But, at t1, we see both User IDs.
    /// let vc = cert.with_policy(p, t1)?;
    /// assert_eq!(vc.userids().count(), 2);
    /// #     Ok(())
    /// # }
    /// ```
    pub fn userids(&self) -> ValidUserIDAmalgamationIter<'a> {
        self.cert.userids().with_policy(self.policy, self.time)
    }

    /// Returns the primary User Attribute, if any.
    ///
    /// If a certificate has any valid User Attributes, then it has a
    /// primary User Attribute.  In other words, it will not have a
    /// primary User Attribute at time `t` if there are no valid User
    /// Attributes at time `t`.
    ///
    /// The primary User Attribute is determined in the same way as
    /// the primary User ID.  See the documentation of
    /// [`ValidCert::primary_userid`] for details.
    ///
    /// [`ValidCert::primary_userid`]: ValidCert::primary_userid()
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// # let (cert, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// let vc = cert.with_policy(p, None)?;
    /// let ua = vc.primary_user_attribute();
    /// # // We don't have an user attributes.  So, this should return an
    /// # // error.
    /// # assert!(ua.is_err());
    /// #     Ok(())
    /// # }
    /// ```
    pub fn primary_user_attribute(&self)
        -> Result<ValidComponentAmalgamation<'a, UserAttribute>>
    {
        ValidComponentAmalgamation::primary(self.cert,
                                            self.cert.user_attributes.iter(),
                                            self.policy(), self.time(), true)
    }

    /// Returns an iterator over the certificate's valid
    /// `UserAttribute`s.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::prelude::*;
    /// # use openpgp::packet::user_attribute::Subpacket;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// # let (cert, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// #
    /// # // Create some user attribute. Doctests do not pass cfg(test),
    /// # // so UserAttribute::arbitrary is not available
    /// # let sp = Subpacket::Unknown(7, vec![7; 7].into_boxed_slice());
    /// # let ua = UserAttribute::new(&[sp]);
    /// #
    /// // Add a User Attribute without a self-signature to the certificate.
    /// let cert = cert.insert_packets(ua)?;
    /// assert_eq!(cert.user_attributes().count(), 1);
    ///
    /// // Without a self-signature, it is definitely not valid.
    /// let vc = cert.with_policy(p, None)?;
    /// assert_eq!(vc.user_attributes().count(), 0);
    /// #     Ok(())
    /// # }
    /// ```
    pub fn user_attributes(&self) -> ValidUserAttributeAmalgamationIter<'a> {
        self.cert.user_attributes().with_policy(self.policy, self.time)
    }

    /// Returns a list of any designated revokers for this certificate.
    ///
    /// This function returns the designated revokers listed on the
    /// primary key's binding signatures and the certificate's direct
    /// key signatures.
    ///
    /// Note: the returned list is deduplicated.
    ///
    /// In order to preserve our API during the 1.x series, this
    /// function takes an optional policy argument.  It should be
    /// `None`, but if it is `Some(_)`, it will be used instead of the
    /// `ValidCert`'s policy.  This makes the function signature
    /// compatible with [`Cert::revocation_keys`].
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::RevocationKey;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (alice, _) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///     .generate()?;
    /// // Make Alice a designated revoker for Bob.
    /// let (bob, _) =
    ///     CertBuilder::general_purpose(None, Some("bob@example.org"))
    ///     .set_revocation_keys(vec![(&alice).into()])
    ///     .generate()?;
    ///
    /// // Make sure Alice is listed as a designated revoker for Bob.
    /// assert_eq!(bob.with_policy(p, None)?.revocation_keys(None)
    ///            .collect::<Vec<&RevocationKey>>(),
    ///            vec![&(&alice).into()]);
    /// # Ok(()) }
    /// ```
    pub fn revocation_keys<P>(&self, policy: P)
        -> Box<dyn Iterator<Item = &'a RevocationKey> + 'a>
    where
        P: Into<Option<&'a dyn Policy>>,
    {
        self.cert.revocation_keys(
            policy.into().unwrap_or_else(|| self.policy()))
    }
}

macro_rules! impl_pref {
    ($subpacket:ident, $rt:ty) => {
        fn $subpacket(&self) -> Option<$rt>
        {
            // When addressed by the fingerprint or keyid, we first
            // look on the primary User ID and then fall back to the
            // direct key signature.  We need to be careful to handle
            // the case where there are no User IDs.
            if let Ok(u) = self.primary_userid() {
                u.$subpacket()
            } else if let Ok(sig) = self.direct_key_signature() {
                sig.$subpacket()
            } else {
                None
            }
        }
    }
}

impl<'a> seal::Sealed for ValidCert<'a> {}
impl<'a> Preferences<'a> for ValidCert<'a>
{
    impl_pref!(preferred_symmetric_algorithms, &'a [SymmetricAlgorithm]);
    impl_pref!(preferred_hash_algorithms, &'a [HashAlgorithm]);
    impl_pref!(preferred_compression_algorithms, &'a [CompressionAlgorithm]);
    impl_pref!(preferred_aead_algorithms, &'a [AEADAlgorithm]);
    impl_pref!(key_server_preferences, KeyServerPreferences);
    impl_pref!(preferred_key_server, &'a [u8]);
    impl_pref!(policy_uri, &'a [u8]);
    impl_pref!(features, Features);
}

#[cfg(test)]
mod test {
    use std::convert::TryInto;

    use crate::serialize::Serialize;
    use crate::policy::StandardPolicy as P;
    use crate::types::Curve;
    use crate::packet::signature;
    use crate::policy::HashAlgoSecurity;
    use super::*;

    use crate::{
        KeyID,
        types::KeyFlags,
    };

    fn parse_cert(data: &[u8], as_message: bool) -> Result<Cert> {
        if as_message {
            let pile = PacketPile::from_bytes(data).unwrap();
            Cert::try_from(pile)
        } else {
            Cert::from_bytes(data)
        }
    }

    #[test]
    fn broken() {
        use crate::types::Timestamp;
        for i in 0..2 {
            let cert = parse_cert(crate::tests::key("testy-broken-no-pk.pgp"),
                                i == 0);
            assert_match!(Error::MalformedCert(_)
                          = cert.err().unwrap().downcast::<Error>().unwrap());

            // According to 4880, a Cert must have a UserID.  But, we
            // don't require it.
            let cert = parse_cert(crate::tests::key("testy-broken-no-uid.pgp"),
                                i == 0);
            assert!(cert.is_ok());

            // We have:
            //
            //   [ pk, user id, sig, subkey ]
            let cert = parse_cert(crate::tests::key("testy-broken-no-sig-on-subkey.pgp"),
                                i == 0).unwrap();
            assert_eq!(cert.primary.key().creation_time(),
                       Timestamp::from(1511355130).into());
            assert_eq!(cert.userids.len(), 1);
            assert_eq!(cert.userids[0].userid().value(),
                       &b"Testy McTestface <testy@example.org>"[..]);
            assert_eq!(cert.userids[0].self_signatures.len(), 1);
            assert_eq!(cert.userids[0].self_signatures[0].digest_prefix(),
                       &[ 0xc6, 0x8f ]);
            assert_eq!(cert.user_attributes.len(), 0);
            assert_eq!(cert.subkeys.len(), 1);
        }
    }

    #[test]
    fn basics() {
        use crate::types::Timestamp;
        for i in 0..2 {
            let cert = parse_cert(crate::tests::key("testy.pgp"),
                                i == 0).unwrap();
            assert_eq!(cert.primary.key().creation_time(),
                       Timestamp::from(1511355130).into());
            assert_eq!(format!("{:X}", cert.fingerprint()),
                       "3E8877C877274692975189F5D03F6F865226FE8B");

            assert_eq!(cert.userids.len(), 1, "number of userids");
            assert_eq!(cert.userids[0].userid().value(),
                       &b"Testy McTestface <testy@example.org>"[..]);
            assert_eq!(cert.userids[0].self_signatures.len(), 1);
            assert_eq!(cert.userids[0].self_signatures[0].digest_prefix(),
                       &[ 0xc6, 0x8f ]);

            assert_eq!(cert.user_attributes.len(), 0);

            assert_eq!(cert.subkeys.len(), 1, "number of subkeys");
            assert_eq!(cert.subkeys[0].key().creation_time(),
                       Timestamp::from(1511355130).into());
            assert_eq!(cert.subkeys[0].self_signatures[0].digest_prefix(),
                       &[ 0xb7, 0xb9 ]);

            let cert = parse_cert(crate::tests::key("testy-no-subkey.pgp"),
                                i == 0).unwrap();
            assert_eq!(cert.primary.key().creation_time(),
                       Timestamp::from(1511355130).into());
            assert_eq!(format!("{:X}", cert.fingerprint()),
                       "3E8877C877274692975189F5D03F6F865226FE8B");

            assert_eq!(cert.user_attributes.len(), 0);

            assert_eq!(cert.userids.len(), 1, "number of userids");
            assert_eq!(cert.userids[0].userid().value(),
                       &b"Testy McTestface <testy@example.org>"[..]);
            assert_eq!(cert.userids[0].self_signatures.len(), 1);
            assert_eq!(cert.userids[0].self_signatures[0].digest_prefix(),
                       &[ 0xc6, 0x8f ]);

            assert_eq!(cert.subkeys.len(), 0, "number of subkeys");

            let cert = parse_cert(crate::tests::key("testy.asc"), i == 0).unwrap();
            assert_eq!(format!("{:X}", cert.fingerprint()),
                       "3E8877C877274692975189F5D03F6F865226FE8B");
        }
    }

    #[test]
    fn only_a_public_key() {
        // Make sure the Cert parser can parse a key that just consists
        // of a public key---no signatures, no user ids, nothing.
        let cert = Cert::from_bytes(crate::tests::key("testy-only-a-pk.pgp")).unwrap();
        assert_eq!(cert.userids.len(), 0);
        assert_eq!(cert.user_attributes.len(), 0);
        assert_eq!(cert.subkeys.len(), 0);
    }

    #[test]
    fn merge() {
        use crate::tests::key;
        let cert_base = Cert::from_bytes(key("bannon-base.gpg")).unwrap();

        // When we merge it with itself, we should get the exact same
        // thing.
        let merged = cert_base.clone().merge_public_and_secret(cert_base.clone()).unwrap();
        assert_eq!(cert_base, merged);

        let cert_add_uid_1
            = Cert::from_bytes(key("bannon-add-uid-1-whitehouse.gov.gpg"))
                .unwrap();
        let cert_add_uid_2
            = Cert::from_bytes(key("bannon-add-uid-2-fox.com.gpg"))
                .unwrap();
        // Duplicate user id, but with a different self-sig.
        let cert_add_uid_3
            = Cert::from_bytes(key("bannon-add-uid-3-whitehouse.gov-dup.gpg"))
                .unwrap();

        let cert_all_uids
            = Cert::from_bytes(key("bannon-all-uids.gpg"))
            .unwrap();
        // We have four User ID packets, but one has the same User ID,
        // just with a different self-signature.
        assert_eq!(cert_all_uids.userids.len(), 3);

        // Merge in order.
        let merged = cert_base.clone().merge_public_and_secret(cert_add_uid_1.clone()).unwrap()
            .merge_public_and_secret(cert_add_uid_2.clone()).unwrap()
            .merge_public_and_secret(cert_add_uid_3.clone()).unwrap();
        assert_eq!(cert_all_uids, merged);

        // Merge in reverse order.
        let merged = cert_base.clone()
            .merge_public_and_secret(cert_add_uid_3.clone()).unwrap()
            .merge_public_and_secret(cert_add_uid_2.clone()).unwrap()
            .merge_public_and_secret(cert_add_uid_1.clone()).unwrap();
        assert_eq!(cert_all_uids, merged);

        let cert_add_subkey_1
            = Cert::from_bytes(key("bannon-add-subkey-1.gpg")).unwrap();
        let cert_add_subkey_2
            = Cert::from_bytes(key("bannon-add-subkey-2.gpg")).unwrap();
        let cert_add_subkey_3
            = Cert::from_bytes(key("bannon-add-subkey-3.gpg")).unwrap();

        let cert_all_subkeys
            = Cert::from_bytes(key("bannon-all-subkeys.gpg")).unwrap();

        // Merge the first user, then the second, then the third.
        let merged = cert_base.clone().merge_public_and_secret(cert_add_subkey_1.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_2.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_3.clone()).unwrap();
        assert_eq!(cert_all_subkeys, merged);

        // Merge the third user, then the second, then the first.
        let merged = cert_base.clone().merge_public_and_secret(cert_add_subkey_3.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_2.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_1.clone()).unwrap();
        assert_eq!(cert_all_subkeys, merged);

        // Merge a lot.
        let merged = cert_base.clone()
            .merge_public_and_secret(cert_add_subkey_1.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_1.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_3.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_1.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_2.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_3.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_3.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_1.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_2.clone()).unwrap();
        assert_eq!(cert_all_subkeys, merged);

        let cert_all
            = Cert::from_bytes(key("bannon-all-uids-subkeys.gpg"))
            .unwrap();

        // Merge all the subkeys with all the uids.
        let merged = cert_all_subkeys.clone()
            .merge_public_and_secret(cert_all_uids.clone()).unwrap();
        assert_eq!(cert_all, merged);

        // Merge all uids with all the subkeys.
        let merged = cert_all_uids.clone()
            .merge_public_and_secret(cert_all_subkeys.clone()).unwrap();
        assert_eq!(cert_all, merged);

        // All the subkeys and the uids in a mixed up order.
        let merged = cert_base.clone()
            .merge_public_and_secret(cert_add_subkey_1.clone()).unwrap()
            .merge_public_and_secret(cert_add_uid_2.clone()).unwrap()
            .merge_public_and_secret(cert_add_uid_1.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_3.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_1.clone()).unwrap()
            .merge_public_and_secret(cert_add_uid_3.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_2.clone()).unwrap()
            .merge_public_and_secret(cert_add_subkey_1.clone()).unwrap()
            .merge_public_and_secret(cert_add_uid_2.clone()).unwrap();
        assert_eq!(cert_all, merged);

        // Certifications.
        let cert_donald_signs_base
            = Cert::from_bytes(key("bannon-the-donald-signs-base.gpg"))
            .unwrap();
        let cert_donald_signs_all
            = Cert::from_bytes(key("bannon-the-donald-signs-all-uids.gpg"))
            .unwrap();
        let cert_ivanka_signs_base
            = Cert::from_bytes(key("bannon-ivanka-signs-base.gpg"))
            .unwrap();
        let cert_ivanka_signs_all
            = Cert::from_bytes(key("bannon-ivanka-signs-all-uids.gpg"))
            .unwrap();

        assert!(cert_donald_signs_base.userids.len() == 1);
        assert!(cert_donald_signs_base.userids[0].self_signatures.len() == 1);
        assert!(cert_base.userids[0].certifications.is_empty());
        assert!(cert_donald_signs_base.userids[0].certifications.len() == 1);

        let merged = cert_donald_signs_base.clone()
            .merge_public_and_secret(cert_ivanka_signs_base.clone()).unwrap();
        assert!(merged.userids.len() == 1);
        assert!(merged.userids[0].self_signatures.len() == 1);
        assert!(merged.userids[0].certifications.len() == 2);

        let merged = cert_donald_signs_base.clone()
            .merge_public_and_secret(cert_donald_signs_all.clone()).unwrap();
        assert!(merged.userids.len() == 3);
        assert!(merged.userids[0].self_signatures.len() == 1);
        // There should be two certifications from the Donald on the
        // first user id.
        assert!(merged.userids[0].certifications.len() == 2);
        assert!(merged.userids[1].certifications.len() == 1);
        assert!(merged.userids[2].certifications.len() == 1);

        let merged = cert_donald_signs_base.clone()
            .merge_public_and_secret(cert_donald_signs_all.clone()).unwrap()
            .merge_public_and_secret(cert_ivanka_signs_base.clone()).unwrap()
            .merge_public_and_secret(cert_ivanka_signs_all.clone()).unwrap();
        assert!(merged.userids.len() == 3);
        assert!(merged.userids[0].self_signatures.len() == 1);
        // There should be two certifications from each of the Donald
        // and Ivanka on the first user id, and one each on the rest.
        assert!(merged.userids[0].certifications.len() == 4);
        assert!(merged.userids[1].certifications.len() == 2);
        assert!(merged.userids[2].certifications.len() == 2);

        // Same as above, but redundant.
        let merged = cert_donald_signs_base.clone()
            .merge_public_and_secret(cert_ivanka_signs_base.clone()).unwrap()
            .merge_public_and_secret(cert_donald_signs_all.clone()).unwrap()
            .merge_public_and_secret(cert_donald_signs_all.clone()).unwrap()
            .merge_public_and_secret(cert_ivanka_signs_all.clone()).unwrap()
            .merge_public_and_secret(cert_ivanka_signs_base.clone()).unwrap()
            .merge_public_and_secret(cert_donald_signs_all.clone()).unwrap()
            .merge_public_and_secret(cert_donald_signs_all.clone()).unwrap()
            .merge_public_and_secret(cert_ivanka_signs_all.clone()).unwrap();
        assert!(merged.userids.len() == 3);
        assert!(merged.userids[0].self_signatures.len() == 1);
        // There should be two certifications from each of the Donald
        // and Ivanka on the first user id, and one each on the rest.
        assert!(merged.userids[0].certifications.len() == 4);
        assert!(merged.userids[1].certifications.len() == 2);
        assert!(merged.userids[2].certifications.len() == 2);
    }

    #[test]
    fn out_of_order_self_sigs_test() {
        // neal-out-of-order.pgp contains all of the self-signatures,
        // but some are out of order.  The canonicalization step
        // should reorder them.
        //
        // original order/new order:
        //
        //  1/ 1. pk
        //  2/ 2. user id #1: neal@walfield.org (good)
        //  3/ 3. sig over user ID #1
        //
        //  4/ 4. user id #2: neal@gnupg.org (good)
        //  5/ 7. sig over user ID #3
        //  6/ 5. sig over user ID #2
        //
        //  7/ 6. user id #3: neal@g10code.com (bad)
        //
        //  8/ 8. user ID #4: neal@pep.foundation (bad)
        //  9/11. sig over user ID #5
        //
        // 10/10. user id #5: neal@pep-project.org (bad)
        // 11/ 9. sig over user ID #4
        //
        // 12/12. user ID #6: neal@sequoia-pgp.org (good)
        // 13/13. sig over user ID #6
        //
        // ----------------------------------------------
        //
        // 14/14. signing subkey #1: 7223B56678E02528 (good)
        // 15/15. sig over subkey #1
        // 16/16. sig over subkey #1
        //
        // 17/17. encryption subkey #2: C2B819056C652598 (good)
        // 18/18. sig over subkey #2
        // 19/21. sig over subkey #3
        // 20/22. sig over subkey #3
        //
        // 21/20. auth subkey #3: A3506AFB820ABD08 (bad)
        // 22/19. sig over subkey #2

        let cert = Cert::from_bytes(crate::tests::key("neal-sigs-out-of-order.pgp"))
            .unwrap();

        let mut userids = cert.userids()
            .map(|u| String::from_utf8_lossy(u.value()).into_owned())
            .collect::<Vec<String>>();
        userids.sort();

        assert_eq!(userids,
                   &[ "Neal H. Walfield <neal@g10code.com>",
                      "Neal H. Walfield <neal@gnupg.org>",
                      "Neal H. Walfield <neal@pep-project.org>",
                      "Neal H. Walfield <neal@pep.foundation>",
                      "Neal H. Walfield <neal@sequoia-pgp.org>",
                      "Neal H. Walfield <neal@walfield.org>",
                   ]);

        let mut subkeys = cert.subkeys()
            .map(|sk| Some(sk.key().keyid()))
            .collect::<Vec<Option<KeyID>>>();
        subkeys.sort();
        assert_eq!(subkeys,
                   &[ "7223B56678E02528".parse().ok(),
                      "A3506AFB820ABD08".parse().ok(),
                      "C2B819056C652598".parse().ok(),
                   ]);

        // DKG's key has all of the self-signatures moved to the last
        // subkey; all user ids/user attributes/subkeys have nothing.
        let cert =
            Cert::from_bytes(crate::tests::key("dkg-sigs-out-of-order.pgp")).unwrap();

        let mut userids = cert.userids()
            .map(|u| String::from_utf8_lossy(u.value()).into_owned())
            .collect::<Vec<String>>();
        userids.sort();

        assert_eq!(userids,
                   &[ "Daniel Kahn Gillmor <dkg-debian.org@fifthhorseman.net>",
                      "Daniel Kahn Gillmor <dkg@aclu.org>",
                      "Daniel Kahn Gillmor <dkg@astro.columbia.edu>",
                      "Daniel Kahn Gillmor <dkg@debian.org>",
                      "Daniel Kahn Gillmor <dkg@fifthhorseman.net>",
                      "Daniel Kahn Gillmor <dkg@openflows.com>",
                   ]);

        assert_eq!(cert.user_attributes.len(), 1);

        let mut subkeys = cert.subkeys()
            .map(|sk| Some(sk.key().keyid()))
            .collect::<Vec<Option<KeyID>>>();
        subkeys.sort();
        assert_eq!(subkeys,
                   &[ "1075 8EBD BD7C FAB5".parse().ok(),
                      "1258 68EA 4BFA 08E4".parse().ok(),
                      "1498 ADC6 C192 3237".parse().ok(),
                      "24EC FF5A FF68 370A".parse().ok(),
                      "3714 7292 14D5 DA70".parse().ok(),
                      "3B7A A7F0 14E6 9B5A".parse().ok(),
                      "5B58 DCF9 C341 6611".parse().ok(),
                      "A524 01B1 1BFD FA5C".parse().ok(),
                      "A70A 96E1 439E A852".parse().ok(),
                      "C61B D3EC 2148 4CFF".parse().ok(),
                      "CAEF A883 2167 5333".parse().ok(),
                      "DC10 4C4E 0CA7 57FB".parse().ok(),
                      "E3A3 2229 449B 0350".parse().ok(),
                   ]);

    }

    /// Tests how we deal with v3 keys, certs, and certifications.
    #[test]
    fn v3_packets() {
        // v3 primary keys are not supported.

        let cert = Cert::from_bytes(crate::tests::key("john-v3.pgp"));
        assert_match!(Error::UnsupportedCert2(..)
                      = cert.err().unwrap().downcast::<Error>().unwrap());

        let cert = Cert::from_bytes(crate::tests::key("john-v3-secret.pgp"));
        assert_match!(Error::UnsupportedCert2(..)
                      = cert.err().unwrap().downcast::<Error>().unwrap());

        // Lutz's key is a v3 key.
        let cert = Cert::from_bytes(crate::tests::key("lutz.gpg"));
        assert_match!(Error::UnsupportedCert2(..)
                      = cert.err().unwrap().downcast::<Error>().unwrap());

        // v3 certifications are not supported

        // dkg's includes some v3 signatures.
        let cert = Cert::from_bytes(crate::tests::key("dkg.gpg"));
        assert!(cert.is_ok(), "dkg.gpg: {:?}", cert);
    }

    #[test]
    fn keyring_with_v3_public_keys() {
        let dkg = crate::tests::key("dkg.gpg");
        let lutz = crate::tests::key("lutz.gpg");

        let cert = Cert::from_bytes(dkg);
        assert!(cert.is_ok(), "dkg.gpg: {:?}", cert);

        // Keyring with two good keys
        let mut combined = vec![];
        combined.extend_from_slice(dkg);
        combined.extend_from_slice(dkg);
        let certs = CertParser::from_bytes(&combined[..]).unwrap()
            .map(|certr| certr.is_ok())
            .collect::<Vec<bool>>();
        assert_eq!(certs, &[ true, true ]);

        // Keyring with a good key, and a bad key.
        let mut combined = vec![];
        combined.extend_from_slice(dkg);
        combined.extend_from_slice(lutz);
        let certs = CertParser::from_bytes(&combined[..]).unwrap()
            .map(|certr| certr.is_ok())
            .collect::<Vec<bool>>();
        assert_eq!(certs, &[ true, false ]);

        // Keyring with a bad key, and a good key.
        let mut combined = vec![];
        combined.extend_from_slice(lutz);
        combined.extend_from_slice(dkg);
        let certs = CertParser::from_bytes(&combined[..]).unwrap()
            .map(|certr| certr.is_ok())
            .collect::<Vec<bool>>();
        assert_eq!(certs, &[ false, true ]);

        // Keyring with a good key, a bad key, and a good key.
        let mut combined = vec![];
        combined.extend_from_slice(dkg);
        combined.extend_from_slice(lutz);
        combined.extend_from_slice(dkg);
        let certs = CertParser::from_bytes(&combined[..]).unwrap()
            .map(|certr| certr.is_ok())
            .collect::<Vec<bool>>();
        assert_eq!(certs, &[ true, false, true ]);

        // Keyring with a good key, a bad key, and a bad key.
        let mut combined = vec![];
        combined.extend_from_slice(dkg);
        combined.extend_from_slice(lutz);
        combined.extend_from_slice(lutz);
        let certs = CertParser::from_bytes(&combined[..]).unwrap()
            .map(|certr| certr.is_ok())
            .collect::<Vec<bool>>();
        assert_eq!(certs, &[ true, false, false ]);

        // Keyring with a good key, a bad key, a bad key, and a good key.
        let mut combined = vec![];
        combined.extend_from_slice(dkg);
        combined.extend_from_slice(lutz);
        combined.extend_from_slice(lutz);
        combined.extend_from_slice(dkg);
        let certs = CertParser::from_bytes(&combined[..]).unwrap()
            .map(|certr| certr.is_ok())
            .collect::<Vec<bool>>();
        assert_eq!(certs, &[ true, false, false, true ]);
    }

    #[test]
    fn merge_with_incomplete_update() {
        let p = &P::new();

        let cert = Cert::from_bytes(crate::tests::key("about-to-expire.expired.pgp"))
            .unwrap();
        cert.primary_key().with_policy(p, None).unwrap().alive().unwrap_err();

        let update =
            Cert::from_bytes(crate::tests::key("about-to-expire.update-no-uid.pgp"))
            .unwrap();
        let cert = cert.merge_public_and_secret(update).unwrap();
        cert.primary_key().with_policy(p, None).unwrap().alive().unwrap();
    }

    #[test]
    fn packet_pile_roundtrip() {
        // Make sure Cert::try_from(Cert::to_packet_pile(cert))
        // does a clean round trip.

        let cert = Cert::from_bytes(crate::tests::key("already-revoked.pgp")).unwrap();
        let cert2
            = Cert::try_from(cert.clone().into_packet_pile()).unwrap();
        assert_eq!(cert, cert2);

        let cert = Cert::from_bytes(
            crate::tests::key("already-revoked-direct-revocation.pgp")).unwrap();
        let cert2
            = Cert::try_from(cert.clone().into_packet_pile()).unwrap();
        assert_eq!(cert, cert2);

        let cert = Cert::from_bytes(
            crate::tests::key("already-revoked-userid-revocation.pgp")).unwrap();
        let cert2
            = Cert::try_from(cert.clone().into_packet_pile()).unwrap();
        assert_eq!(cert, cert2);

        let cert = Cert::from_bytes(
            crate::tests::key("already-revoked-subkey-revocation.pgp")).unwrap();
        let cert2
            = Cert::try_from(cert.clone().into_packet_pile()).unwrap();
        assert_eq!(cert, cert2);
    }

    #[test]
    fn insert_packets_add_sig() {
        use crate::armor;
        use crate::packet::Tag;

        // Merge the revocation certificate into the Cert and make sure
        // it shows up.
        let cert = Cert::from_bytes(crate::tests::key("already-revoked.pgp")).unwrap();

        let rev = crate::tests::key("already-revoked.rev");
        let rev = PacketPile::from_reader(armor::Reader::from_reader(rev, None))
            .unwrap();

        let rev : Vec<Packet> = rev.into_children().collect();
        assert_eq!(rev.len(), 1);
        assert_eq!(rev[0].tag(), Tag::Signature);

        let packets_pre_merge = cert.clone().into_packets().count();
        let cert = cert.insert_packets(rev).unwrap();
        let packets_post_merge = cert.clone().into_packets().count();
        assert_eq!(packets_post_merge, packets_pre_merge + 1);
    }

    #[test]
    fn insert_packets_update_sig() -> Result<()> {
        use std::time::Duration;

        use crate::packet::signature::subpacket::Subpacket;
        use crate::packet::signature::subpacket::SubpacketValue;

        let (cert, _) = CertBuilder::general_purpose(None, Some("Test"))
            .generate()?;
        let packets = cert.clone().into_packets().count();

        // Merge a signature with different unhashed subpacket areas.
        // Make sure only the last variant is merged.
        let sig = cert.primary_key().self_signatures().next()
            .expect("binding signature");

        let a = Subpacket::new(
            SubpacketValue::SignatureExpirationTime(
                Duration::new(1, 0).try_into()?),
            false)?;
        let b = Subpacket::new(
            SubpacketValue::SignatureExpirationTime(
                Duration::new(2, 0).try_into()?),
            false)?;

        let mut sig_a = sig.clone();
        sig_a.unhashed_area_mut().add(a)?;
        let mut sig_b = sig.clone();
        sig_b.unhashed_area_mut().add(b)?;

        // Insert sig_a, make sure it (and it alone) appears.
        let cert2 = cert.clone().insert_packets(sig_a.clone())?;
        let mut sigs = cert2.primary_key().self_signatures();
        assert_eq!(sigs.next(), Some(&sig_a));
        assert!(sigs.next().is_none());
        assert_eq!(cert2.clone().into_packets().count(), packets);

        // Insert sig_b, make sure it (and it alone) appears.
        let cert2 = cert.clone().insert_packets(sig_b.clone())?;
        let mut sigs = cert2.primary_key().self_signatures();
        assert_eq!(sigs.next(), Some(&sig_b));
        assert!(sigs.next().is_none());
        assert_eq!(cert2.clone().into_packets().count(), packets);

        // Insert sig_a and sig_b.  Make sure sig_b (and it alone)
        // appears.
        let cert2 = cert.clone().insert_packets(
            vec![ sig_a.clone(), sig_b.clone() ])?;
        let mut sigs = cert2.primary_key().self_signatures();
        assert_eq!(sigs.next(), Some(&sig_b));
        assert!(sigs.next().is_none());
        assert_eq!(cert2.clone().into_packets().count(), packets);

        // Insert sig_b and sig_a.  Make sure sig_a (and it alone)
        // appears.
        let cert2 = cert.clone().insert_packets(
            vec![ sig_b.clone(), sig_a.clone() ])?;
        let mut sigs = cert2.primary_key().self_signatures();
        assert_eq!(sigs.next(), Some(&sig_a));
        assert!(sigs.next().is_none());
        assert_eq!(cert2.clone().into_packets().count(), packets);

        Ok(())
    }

    #[test]
    fn insert_packets_add_userid() -> Result<()> {
        let (cert, _) = CertBuilder::general_purpose(None, Some("a"))
            .generate()?;
        let packets = cert.clone().into_packets().count();

        let uid_a = UserID::from("a");
        let uid_b = UserID::from("b");

        // Insert a, make sure it appears once.
        let cert2 = cert.clone().insert_packets(uid_a.clone())?;
        let mut uids = cert2.userids();
        assert_eq!(uids.next().unwrap().userid(), &uid_a);
        assert!(uids.next().is_none());
        assert_eq!(cert2.clone().into_packets().count(), packets);

        // Insert b, make sure it also appears.
        let cert2 = cert.clone().insert_packets(uid_b.clone())?;
        let mut uids: Vec<UserID>
            = cert2.userids().map(|ua| ua.userid().clone()).collect();
        uids.sort();
        let mut uids = uids.iter();
        assert_eq!(uids.next().unwrap(), &uid_a);
        assert_eq!(uids.next().unwrap(), &uid_b);
        assert!(uids.next().is_none());
        assert_eq!(cert2.clone().into_packets().count(), packets + 1);

        Ok(())
    }

    #[test]
    fn insert_packets_update_key() -> Result<()> {
        use crate::crypto::Password;

        let (cert, _) = CertBuilder::new().generate()?;
        let packets = cert.clone().into_packets().count();
        assert_eq!(cert.keys().count(), 1);

        let key = cert.keys().secret().next().unwrap().key();
        assert!(key.has_secret());
        let key_a = key.clone().encrypt_secret(&Password::from("a"))?
            .role_into_primary();
        let key_b = key.clone().encrypt_secret(&Password::from("b"))?
            .role_into_primary();

        // Insert variant a.
        let cert2 = cert.clone().insert_packets(key_a.clone())?;
        assert_eq!(cert2.primary_key().key().parts_as_secret().unwrap(),
                   &key_a);
        assert_eq!(cert2.clone().into_packets().count(), packets);

        // Insert variant b.
        let cert2 = cert.clone().insert_packets(key_b.clone())?;
        assert_eq!(cert2.primary_key().key().parts_as_secret().unwrap(),
                   &key_b);
        assert_eq!(cert2.clone().into_packets().count(), packets);

        // Insert variant a then b.  We should keep b.
        let cert2 = cert.clone().insert_packets(
            vec![ key_a.clone(), key_b.clone() ])?;
        assert_eq!(cert2.primary_key().key().parts_as_secret().unwrap(),
                   &key_b);
        assert_eq!(cert2.clone().into_packets().count(), packets);

        // Insert variant b then a.  We should keep a.
        let cert2 = cert.clone().insert_packets(
            vec![ key_b.clone(), key_a.clone() ])?;
        assert_eq!(cert2.primary_key().key().parts_as_secret().unwrap(),
                   &key_a);
        assert_eq!(cert2.clone().into_packets().count(), packets);

        Ok(())
    }

    #[test]
    fn set_validity_period() {
        let p = &P::new();

        let (cert, _) = CertBuilder::general_purpose(None, Some("Test"))
            .generate().unwrap();
        assert_eq!(cert.clone().into_packet_pile().children().count(),
                   1 // primary key
                   + 1 // direct key signature
                   + 1 // userid
                   + 1 // binding signature
                   + 1 // subkey
                   + 1 // binding signature
                   + 1 // subkey
                   + 1 // binding signature
        );
        let cert = check_set_validity_period(p, cert);
        assert_eq!(cert.clone().into_packet_pile().children().count(),
                   1 // primary key
                   + 1 // direct key signature
                   + 2 // two new direct key signatures
                   + 1 // userid
                   + 1 // binding signature
                   + 2 // two new binding signatures
                   + 1 // subkey
                   + 1 // binding signature
                   + 1 // subkey
                   + 1 // binding signature
        );
    }

    #[test]
    fn set_validity_period_two_uids() -> Result<()> {
        use quickcheck::{Arbitrary, Gen};
        let mut gen = Gen::new(16);
        let p = &P::new();

        let userid1 = UserID::arbitrary(&mut gen);
        // The two user ids need to be unique.
        let mut userid2 = UserID::arbitrary(&mut gen);
        while userid1 == userid2 {
            userid2 = UserID::arbitrary(&mut gen);
        }

        let (cert, _) = CertBuilder::general_purpose(
            None, Some(userid1))
            .add_userid(userid2)
            .generate()?;
        let primary_uid = cert.with_policy(p, None)?.primary_userid()?.userid().clone();
        assert_eq!(cert.clone().into_packet_pile().children().count(),
                   1 // primary key
                   + 1 // direct key signature
                   + 1 // userid
                   + 1 // binding signature
                   + 1 // userid
                   + 1 // binding signature
                   + 1 // subkey
                   + 1 // binding signature
                   + 1 // subkey
                   + 1 // binding signature
        );
        let cert = check_set_validity_period(p, cert);
        assert_eq!(cert.clone().into_packet_pile().children().count(),
                   1 // primary key
                   + 1 // direct key signature
                   + 2 // two new direct key signatures
                   + 1 // userid
                   + 1 // binding signature
                   + 2 // two new binding signatures
                   + 1 // userid
                   + 1 // binding signature
                   + 2 // two new binding signatures
                   + 1 // subkey
                   + 1 // binding signature
                   + 1 // subkey
                   + 1 // binding signature
        );
        assert_eq!(&primary_uid, cert.with_policy(p, None)?.primary_userid()?.userid());
        Ok(())
    }

    #[test]
    fn set_validity_period_uidless() {
        use crate::types::Duration;
        let p = &P::new();

        let (cert, _) = CertBuilder::new()
            .set_validity_period(None) // Just to assert this works.
            .set_validity_period(Some(Duration::weeks(52).unwrap().try_into().unwrap()))
            .generate().unwrap();
        assert_eq!(cert.clone().into_packet_pile().children().count(),
                   1 // primary key
                   + 1 // direct key signature
        );
        let cert = check_set_validity_period(p, cert);
        assert_eq!(cert.clone().into_packet_pile().children().count(),
                   1 // primary key
                   + 1 // direct key signature
                   + 2 // two new direct key signatures
        );
    }
    fn check_set_validity_period(policy: &dyn Policy, cert: Cert) -> Cert {
        let now = cert.primary_key().creation_time();
        let a_sec = time::Duration::new(1, 0);

        let expiry_orig = cert.primary_key().with_policy(policy, now).unwrap()
            .key_validity_period()
            .expect("Keys expire by default.");

        let mut keypair = cert.primary_key().key().clone().parts_into_secret()
            .unwrap().into_keypair().unwrap();

        // Clear the expiration.
        let as_of1 = now + time::Duration::new(10, 0);
        let cert = cert.set_validity_period_as_of(
            policy, &mut keypair, None, as_of1).unwrap();
        {
            // If t < as_of1, we should get the original expiry.
            assert_eq!(cert.primary_key().with_policy(policy, now).unwrap()
                           .key_validity_period(),
                       Some(expiry_orig));
            assert_eq!(cert.primary_key().with_policy(policy, as_of1 - a_sec).unwrap()
                           .key_validity_period(),
                       Some(expiry_orig));
            // If t >= as_of1, we should get the new expiry.
            assert_eq!(cert.primary_key().with_policy(policy, as_of1).unwrap()
                           .key_validity_period(),
                       None);
        }

        // Shorten the expiry.  (The default expiration should be at
        // least a few weeks, so removing an hour should still keep us
        // over 0.)
        let expiry_new = expiry_orig - time::Duration::new(60 * 60, 0);
        assert!(expiry_new > time::Duration::new(0, 0));

        let as_of2 = as_of1 + time::Duration::new(10, 0);
        let cert = cert.set_validity_period_as_of(
            policy, &mut keypair, Some(expiry_new), as_of2).unwrap();
        {
            // If t < as_of1, we should get the original expiry.
            assert_eq!(cert.primary_key().with_policy(policy, now).unwrap()
                           .key_validity_period(),
                       Some(expiry_orig));
            assert_eq!(cert.primary_key().with_policy(policy, as_of1 - a_sec).unwrap()
                           .key_validity_period(),
                       Some(expiry_orig));
            // If as_of1 <= t < as_of2, we should get the second
            // expiry (None).
            assert_eq!(cert.primary_key().with_policy(policy, as_of1).unwrap()
                           .key_validity_period(),
                       None);
            assert_eq!(cert.primary_key().with_policy(policy, as_of2 - a_sec).unwrap()
                           .key_validity_period(),
                       None);
            // If t <= as_of2, we should get the new expiry.
            assert_eq!(cert.primary_key().with_policy(policy, as_of2).unwrap()
                           .key_validity_period(),
                       Some(expiry_new));
        }
        cert
    }

    #[test]
    fn direct_key_sig() {
        use crate::types::SignatureType;
        // XXX: testing sequoia against itself isn't optimal, but I couldn't
        // find a tool to generate direct key signatures :-(

        let p = &P::new();

        let (cert1, _) = CertBuilder::new().generate().unwrap();
        let mut buf = Vec::default();

        cert1.serialize(&mut buf).unwrap();
        let cert2 = Cert::from_bytes(&buf).unwrap();

        assert_eq!(
            cert2.primary_key().with_policy(p, None).unwrap()
                .direct_key_signature().unwrap().typ(),
            SignatureType::DirectKey);
        assert_eq!(cert2.userids().count(), 0);
    }

    #[test]
    fn revoked() {
        fn check(cert: &Cert, direct_revoked: bool,
                 userid_revoked: bool, subkey_revoked: bool) {
            let p = &P::new();

            // If we have a user id---even if it is revoked---we have
            // a primary key signature.
            let typ = cert.primary_key().with_policy(p, None).unwrap()
                .binding_signature().typ();
            assert_eq!(typ, SignatureType::PositiveCertification,
                       "{:#?}", cert);

            let revoked = cert.revocation_status(p, None);
            if direct_revoked {
                assert_match!(RevocationStatus::Revoked(_) = revoked,
                              "{:#?}", cert);
            } else {
                assert_eq!(revoked, RevocationStatus::NotAsFarAsWeKnow,
                           "{:#?}", cert);
            }

            for userid in cert.userids().with_policy(p, None) {
                let typ = userid.binding_signature().typ();
                assert_eq!(typ, SignatureType::PositiveCertification,
                           "{:#?}", cert);

                let revoked = userid.revocation_status();
                if userid_revoked {
                    assert_match!(RevocationStatus::Revoked(_) = revoked);
                } else {
                    assert_eq!(RevocationStatus::NotAsFarAsWeKnow, revoked,
                               "{:#?}", cert);
                }
            }

            for subkey in cert.subkeys() {
                let typ = subkey.binding_signature(p, None).unwrap().typ();
                assert_eq!(typ, SignatureType::SubkeyBinding,
                           "{:#?}", cert);

                let revoked = subkey.revocation_status(p, None);
                if subkey_revoked {
                    assert_match!(RevocationStatus::Revoked(_) = revoked);
                } else {
                    assert_eq!(RevocationStatus::NotAsFarAsWeKnow, revoked,
                               "{:#?}", cert);
                }
            }
        }

        let cert = Cert::from_bytes(crate::tests::key("already-revoked.pgp")).unwrap();
        check(&cert, false, false, false);

        let d = Cert::from_bytes(
            crate::tests::key("already-revoked-direct-revocation.pgp")).unwrap();
        check(&d, true, false, false);

        check(&cert.clone().merge_public_and_secret(d.clone()).unwrap(), true, false, false);
        // Make sure the merge order does not matter.
        check(&d.clone().merge_public_and_secret(cert.clone()).unwrap(), true, false, false);

        let u = Cert::from_bytes(
            crate::tests::key("already-revoked-userid-revocation.pgp")).unwrap();
        check(&u, false, true, false);

        check(&cert.clone().merge_public_and_secret(u.clone()).unwrap(), false, true, false);
        check(&u.clone().merge_public_and_secret(cert.clone()).unwrap(), false, true, false);

        let k = Cert::from_bytes(
            crate::tests::key("already-revoked-subkey-revocation.pgp")).unwrap();
        check(&k, false, false, true);

        check(&cert.clone().merge_public_and_secret(k.clone()).unwrap(), false, false, true);
        check(&k.clone().merge_public_and_secret(cert.clone()).unwrap(), false, false, true);

        // direct and user id revocation.
        check(&d.clone().merge_public_and_secret(u.clone()).unwrap(), true, true, false);
        check(&u.clone().merge_public_and_secret(d.clone()).unwrap(), true, true, false);

        // direct and subkey revocation.
        check(&d.clone().merge_public_and_secret(k.clone()).unwrap(), true, false, true);
        check(&k.clone().merge_public_and_secret(d.clone()).unwrap(), true, false, true);

        // user id and subkey revocation.
        check(&u.clone().merge_public_and_secret(k.clone()).unwrap(), false, true, true);
        check(&k.clone().merge_public_and_secret(u.clone()).unwrap(), false, true, true);

        // direct, user id and subkey revocation.
        check(&d.clone().merge_public_and_secret(u.clone().merge_public_and_secret(k.clone()).unwrap()).unwrap(),
              true, true, true);
        check(&d.clone().merge_public_and_secret(k.clone().merge_public_and_secret(u.clone()).unwrap()).unwrap(),
              true, true, true);
    }

    #[test]
    fn revoke() {
        let p = &P::new();

        let (cert, _) = CertBuilder::general_purpose(None, Some("Test"))
            .generate().unwrap();
        assert_eq!(RevocationStatus::NotAsFarAsWeKnow,
                   cert.revocation_status(p, None));

        let mut keypair = cert.primary_key().key().clone().parts_into_secret()
            .unwrap().into_keypair().unwrap();

        let sig = CertRevocationBuilder::new()
            .set_reason_for_revocation(
                ReasonForRevocation::KeyCompromised,
                b"It was the maid :/").unwrap()
            .build(&mut keypair, &cert, None)
            .unwrap();
        assert_eq!(sig.typ(), SignatureType::KeyRevocation);
        assert_eq!(sig.issuers().collect::<Vec<_>>(),
                   vec![ &cert.keyid() ]);
        assert_eq!(sig.issuer_fingerprints().collect::<Vec<_>>(),
                   vec![ &cert.fingerprint() ]);

        let cert = cert.insert_packets(sig).unwrap();
        assert_match!(RevocationStatus::Revoked(_) = cert.revocation_status(p, None));


        // Have other revoke cert.
        let (other, _) = CertBuilder::general_purpose(None, Some("Test 2"))
            .generate().unwrap();

        let mut keypair = other.primary_key().key().clone().parts_into_secret()
            .unwrap().into_keypair().unwrap();

        let sig = CertRevocationBuilder::new()
            .set_reason_for_revocation(
                ReasonForRevocation::KeyCompromised,
                b"It was the maid :/").unwrap()
            .build(&mut keypair, &cert, None)
            .unwrap();

        assert_eq!(sig.typ(), SignatureType::KeyRevocation);
        assert_eq!(sig.issuers().collect::<Vec<_>>(),
                   vec![ &other.keyid() ]);
        assert_eq!(sig.issuer_fingerprints().collect::<Vec<_>>(),
                   vec![ &other.fingerprint() ]);
    }

    #[test]
    fn revoke_subkey() {
        let p = &P::new();
        let (cert, _) = CertBuilder::new()
            .add_transport_encryption_subkey()
            .generate().unwrap();

        let sig = {
            let subkey = cert.subkeys().next().unwrap();
            assert_eq!(RevocationStatus::NotAsFarAsWeKnow,
                       subkey.revocation_status(p, None));

            let mut keypair = cert.primary_key().key().clone().parts_into_secret()
                .unwrap().into_keypair().unwrap();
            SubkeyRevocationBuilder::new()
                .set_reason_for_revocation(
                    ReasonForRevocation::UIDRetired,
                    b"It was the maid :/").unwrap()
                .build(&mut keypair, &cert, subkey.key(), None)
                .unwrap()
        };
        assert_eq!(sig.typ(), SignatureType::SubkeyRevocation);
        let cert = cert.insert_packets(sig).unwrap();
        assert_eq!(RevocationStatus::NotAsFarAsWeKnow,
                   cert.revocation_status(p, None));

        let subkey = cert.subkeys().next().unwrap();
        assert_match!(RevocationStatus::Revoked(_)
                      = subkey.revocation_status(p, None));
    }

    #[test]
    fn revoke_uid() {
        let p = &P::new();
        let (cert, _) = CertBuilder::new()
            .add_userid("Test1")
            .add_userid("Test2")
            .generate().unwrap();

        let sig = {
            let uid = cert.userids().with_policy(p, None).nth(1).unwrap();
            assert_eq!(RevocationStatus::NotAsFarAsWeKnow, uid.revocation_status());

            let mut keypair = cert.primary_key().key().clone().parts_into_secret()
                .unwrap().into_keypair().unwrap();
            UserIDRevocationBuilder::new()
                .set_reason_for_revocation(
                    ReasonForRevocation::UIDRetired,
                    b"It was the maid :/").unwrap()
                .build(&mut keypair, &cert, uid.userid(), None)
                .unwrap()
        };
        assert_eq!(sig.typ(), SignatureType::CertificationRevocation);
        let cert = cert.insert_packets(sig).unwrap();
        assert_eq!(RevocationStatus::NotAsFarAsWeKnow,
                   cert.revocation_status(p, None));

        let uid = cert.userids().with_policy(p, None).nth(1).unwrap();
        assert_match!(RevocationStatus::Revoked(_) = uid.revocation_status());
    }

    #[test]
    fn key_revoked() {
        use crate::types::Features;
        use crate::packet::key::Key4;
        use rand::{thread_rng, Rng, distributions::Open01};

        let p = &P::new();

        /*
         * t1: 1st binding sig ctime
         * t2: soft rev sig ctime
         * t3: 2nd binding sig ctime
         * t4: hard rev sig ctime
         *
         * [0,t1): invalid, but not revoked
         * [t1,t2): valid (not revocations)
         * [t2,t3): revoked (soft revocation)
         * [t3,t4): valid again (new self sig)
         * [t4,inf): hard revocation (hard revocation)
         *
         * Once the hard revocation is merged, then the Cert is
         * considered revoked at all times.
         */
        let t1 = time::UNIX_EPOCH + time::Duration::new(946681200, 0);  // 2000-1-1
        let t2 = time::UNIX_EPOCH + time::Duration::new(978303600, 0);  // 2001-1-1
        let t3 = time::UNIX_EPOCH + time::Duration::new(1009839600, 0); // 2002-1-1
        let t4 = time::UNIX_EPOCH + time::Duration::new(1041375600, 0); // 2003-1-1

        let mut key: key::SecretKey
            = Key4::generate_ecc(true, Curve::Ed25519).unwrap().into();
        key.set_creation_time(t1).unwrap();
        let mut pair = key.clone().into_keypair().unwrap();
        let (bind1, rev1, bind2, rev2) = {
            let bind1 = signature::SignatureBuilder::new(SignatureType::DirectKey)
                .set_features(Features::sequoia()).unwrap()
                .set_key_flags(KeyFlags::empty()).unwrap()
                .set_signature_creation_time(t1).unwrap()
                .set_key_validity_period(Some(time::Duration::new(10 * 52 * 7 * 24 * 60 * 60, 0))).unwrap()
                .set_preferred_hash_algorithms(vec![HashAlgorithm::SHA512]).unwrap()
                .sign_direct_key(&mut pair, key.parts_as_public()).unwrap();

            let rev1 = signature::SignatureBuilder::new(SignatureType::KeyRevocation)
                .set_signature_creation_time(t2).unwrap()
                .set_reason_for_revocation(ReasonForRevocation::KeySuperseded,
                                           &b""[..]).unwrap()
                .sign_direct_key(&mut pair, key.parts_as_public()).unwrap();

            let bind2 = signature::SignatureBuilder::new(SignatureType::DirectKey)
                .set_features(Features::sequoia()).unwrap()
                .set_key_flags(KeyFlags::empty()).unwrap()
                .set_signature_creation_time(t3).unwrap()
                .set_key_validity_period(Some(time::Duration::new(10 * 52 * 7 * 24 * 60 * 60, 0))).unwrap()
                .set_preferred_hash_algorithms(vec![HashAlgorithm::SHA512]).unwrap()
                .sign_direct_key(&mut pair, key.parts_as_public()).unwrap();

            let rev2 = signature::SignatureBuilder::new(SignatureType::KeyRevocation)
                .set_signature_creation_time(t4).unwrap()
                .set_reason_for_revocation(ReasonForRevocation::KeyCompromised,
                                           &b""[..]).unwrap()
                .sign_direct_key(&mut pair, key.parts_as_public()).unwrap();

            (bind1, rev1, bind2, rev2)
        };
        let pk : key::PublicKey = key.into();
        let cert = Cert::try_from(vec![
            pk.into(),
            bind1.into(),
            bind2.into(),
            rev1.into()
        ]).unwrap();

        let f1: f32 = thread_rng().sample(Open01);
        let f2: f32 = thread_rng().sample(Open01);
        let f3: f32 = thread_rng().sample(Open01);
        let f4: f32 = thread_rng().sample(Open01);
        let te1 = t1 - time::Duration::new((60. * 60. * 24. * 300.0 * f1) as u64, 0);
        let t12 = t1 + time::Duration::new((60. * 60. * 24. * 300.0 * f2) as u64, 0);
        let t23 = t2 + time::Duration::new((60. * 60. * 24. * 300.0 * f3) as u64, 0);
        let t34 = t3 + time::Duration::new((60. * 60. * 24. * 300.0 * f4) as u64, 0);

        assert_eq!(cert.revocation_status(p, te1), RevocationStatus::NotAsFarAsWeKnow);
        assert_eq!(cert.revocation_status(p, t12), RevocationStatus::NotAsFarAsWeKnow);
        assert_match!(RevocationStatus::Revoked(_) = cert.revocation_status(p, t23));
        assert_eq!(cert.revocation_status(p, t34), RevocationStatus::NotAsFarAsWeKnow);

        // Merge in the hard revocation.
        let cert = cert.insert_packets(rev2).unwrap();
        assert_match!(RevocationStatus::Revoked(_) = cert.revocation_status(p, te1));
        assert_match!(RevocationStatus::Revoked(_) = cert.revocation_status(p, t12));
        assert_match!(RevocationStatus::Revoked(_) = cert.revocation_status(p, t23));
        assert_match!(RevocationStatus::Revoked(_) = cert.revocation_status(p, t34));
        assert_match!(RevocationStatus::Revoked(_) = cert.revocation_status(p, t4));
        assert_match!(RevocationStatus::Revoked(_)
                      = cert.revocation_status(p, crate::now()));
    }

    #[test]
    fn key_revoked2() {
        tracer!(true, "cert_revoked2", 0);

        let p = &P::new();

        fn cert_revoked<T>(p: &dyn Policy, cert: &Cert, t: T) -> bool
            where T: Into<Option<time::SystemTime>>
        {
            !matches!(
                cert.revocation_status(p, t),
                RevocationStatus::NotAsFarAsWeKnow
            )
        }

        fn subkey_revoked<T>(p: &dyn Policy, cert: &Cert, t: T) -> bool
            where T: Into<Option<time::SystemTime>>
        {
            !matches!(
                cert.subkeys().next().unwrap().bundle().revocation_status(p, t),
                RevocationStatus::NotAsFarAsWeKnow
            )
        }

        let tests : [(&str, Box<dyn Fn(&dyn Policy, &Cert, _) -> bool>); 2] = [
            ("cert", Box::new(cert_revoked)),
            ("subkey", Box::new(subkey_revoked)),
        ];

        for (f, revoked) in tests.iter()
        {
            t!("Checking {} revocation", f);

            t!("Normal key");
            let cert = Cert::from_bytes(
                crate::tests::key(
                    &format!("really-revoked-{}-0-public.pgp", f))).unwrap();
            let selfsig0 = cert.primary_key().with_policy(p, None).unwrap()
                .binding_signature().signature_creation_time().unwrap();

            assert!(!revoked(p, &cert, Some(selfsig0)));
            assert!(!revoked(p, &cert, None));

            t!("Soft revocation");
            let cert = cert.merge_public_and_secret(
                Cert::from_bytes(
                    crate::tests::key(
                        &format!("really-revoked-{}-1-soft-revocation.pgp", f))
                ).unwrap()).unwrap();
            // A soft revocation made after `t` is ignored when
            // determining whether the key is revoked at time `t`.
            assert!(!revoked(p, &cert, Some(selfsig0)));
            assert!(revoked(p, &cert, None));

            t!("New self signature");
            let cert = cert.merge_public_and_secret(
                Cert::from_bytes(
                    crate::tests::key(
                        &format!("really-revoked-{}-2-new-self-sig.pgp", f))
                ).unwrap()).unwrap();
            assert!(!revoked(p, &cert, Some(selfsig0)));
            // Newer self-sig override older soft revocations.
            assert!(!revoked(p, &cert, None));

            t!("Hard revocation");
            let cert = cert.merge_public_and_secret(
                Cert::from_bytes(
                    crate::tests::key(
                        &format!("really-revoked-{}-3-hard-revocation.pgp", f))
                ).unwrap()).unwrap();
            // Hard revocations trump all.
            assert!(revoked(p, &cert, Some(selfsig0)));
            assert!(revoked(p, &cert, None));

            t!("New self signature");
            let cert = cert.merge_public_and_secret(
                Cert::from_bytes(
                    crate::tests::key(
                        &format!("really-revoked-{}-4-new-self-sig.pgp", f))
                ).unwrap()).unwrap();
            assert!(revoked(p, &cert, Some(selfsig0)));
            assert!(revoked(p, &cert, None));
        }
    }

    #[test]
    fn userid_revoked2() {
        fn check_userids<T>(p: &dyn Policy, cert: &Cert, revoked: bool, t: T)
            where T: Into<Option<time::SystemTime>>, T: Copy
        {
            assert_match!(RevocationStatus::NotAsFarAsWeKnow
                          = cert.revocation_status(p, None));

            let mut slim_shady = false;
            let mut eminem = false;
            for b in cert.userids().with_policy(p, t) {
                if b.userid().value() == b"Slim Shady" {
                    assert!(!slim_shady);
                    slim_shady = true;

                    if revoked {
                        assert_match!(RevocationStatus::Revoked(_)
                                      = b.revocation_status());
                    } else {
                        assert_match!(RevocationStatus::NotAsFarAsWeKnow
                                      = b.revocation_status());
                    }
                } else {
                    assert!(!eminem);
                    eminem = true;

                    assert_match!(RevocationStatus::NotAsFarAsWeKnow
                                  = b.revocation_status());
                }
            }

            assert!(slim_shady);
            assert!(eminem);
        }

        fn check_uas<T>(p: &dyn Policy, cert: &Cert, revoked: bool, t: T)
            where T: Into<Option<time::SystemTime>>, T: Copy
        {
            assert_match!(RevocationStatus::NotAsFarAsWeKnow
                          = cert.revocation_status(p, None));

            assert_eq!(cert.user_attributes().count(), 1);
            let ua = cert.user_attributes().next().unwrap();
            if revoked {
                assert_match!(RevocationStatus::Revoked(_)
                              = ua.revocation_status(p, t));
            } else {
                assert_match!(RevocationStatus::NotAsFarAsWeKnow
                              = ua.revocation_status(p, t));
            }
        }

        tracer!(true, "userid_revoked2", 0);

        let p = &P::new();
        let tests : [(&str, Box<dyn Fn(&dyn Policy, &Cert, bool, _)>); 2] = [
            ("userid", Box::new(check_userids)),
            ("user-attribute", Box::new(check_uas)),
        ];

        for (f, check) in tests.iter()
        {
            t!("Checking {} revocation", f);

            t!("Normal key");
            let cert = Cert::from_bytes(
                crate::tests::key(
                    &format!("really-revoked-{}-0-public.pgp", f))).unwrap();

            let now = crate::now();
            let selfsig0
                = cert.userids().with_policy(p, now).map(|b| {
                    b.binding_signature().signature_creation_time().unwrap()
                })
                .max().unwrap();

            check(p, &cert, false, selfsig0);
            check(p, &cert, false, now);

            // A soft-revocation.
            let cert = cert.merge_public_and_secret(
                Cert::from_bytes(
                    crate::tests::key(
                        &format!("really-revoked-{}-1-soft-revocation.pgp", f))
                ).unwrap()).unwrap();

            check(p, &cert, false, selfsig0);
            check(p, &cert, true, now);

            // A new self signature.  This should override the soft-revocation.
            let cert = cert.merge_public_and_secret(
                Cert::from_bytes(
                    crate::tests::key(
                        &format!("really-revoked-{}-2-new-self-sig.pgp", f))
                ).unwrap()).unwrap();

            check(p, &cert, false, selfsig0);
            check(p, &cert, false, now);

            // A hard revocation.  Unlike for Certs, this does NOT trumps
            // everything.
            let cert = cert.merge_public_and_secret(
                Cert::from_bytes(
                    crate::tests::key(
                        &format!("really-revoked-{}-3-hard-revocation.pgp", f))
                ).unwrap()).unwrap();

            check(p, &cert, false, selfsig0);
            check(p, &cert, true, now);

            // A newer self signature.
            let cert = cert.merge_public_and_secret(
                Cert::from_bytes(
                    crate::tests::key(
                        &format!("really-revoked-{}-4-new-self-sig.pgp", f))
                ).unwrap()).unwrap();

            check(p, &cert, false, selfsig0);
            check(p, &cert, false, now);
        }
    }

    #[test]
    fn unrevoked() {
        let p = &P::new();
        let cert =
            Cert::from_bytes(crate::tests::key("un-revoked-userid.pgp")).unwrap();

        for uid in cert.userids().with_policy(p, None) {
            assert_eq!(uid.revocation_status(), RevocationStatus::NotAsFarAsWeKnow);
        }
    }

    #[test]
    fn is_tsk() {
        let cert = Cert::from_bytes(
            crate::tests::key("already-revoked.pgp")).unwrap();
        assert!(! cert.is_tsk());

        let cert = Cert::from_bytes(
            crate::tests::key("already-revoked-private.pgp")).unwrap();
        assert!(cert.is_tsk());
    }

    #[test]
    fn export_only_exports_public_key() {
        let cert = Cert::from_bytes(
            crate::tests::key("testy-new-private.pgp")).unwrap();
        assert!(cert.is_tsk());

        let mut v = Vec::new();
        cert.serialize(&mut v).unwrap();
        let cert = Cert::from_bytes(&v).unwrap();
        assert!(! cert.is_tsk());
    }

    // Make sure that when merging two Certs, the primary key and
    // subkeys with and without a private key are merged.
    #[test]
    fn public_private_merge() {
        let (tsk, _) = CertBuilder::general_purpose(None, Some("foo@example.com"))
            .generate().unwrap();
        // tsk is now a cert, but it still has its private bits.
        assert!(tsk.primary.key().has_secret());
        assert!(tsk.is_tsk());
        let subkey_count = tsk.subkeys().len();
        assert!(subkey_count > 0);
        assert!(tsk.subkeys().all(|k| k.key().has_secret()));

        // This will write out the tsk as a cert, i.e., without any
        // private bits.
        let mut cert_bytes = Vec::new();
        tsk.serialize(&mut cert_bytes).unwrap();

        // Reading it back in, the private bits have been stripped.
        let cert = Cert::from_bytes(&cert_bytes[..]).unwrap();
        assert!(! cert.primary.key().has_secret());
        assert!(!cert.is_tsk());
        assert!(cert.subkeys().all(|k| ! k.key().has_secret()));

        let merge1 = cert.clone().merge_public_and_secret(tsk.clone()).unwrap();
        assert!(merge1.is_tsk());
        assert!(merge1.primary.key().has_secret());
        assert_eq!(merge1.subkeys().len(), subkey_count);
        assert!(merge1.subkeys().all(|k| k.key().has_secret()));

        let merge2 = tsk.clone().merge_public_and_secret(cert.clone()).unwrap();
        assert!(merge2.is_tsk());
        assert!(merge2.primary.key().has_secret());
        assert_eq!(merge2.subkeys().len(), subkey_count);
        assert!(merge2.subkeys().all(|k| k.key().has_secret()));
    }

    #[test]
    fn issue_120() {
        let cert = "
-----BEGIN PGP ARMORED FILE-----

xcBNBFoVcvoBCACykTKOJddF8SSUAfCDHk86cNTaYnjCoy72rMgWJsrMLnz/V16B
J9M7l6nrQ0JMnH2Du02A3w+kNb5q97IZ/M6NkqOOl7uqjyRGPV+XKwt0G5mN/ovg
8630BZAYS3QzavYf3tni9aikiGH+zTFX5pynTNfYRXNBof3Xfzl92yad2bIt4ITD
NfKPvHRko/tqWbclzzEn72gGVggt1/k/0dKhfsGzNogHxg4GIQ/jR/XcqbDFR3RC
/JJjnTOUPGsC1y82Xlu8udWBVn5mlDyxkad5laUpWWg17anvczEAyx4TTOVItLSu
43iPdKHSs9vMXWYID0bg913VusZ2Ofv690nDABEBAAHNJFRlc3R5IE1jVGVzdGZh
Y2UgPHRlc3R5QGV4YW1wbGUub3JnPsLAlAQTAQgAPhYhBD6Id8h3J0aSl1GJ9dA/
b4ZSJv6LBQJaFXL6AhsDBQkDwmcABQsJCAcCBhUICQoLAgQWAgMBAh4BAheAAAoJ
ENA/b4ZSJv6Lxo8H/1XMt+Nqa6e0SG/up3ypKe5nplA0p/9j/s2EIsP8S8uPUd+c
WS17XOmPwkNDmHeL3J6hzwL74NlYSLEtyf7WoOV74xAKQA9WkqaKPHCtpll8aFWA
ktQDLWTPeKuUuSlobAoRtO17ZmheSQzmm7JYt4Ahkxt3agqGT05OsaAey6nIKqpq
ArokvdHTZ7AFZeSJIWmuCoT9M1lo3LAtLnRGOhBMJ5dDIeOwflJwNBXlJVi4mDPK
+fumV0MbSPvZd1/ivFjSpQyudWWtv1R1nAK7+a4CPTGxPvAQkLtRsL/V+Q7F3BJG
jAn4QVx8p4t3NOPuNgcoZpLBE3sc4Nfs5/CphMLHwE0EWhVy+gEIALSpjYD+tuWC
rj6FGP6crQjQzVlH+7axoM1ooTwiPs4fzzt2iLw3CJyDUviM5F9ZBQTei635RsAR
a/CJTSQYAEU5yXXxhoe0OtwnuvsBSvVT7Fox3pkfNTQmwMvkEbodhfKpqBbDKCL8
f5A8Bb7aISsLf0XRHWDkHVqlz8LnOR3f44wEWiTeIxLc8S1QtwX/ExyW47oPsjs9
ShCmwfSpcngH/vGBRTO7WeI54xcAtKSm/20B/MgrUl5qFo17kUWot2C6KjuZKkHk
3WZmJwQz+6rTB11w4AXt8vKkptYQCkfat2FydGpgRO5dVg6aWNJefOJNkC7MmlzC
ZrrAK8FJ6jcAEQEAAcLAdgQYAQgAIBYhBD6Id8h3J0aSl1GJ9dA/b4ZSJv6LBQJa
FXL6AhsMAAoJENA/b4ZSJv6Lt7kH/jPr5wg8lcamuLj4lydYiLttvvTtDTlD1TL+
IfwVARB/ruoerlEDr0zX1t3DCEcvJDiZfOqJbXtHt70+7NzFXrYxfaNFmikMgSQT
XqHrMQho4qpseVOeJPWGzGOcrxCdw/ZgrWbkDlAU5KaIvk+M4wFPivjbtW2Ro2/F
J4I/ZHhJlIPmM+hUErHC103b08pBENXDQlXDma7LijH5kWhyfF2Ji7Ft0EjghBaW
AeGalQHjc5kAZu5R76Mwt06MEQ/HL1pIvufTFxkr/SzIv8Ih7Kexb0IrybmfD351
Pu1xwz57O4zo1VYf6TqHJzVC3OMvMUM2hhdecMUe5x6GorNaj6g=
=1Vzu
-----END PGP ARMORED FILE-----
";
        assert!(Cert::from_bytes(cert).is_err());
    }

    #[test]
    fn missing_uids() {
        let (cert, _) = CertBuilder::new()
            .add_userid("test1@example.com")
            .add_userid("test2@example.com")
            .add_transport_encryption_subkey()
            .add_certification_subkey()
            .generate().unwrap();
        assert_eq!(cert.subkeys().len(), 2);
        let pile = cert
            .into_packet_pile()
            .into_children()
            .filter(|pkt| {
                match pkt {
                    &Packet::PublicKey(_) | &Packet::PublicSubkey(_)
                    | &Packet::SecretKey(_) | &Packet::SecretSubkey(_) => true,
                    &Packet::Signature(ref sig) => {
                        sig.typ() == SignatureType::DirectKey
                            || sig.typ() == SignatureType::SubkeyBinding
                    }
                    e => {
                        eprintln!("{:?}", e);
                        false
                    }
                }
            })
        .collect::<Vec<_>>();
        eprintln!("parse back");
        let cert = Cert::try_from(pile).unwrap();

        assert_eq!(cert.subkeys().len(), 2);
    }

    #[test]
    fn signature_order() {
        let p = &P::new();
        let neal = Cert::from_bytes(crate::tests::key("neal.pgp")).unwrap();

        // This test is useless if we don't have some lists with more
        // than one signature.
        let mut cmps = 0;

        for uid in neal.userids() {
            for sigs in [
                uid.self_signatures().collect::<Vec<_>>(),
                uid.certifications().collect::<Vec<_>>(),
                uid.self_revocations().collect::<Vec<_>>(),
                uid.other_revocations().collect::<Vec<_>>()
            ].iter() {
                for sigs in sigs.windows(2) {
                    cmps += 1;
                    assert!(sigs[0].signature_creation_time()
                            >= sigs[1].signature_creation_time());
                }
            }

            // Make sure we return the most recent first.
            assert_eq!(uid.self_signatures().next().unwrap(),
                       uid.binding_signature(p, None).unwrap());
        }

        assert!(cmps > 0);
    }

    #[test]
    fn cert_reject_keyrings() {
        let mut keyring = Vec::new();
        keyring.extend_from_slice(crate::tests::key("neal.pgp"));
        keyring.extend_from_slice(crate::tests::key("neal.pgp"));
        assert!(Cert::from_bytes(&keyring).is_err());
    }

    #[test]
    fn primary_userid() {
        // 'really-revoked-userid' has two user ids.  One of them is
        // revoked and then restored.  Neither of the user ids has the
        // primary userid bit set.
        //
        // This test makes sure that Cert::primary_userid prefers
        // unrevoked user ids to revoked user ids, even if the latter
        // have newer self signatures.

        let p = &P::new();
        let cert = Cert::from_bytes(
            crate::tests::key("really-revoked-userid-0-public.pgp")).unwrap();

        let now = crate::now();
        let selfsig0
            = cert.userids().with_policy(p, now).map(|b| {
                b.binding_signature().signature_creation_time().unwrap()
            })
            .max().unwrap();

        // The self-sig for:
        //
        //   Slim Shady: 2019-09-14T14:21
        //   Eminem:     2019-09-14T14:22
        assert_eq!(cert.with_policy(p, selfsig0).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"Eminem");
        assert_eq!(cert.with_policy(p, now).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"Eminem");

        // A soft-revocation for "Slim Shady".
        let cert = cert.merge_public_and_secret(
            Cert::from_bytes(
                crate::tests::key("really-revoked-userid-1-soft-revocation.pgp")
            ).unwrap()).unwrap();

        assert_eq!(cert.with_policy(p, selfsig0).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"Eminem");
        assert_eq!(cert.with_policy(p, now).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"Eminem");

        // A new self signature for "Slim Shady".  This should
        // override the soft-revocation.
        let cert = cert.merge_public_and_secret(
            Cert::from_bytes(
                crate::tests::key("really-revoked-userid-2-new-self-sig.pgp")
            ).unwrap()).unwrap();

        assert_eq!(cert.with_policy(p, selfsig0).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"Eminem");
        assert_eq!(cert.with_policy(p, now).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"Slim Shady");

        // A hard revocation for "Slim Shady".
        let cert = cert.merge_public_and_secret(
            Cert::from_bytes(
                crate::tests::key("really-revoked-userid-3-hard-revocation.pgp")
            ).unwrap()).unwrap();

        assert_eq!(cert.with_policy(p, selfsig0).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"Eminem");
        assert_eq!(cert.with_policy(p, now).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"Eminem");

        // A newer self signature for "Slim Shady". Unlike for Certs, this
        // does NOT trump everything.
        let cert = cert.merge_public_and_secret(
            Cert::from_bytes(
                crate::tests::key("really-revoked-userid-4-new-self-sig.pgp")
            ).unwrap()).unwrap();

        assert_eq!(cert.with_policy(p, selfsig0).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"Eminem");
        assert_eq!(cert.with_policy(p, now).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"Slim Shady");

        // Play with the primary user id flag.

        let cert = Cert::from_bytes(
            crate::tests::key("primary-key-0-public.pgp")).unwrap();
        let selfsig0
            = cert.userids().with_policy(p, now).map(|b| {
                b.binding_signature().signature_creation_time().unwrap()
            })
            .max().unwrap();

        // There is only a single User ID.
        assert_eq!(cert.with_policy(p, selfsig0).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"aaaaa");
        assert_eq!(cert.with_policy(p, now).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"aaaaa");


        // Add a second user id.  Since neither is marked primary, the
        // newer one should be considered primary.
        let cert = cert.merge_public_and_secret(
            Cert::from_bytes(
                crate::tests::key("primary-key-1-add-userid-bbbbb.pgp")
            ).unwrap()).unwrap();

        assert_eq!(cert.with_policy(p, selfsig0).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"aaaaa");
        assert_eq!(cert.with_policy(p, now).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"bbbbb");

        // Mark aaaaa as primary.  It is now primary and the newest one.
        let cert = cert.merge_public_and_secret(
            Cert::from_bytes(
                crate::tests::key("primary-key-2-make-aaaaa-primary.pgp")
            ).unwrap()).unwrap();

        assert_eq!(cert.with_policy(p, selfsig0).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"aaaaa");
        assert_eq!(cert.with_policy(p, now).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"aaaaa");

        // Update the preferences on bbbbb.  It is now the newest, but
        // it is not marked as primary.
        let cert = cert.merge_public_and_secret(
            Cert::from_bytes(
                crate::tests::key("primary-key-3-make-bbbbb-new-self-sig.pgp")
            ).unwrap()).unwrap();

        assert_eq!(cert.with_policy(p, selfsig0).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"aaaaa");
        assert_eq!(cert.with_policy(p, now).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"aaaaa");

        // Mark bbbbb as primary.  It is now the newest and marked as
        // primary.
        let cert = cert.merge_public_and_secret(
            Cert::from_bytes(
                crate::tests::key("primary-key-4-make-bbbbb-primary.pgp")
            ).unwrap()).unwrap();

        assert_eq!(cert.with_policy(p, selfsig0).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"aaaaa");
        assert_eq!(cert.with_policy(p, now).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"bbbbb");

        // Update the preferences on aaaaa.  It is now has the newest
        // self sig, but that self sig does not say that it is
        // primary.
        let cert = cert.merge_public_and_secret(
            Cert::from_bytes(
                crate::tests::key("primary-key-5-make-aaaaa-self-sig.pgp")
            ).unwrap()).unwrap();

        assert_eq!(cert.with_policy(p, selfsig0).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"aaaaa");
        assert_eq!(cert.with_policy(p, now).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"bbbbb");

        // Hard revoke aaaaa.  Unlike with Certs, a hard revocation is
        // not treated specially.
        let cert = cert.merge_public_and_secret(
            Cert::from_bytes(
                crate::tests::key("primary-key-6-revoked-aaaaa.pgp")
            ).unwrap()).unwrap();

        assert_eq!(cert.with_policy(p, selfsig0).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"aaaaa");
        assert_eq!(cert.with_policy(p, now).unwrap()
                   .primary_userid().unwrap().userid().value(),
                   b"bbbbb");
    }

    #[test]
    fn binding_signature_lookup() {
        // Check that searching for the right binding signature works
        // even when there are signatures with the same time.

        use crate::types::Features;
        use crate::packet::key::Key4;

        let p = &P::new();

        let a_sec = time::Duration::new(1, 0);
        let time_zero = time::UNIX_EPOCH;

        let t1 = time::UNIX_EPOCH + time::Duration::new(946681200, 0);  // 2000-1-1
        let t2 = time::UNIX_EPOCH + time::Duration::new(978303600, 0);  // 2001-1-1
        let t3 = time::UNIX_EPOCH + time::Duration::new(1009839600, 0); // 2002-1-1
        let t4 = time::UNIX_EPOCH + time::Duration::new(1041375600, 0); // 2003-1-1

        let mut key: key::SecretKey
            = Key4::generate_ecc(true, Curve::Ed25519).unwrap().into();
        key.set_creation_time(t1).unwrap();
        let mut pair = key.clone().into_keypair().unwrap();
        let pk : key::PublicKey = key.clone().into();
        let mut cert = Cert::try_from(vec![
            pk.into(),
        ]).unwrap();
        let uid: UserID = "foo@example.org".into();
        let sig = uid.certify(&mut pair, &cert,
                              SignatureType::PositiveCertification,
                              None,
                              t1).unwrap();
        cert = cert.insert_packets(
            vec![Packet::from(uid), sig.into()]).unwrap();

        const N: usize = 5;
        for (t, offset) in &[ (t2, 0), (t4, 0), (t3, 1 * N), (t1, 3 * N) ] {
            for i in 0..N {
                let binding = signature::SignatureBuilder::new(SignatureType::DirectKey)
                    .set_features(Features::sequoia()).unwrap()
                    .set_key_flags(KeyFlags::empty()).unwrap()
                    .set_signature_creation_time(t1).unwrap()
                    // Vary this...
                    .set_key_validity_period(Some(
                        time::Duration::new((1 + i as u64) * 24 * 60 * 60, 0)))
                    .unwrap()
                    .set_preferred_hash_algorithms(vec![HashAlgorithm::SHA512]).unwrap()
                    .set_signature_creation_time(*t).unwrap()
                    .sign_direct_key(&mut pair, key.parts_as_public()).unwrap();

                let binding : Packet = binding.into();

                cert = cert.insert_packets(binding).unwrap();
                // A time that matches multiple signatures.
                let direct_signatures =
                    cert.primary_key().bundle().self_signatures();
                assert_eq!(cert.primary_key().with_policy(p, *t).unwrap()
                           .direct_key_signature().ok(),
                           direct_signatures.get(*offset));
                // A time that doesn't match any signature.
                assert_eq!(cert.primary_key().with_policy(p, *t + a_sec).unwrap()
                           .direct_key_signature().ok(),
                           direct_signatures.get(*offset));

                // The current time, which should use the first signature.
                assert_eq!(cert.primary_key().with_policy(p, None).unwrap()
                           .direct_key_signature().ok(),
                           direct_signatures.get(0));

                // The beginning of time, which should return no
                // binding signatures.
                assert!(cert.primary_key().with_policy(p, time_zero).is_err());
            }
        }
    }

    #[test]
    fn keysigning_party() {
        use crate::packet::signature;

        for cs in &[ CipherSuite::Cv25519,
                     CipherSuite::P256,
                     CipherSuite::P384,
                     CipherSuite::P521,
                     CipherSuite::RSA2k ]
        {
            if cs.is_supported().is_err() {
                eprintln!("Skipping {:?} because it is not supported.", cs);
                continue;
            }

            let (alice, _) = CertBuilder::new()
                .set_cipher_suite(*cs)
                .add_userid("alice@foo.com")
                .generate().unwrap();

            let (bob, _) = CertBuilder::new()
                .set_cipher_suite(*cs)
                .add_userid("bob@bar.com")
                .add_signing_subkey()
                .generate().unwrap();

            assert_eq!(bob.userids().len(), 1);
            let bob_userid_binding = bob.userids().next().unwrap();
            assert_eq!(bob_userid_binding.userid().value(), b"bob@bar.com");

            let sig_template
                = signature::SignatureBuilder::new(SignatureType::GenericCertification)
                      .set_trust_signature(255, 120)
                      .unwrap();

            // Have alice certify the binding "bob@bar.com" and bob's key.
            let mut alice_certifies_bob
                = bob_userid_binding.userid().bind(
                    &mut alice.primary_key().key().clone().parts_into_secret()
                        .unwrap().into_keypair().unwrap(),
                    &bob,
                    sig_template).unwrap();

            let bob = bob.insert_packets(alice_certifies_bob.clone()).unwrap();

            // Make sure the certification is merged, and put in the right
            // place.
            assert_eq!(bob.userids().len(), 1);
            let bob_userid_binding = bob.userids().next().unwrap();
            assert_eq!(bob_userid_binding.userid().value(), b"bob@bar.com");

            // Canonicalizing Bob's cert without having Alice's key
            // has to resort to a heuristic to order third party
            // signatures.  However, since we know the signature's
            // type (GenericCertification), we know that it can only
            // go to the only userid, so there is no ambiguity in this
            // case.
            assert_eq!(bob_userid_binding.certifications().collect::<Vec<_>>(),
                       vec![&alice_certifies_bob]);

            // Make sure the certification is correct.
            alice_certifies_bob
                .verify_userid_binding(alice.primary_key().key(),
                                       bob.primary_key().key(),
                                       bob_userid_binding.userid()).unwrap();
        }
   }

    #[test]
    fn decrypt_encrypt_secrets() -> Result<()> {
        let p: crate::crypto::Password = "streng geheim".into();
        let (mut cert, _) = CertBuilder::new()
            .add_transport_encryption_subkey()
            .set_password(Some(p.clone()))
            .generate()?;
        assert_eq!(cert.keys().secret().count(), 2);
        assert_eq!(cert.keys().unencrypted_secret().count(), 0);

        for (i, ka) in cert.clone().keys().secret().enumerate() {
            let key = ka.key().clone().decrypt_secret(&p)?;
            cert = if i == 0 {
                cert.insert_packets(key.role_into_primary())?
            } else {
                cert.insert_packets(key.role_into_subordinate())?
            };
            assert_eq!(cert.keys().secret().count(), 2);
            assert_eq!(cert.keys().unencrypted_secret().count(), i + 1);
        }

        assert_eq!(cert.keys().secret().count(), 2);
        assert_eq!(cert.keys().unencrypted_secret().count(), 2);

        for (i, ka) in cert.clone().keys().secret().enumerate() {
            let key = ka.key().clone().encrypt_secret(&p)?;
            cert = if i == 0 {
                cert.insert_packets(key.role_into_primary())?
            } else {
                cert.insert_packets(key.role_into_subordinate())?
            };
            assert_eq!(cert.keys().secret().count(), 2);
            assert_eq!(cert.keys().unencrypted_secret().count(), 2 - 1 - i);
        }

        assert_eq!(cert.keys().secret().count(), 2);
        assert_eq!(cert.keys().unencrypted_secret().count(), 0);
        Ok(())
    }

    /// Tests that Cert::into_packets() and Cert::serialize(..) agree.
    #[test]
    fn test_into_packets() -> Result<()> {
        use crate::serialize::SerializeInto;

        let dkg = Cert::from_bytes(crate::tests::key("dkg.gpg"))?;
        let mut buf = Vec::new();
        for p in dkg.clone().into_packets() {
            p.serialize(&mut buf)?;
        }
        let dkg = dkg.to_vec()?;
        if false && buf != dkg {
            std::fs::write("/tmp/buf", &buf)?;
            std::fs::write("/tmp/dkg", &dkg)?;
        }
        assert_eq!(buf, dkg);
        Ok(())
    }

    #[test]
    fn test_canonicalization() -> Result<()> {
        let p = crate::policy::StandardPolicy::new();

        let primary: Key<_, key::PrimaryRole> =
            key::Key4::generate_ecc(true, Curve::Ed25519)?.into();
        let mut primary_pair = primary.clone().into_keypair()?;
        let cert = Cert::try_from(vec![primary.into()])?;

        // We now add components without binding signatures.  They
        // should be kept, be enumerable, but ignored if a policy is
        // applied.

        // Add a bare userid.
        let uid = UserID::from("foo@example.org");
        let cert = cert.insert_packets(uid)?;
        assert_eq!(cert.userids().count(), 1);
        assert_eq!(cert.userids().with_policy(&p, None).count(), 0);

        // Add a bare user attribute.
        use packet::user_attribute::{Subpacket, Image};
        let ua = UserAttribute::new(&[
            Subpacket::Image(
                Image::Private(100, vec![0, 1, 2].into_boxed_slice())),
        ])?;
        let cert = cert.insert_packets(ua)?;
        assert_eq!(cert.user_attributes().count(), 1);
        assert_eq!(cert.user_attributes().with_policy(&p, None).count(), 0);

        // Add a bare signing subkey.
        let signing_subkey: Key<_, key::SubordinateRole> =
            key::Key4::generate_ecc(true, Curve::Ed25519)?.into();
        let _signing_subkey_pair = signing_subkey.clone().into_keypair()?;
        let cert = cert.insert_packets(signing_subkey)?;
        assert_eq!(cert.keys().subkeys().count(), 1);
        assert_eq!(cert.keys().subkeys().with_policy(&p, None).count(), 0);

        // Add a component that Sequoia doesn't understand.
        let mut fake_key = packet::Unknown::new(
            packet::Tag::PublicSubkey, anyhow::anyhow!("fake key"));
        fake_key.set_body("fake key".into());
        let fake_binding = signature::SignatureBuilder::new(
                SignatureType::Unknown(SignatureType::SubkeyBinding.into()))
            .sign_standalone(&mut primary_pair)?;
        let cert = cert.insert_packets(vec![Packet::from(fake_key),
                                           fake_binding.clone().into()])?;
        assert_eq!(cert.unknowns().count(), 1);
        assert_eq!(cert.unknowns().next().unwrap().unknown().tag(),
                   packet::Tag::PublicSubkey);
        assert_eq!(cert.unknowns().next().unwrap().self_signatures().collect::<Vec<_>>(),
                   vec![&fake_binding]);

        Ok(())
    }

    #[test]
    fn canonicalize_with_v3_sig() -> Result<()> {
        if ! crate::types::PublicKeyAlgorithm::DSA.is_supported() {
            eprintln!("Skipping because DSA is not supported");
            return Ok(());
        }

        // This test relies on being able to validate SHA-1
        // signatures.  The standard policy rejects SHA-1.  So, use a
        // custom policy.
        let p = &P::new();
        let sha1 =
            p.hash_cutoff(
                HashAlgorithm::SHA1, HashAlgoSecurity::CollisionResistance)
            .unwrap();
        let p = &P::at(sha1 - std::time::Duration::from_secs(1));

        let cert = Cert::from_bytes(
            crate::tests::key("eike-v3-v4.pgp"))?;
        dbg!(&cert);
        assert_eq!(cert.userids()
                   .with_policy(p, None)
                   .count(), 1);
        Ok(())
    }

    /// Asserts that key expiration times on direct key signatures are
    /// honored.
    #[test]
    fn issue_215() {
        let p = &P::new();
         let cert = Cert::from_bytes(crate::tests::key(
            "issue-215-expiration-on-direct-key-sig.pgp")).unwrap();
        assert_match!(
            Error::Expired(_)
                = cert.with_policy(p, None).unwrap().alive()
                .unwrap_err().downcast().unwrap());
        assert_match!(
            Error::Expired(_)
                = cert.primary_key().with_policy(p, None).unwrap()
                    .alive().unwrap_err().downcast().unwrap());
    }

    /// Tests that secrets are kept when merging.
    #[test]
    fn merge_keeps_secrets() -> Result<()> {
        let (cert_s, _) =
            CertBuilder::general_purpose(None, Some("uid")).generate()?;
        let cert_p = cert_s.clone().strip_secret_key_material();

        // Merge key into cert.
        let cert = cert_p.clone().merge_public_and_secret(cert_s.clone())?;
        assert!(cert.keys().all(|ka| ka.has_secret()));

        // Merge cert into key.
        let cert = cert_s.clone().merge_public_and_secret(cert_p.clone())?;
        assert!(cert.keys().all(|ka| ka.has_secret()));

        Ok(())
    }

    /// Tests that secrets that are merged in are preferred to
    /// existing secrets.
    #[test]
    fn merge_prefers_merged_in_secrets() -> Result<()> {
        let pw: crate::crypto::Password = "foo".into();
        let (cert_encrypted_secrets, _) =
            CertBuilder::general_purpose(None, Some("uid"))
            .set_password(Some(pw.clone()))
            .generate()?;

        let mut cert_plain_secrets = cert_encrypted_secrets.clone();
        for ka in cert_encrypted_secrets.keys().secret() {
            assert!(! ka.has_unencrypted_secret());
            let key = ka.key().clone().decrypt_secret(&pw)?;
            assert!(key.has_unencrypted_secret());

            let key: Packet = if ka.primary() {
                key.role_into_primary().into()
            } else {
                key.role_into_subordinate().into()
            };

            cert_plain_secrets =
                cert_plain_secrets.insert_packets(vec![key])?;
        }
        assert!(
            cert_plain_secrets.keys().all(|ka| ka.has_unencrypted_secret()));

        // Merge unencrypted secrets into encrypted secrets.
        let cert = cert_encrypted_secrets.clone().merge_public_and_secret(
            cert_plain_secrets.clone())?;
        assert!(cert.keys().all(|ka| ka.has_unencrypted_secret()));

        // Merge encrypted secrets into unencrypted secrets.
        let cert = cert_plain_secrets.clone().merge_public_and_secret(
            cert_encrypted_secrets.clone())?;
        assert!(cert.keys().all(|ka| ka.has_secret()
                                && ! ka.has_unencrypted_secret()));

        Ok(())
    }

    /// Tests that secrets are kept when canonicalizing.
    #[test]
    fn canonicalizing_keeps_secrets() -> Result<()> {
        let primary: Key<_, key::PrimaryRole> =
            key::Key4::generate_ecc(true, Curve::Ed25519)?.into();
        let mut primary_pair = primary.clone().into_keypair()?;
        let cert = Cert::try_from(vec![primary.clone().into()])?;

        let subkey_sec: Key<_, key::SubordinateRole> =
            key::Key4::generate_ecc(false, Curve::Cv25519)?.into();
        let subkey_pub = subkey_sec.clone().take_secret().0;
        let builder = signature::SignatureBuilder::new(SignatureType::SubkeyBinding)
            .set_key_flags(KeyFlags::empty()
                           .set_transport_encryption())?;
        let binding = subkey_sec.bind(&mut primary_pair, &cert, builder)?;

        let cert = Cert::try_from(vec![
            primary.clone().into(),
            subkey_pub.clone().into(),
            binding.clone().into(),
            subkey_sec.clone().into(),
            binding.clone().into(),
        ])?;
        assert_eq!(cert.keys().subkeys().count(), 1);
        assert_eq!(cert.keys().unencrypted_secret().subkeys().count(), 1);

        let cert = Cert::try_from(vec![
            primary.clone().into(),
            subkey_sec.clone().into(),
            binding.clone().into(),
            subkey_pub.clone().into(),
            binding.clone().into(),
        ])?;
        assert_eq!(cert.keys().subkeys().count(), 1);
        assert_eq!(cert.keys().unencrypted_secret().subkeys().count(), 1);
        Ok(())
    }

    /// Demonstrates that subkeys are kept if a userid is later added
    /// without any keyflags.
    #[test]
    fn issue_361() -> Result<()> {
        let (cert, _) = CertBuilder::new()
            .add_transport_encryption_subkey()
            .generate()?;
        let p = &P::new();
        let cert_at = cert.with_policy(p,
                                       cert.primary_key().creation_time()
                                       + time::Duration::new(300, 0))
            .unwrap();
        assert_eq!(cert_at.userids().count(), 0);
        assert_eq!(cert_at.keys().count(), 2);

        let mut primary_pair = cert.primary_key().key().clone()
            .parts_into_secret()?.into_keypair()?;
        let uid: UserID = "foo@example.org".into();
        let sig = uid.bind(
            &mut primary_pair, &cert,
            signature::SignatureBuilder::new(SignatureType::PositiveCertification))?;
        let cert = cert.insert_packets(vec![
            Packet::from(uid),
            sig.into(),
        ])?;

        let cert_at = cert.with_policy(p,
                                       cert.primary_key().creation_time()
                                       + time::Duration::new(300, 0))
            .unwrap();
        assert_eq!(cert_at.userids().count(), 1);
        assert_eq!(cert_at.keys().count(), 2);
        Ok(())
    }

    /// Demonstrates that binding signatures are considered valid even
    /// if the primary key is not marked as certification-capable.
    #[test]
    fn issue_321() -> Result<()> {
        let cert = Cert::from_bytes(
            crate::tests::file("contrib/pep/pEpkey-netpgp.asc"))?;
        assert_eq!(cert.userids().count(), 1);
        assert_eq!(cert.keys().count(), 1);

        let mut p = P::new();
        p.accept_hash(HashAlgorithm::SHA1);
        let cert_at = cert.with_policy(&p, cert.primary_key().creation_time())
            .unwrap();
        assert_eq!(cert_at.userids().count(), 1);
        assert_eq!(cert_at.keys().count(), 1);
        Ok(())
    }

    #[test]
    fn policy_uri_some() -> Result<()> {
        use crate::packet::prelude::SignatureBuilder;
        use crate::policy::StandardPolicy;

        let p = &StandardPolicy::new();

        let (alice, _) = CertBuilder::new().add_userid("Alice").generate()?;

        let sig = SignatureBuilder::from(
            alice
            .with_policy(p, None)?
            .direct_key_signature().expect("Direct key signature")
            .clone()
        )
            .set_policy_uri("https://example.org/~alice/signing-policy.txt")?;
        assert_eq!(sig.policy_uri(), Some("https://example.org/~alice/signing-policy.txt".as_bytes()));
        Ok(())
    }

    #[test]
    fn policy_uri_none() -> Result<()> {
        use crate::packet::prelude::SignatureBuilder;
        use crate::policy::StandardPolicy;

        let p = &StandardPolicy::new();

        let (alice, _) = CertBuilder::new().add_userid("Alice").generate()?;

        let sig = SignatureBuilder::from(
            alice
            .with_policy(p, None)?
            .direct_key_signature().expect("Direct key signature")
            .clone()
        );
        assert_eq!(sig.policy_uri(), None);
        Ok(())
    }

    #[test]
    fn different_preferences() -> Result<()> {
        use crate::cert::Preferences;
        let p = &crate::policy::StandardPolicy::new();

        // This key returns different preferences depending on how you
        // address it.  (It has two user ids and the user ids have
        // different preference packets on their respective self
        // signatures.)

        let cert = Cert::from_bytes(
            crate::tests::key("different-preferences.asc"))?;
        assert_eq!(cert.userids().count(), 2);

        if let Some(userid) = cert.userids().next() {
            assert_eq!(userid.userid().value(),
                       &b"Alice Confusion <alice@example.com>"[..]);

            let userid = userid.with_policy(p, None).expect("valid");

            use crate::types::SymmetricAlgorithm::*;
            assert_eq!(userid.preferred_symmetric_algorithms(),
                       Some(&[ AES256, AES192, AES128, TripleDES ][..]));

            use crate::types::HashAlgorithm::*;
            assert_eq!(userid.preferred_hash_algorithms(),
                       Some(&[ SHA512, SHA384, SHA256, SHA224, SHA1 ][..]));

            use crate::types::CompressionAlgorithm::*;
            assert_eq!(userid.preferred_compression_algorithms(),
                       Some(&[ Zlib, BZip2, Zip ][..]));

            assert_eq!(userid.preferred_aead_algorithms(), None);

            // assert_eq!(userid.key_server_preferences(),
            //            Some(KeyServerPreferences::new(&[])));

            assert_eq!(userid.features(),
                       Some(Features::new(&[]).set_mdc()));
        } else {
            panic!("two user ids");
        }

        if let Some(userid) = cert.userids().next() {
            assert_eq!(userid.userid().value(),
                       &b"Alice Confusion <alice@example.com>"[..]);

            let userid = userid.with_policy(p, None).expect("valid");

            use crate::types::SymmetricAlgorithm::*;
            assert_eq!(userid.preferred_symmetric_algorithms(),
                       Some(&[ AES256, AES192, AES128, TripleDES ][..]));

            use crate::types::HashAlgorithm::*;
            assert_eq!(userid.preferred_hash_algorithms(),
                       Some(&[ SHA512, SHA384, SHA256, SHA224, SHA1 ][..]));

            use crate::types::CompressionAlgorithm::*;
            assert_eq!(userid.preferred_compression_algorithms(),
                       Some(&[ Zlib, BZip2, Zip ][..]));

            assert_eq!(userid.preferred_aead_algorithms(), None);

            assert_eq!(userid.key_server_preferences(),
                       Some(KeyServerPreferences::new(&[0x80])));

            assert_eq!(userid.features(),
                       Some(Features::new(&[]).set_mdc()));

            // Using the certificate should choose the primary user
            // id, which is this one (because it is lexicographically
            // earlier).
            let cert = cert.with_policy(p, None).expect("valid");
            assert_eq!(userid.preferred_symmetric_algorithms(),
                       cert.preferred_symmetric_algorithms());
            assert_eq!(userid.preferred_hash_algorithms(),
                       cert.preferred_hash_algorithms());
            assert_eq!(userid.preferred_compression_algorithms(),
                       cert.preferred_compression_algorithms());
            assert_eq!(userid.preferred_aead_algorithms(),
                       cert.preferred_aead_algorithms());
            assert_eq!(userid.key_server_preferences(),
                       cert.key_server_preferences());
            assert_eq!(userid.features(),
                       cert.features());
        } else {
            panic!("two user ids");
        }

        if let Some(userid) = cert.userids().nth(1) {
            assert_eq!(userid.userid().value(),
                       &b"Alice Confusion <alice@example.net>"[..]);

            let userid = userid.with_policy(p, None).expect("valid");

            use crate::types::SymmetricAlgorithm::*;
            assert_eq!(userid.preferred_symmetric_algorithms(),
                       Some(&[ AES192, AES256, AES128, TripleDES ][..]));

            use crate::types::HashAlgorithm::*;
            assert_eq!(userid.preferred_hash_algorithms(),
                       Some(&[ SHA384, SHA512, SHA256, SHA224, SHA1 ][..]));

            use crate::types::CompressionAlgorithm::*;
            assert_eq!(userid.preferred_compression_algorithms(),
                       Some(&[ BZip2, Zlib, Zip ][..]));

            assert_eq!(userid.preferred_aead_algorithms(), None);

            assert_eq!(userid.key_server_preferences(),
                       Some(KeyServerPreferences::new(&[0x80])));

            assert_eq!(userid.features(),
                       Some(Features::new(&[]).set_mdc()));
        } else {
            panic!("two user ids");
        }

        Ok(())
    }

    #[test]
    fn unsigned_components() -> Result<()> {
        // We have a certificate with an unsigned User ID, User
        // Attribute, encryption-capable subkey, and signing-capable
        // subkey.  (Actually, they are signed, but the signatures are
        // bad.)  We expect that when we parse such a certificate the
        // unsigned components are not dropped and they appear when
        // iterating over the components using, e.g., Cert::userids,
        // but not when we check for valid components.

        let p = &crate::policy::StandardPolicy::new();

        let cert = Cert::from_bytes(
            crate::tests::key("certificate-with-unsigned-components.asc"))?;

        assert_eq!(cert.userids().count(), 2);
        assert_eq!(cert.userids().with_policy(p, None).count(), 1);

        assert_eq!(cert.user_attributes().count(), 2);
        assert_eq!(cert.user_attributes().with_policy(p, None).count(), 1);

        assert_eq!(cert.keys().count(), 1 + 4);
        assert_eq!(cert.keys().with_policy(p, None).count(), 1 + 2);
        Ok(())
    }

    #[test]
    fn issue_504() -> Result<()> {
        let mut keyring = crate::tests::key("testy.pgp").to_vec();
        keyring.extend_from_slice(crate::tests::key("testy-new.pgp"));

        // TryFrom<PacketPile>
        let pp = PacketPile::from_bytes(&keyring)?;
        assert!(matches!(
            Cert::try_from(pp.clone()).unwrap_err().downcast().unwrap(),
            Error::MalformedCert(_)
        ));

        // Cert::TryFrom<Vec<Packet>>
        let v: Vec<Packet> = pp.into();
        assert!(matches!(
            Cert::try_from(v.clone()).unwrap_err().downcast().unwrap(),
            Error::MalformedCert(_)
        ));

        // Cert::from_packet
        assert!(matches!(
            Cert::from_packets(v.into_iter()).unwrap_err().downcast().unwrap(),
            Error::MalformedCert(_)
        ));

        // Cert::TryFrom<PacketParserResult>
        let ppr = PacketParser::from_bytes(&keyring)?;
        assert!(matches!(
            Cert::try_from(ppr).unwrap_err().downcast().unwrap(),
            Error::MalformedCert(_)
        ));
        Ok(())
    }

    /// Tests whether the policy is applied to primary key binding
    /// signatures.
    #[test]
    fn issue_531() -> Result<()> {
        let cert =
            Cert::from_bytes(crate::tests::key("peter-sha1-backsig.pgp"))?;
        let p = &crate::policy::NullPolicy::new();
        assert_eq!(cert.with_policy(p, None)?.keys().for_signing().count(), 1);
        let mut p = crate::policy::StandardPolicy::new();
        p.reject_hash(HashAlgorithm::SHA1);
        assert_eq!(cert.with_policy(&p, None)?.keys().for_signing().count(), 0);
        Ok(())
    }

    /// Tests whether expired primary key binding signatures are
    /// rejected.
    #[test]
    fn issue_539() -> Result<()> {
        let cert =
            Cert::from_bytes(crate::tests::key("peter-expired-backsig.pgp"))?;
        let p = &crate::policy::NullPolicy::new();
        assert_eq!(cert.with_policy(p, None)?.keys().for_signing().count(), 0);
        let p = &crate::policy::StandardPolicy::new();
        assert_eq!(cert.with_policy(p, None)?.keys().for_signing().count(), 0);
        Ok(())
    }

    /// Tests whether signatures are properly deduplicated.
    #[test]
    fn issue_568() -> Result<()> {
        use crate::packet::signature::subpacket::*;

        let (cert, _) = CertBuilder::general_purpose(
            None, Some("alice@example.org")).generate().unwrap();
        assert_eq!(cert.userids().count(), 1);
        assert_eq!(cert.subkeys().count(), 2);
        assert_eq!(cert.unknowns().count(), 0);
        assert_eq!(cert.bad_signatures().count(), 0);
        assert_eq!(cert.userids().next().unwrap().self_signatures().count(), 1);
        assert_eq!(cert.subkeys().next().unwrap().self_signatures().count(), 1);
        assert_eq!(cert.subkeys().nth(1).unwrap().self_signatures().count(), 1);

        // Create a variant of cert where the signatures have
        // additional information in the unhashed area.
        let cert_b = cert.clone();
        let mut packets = crate::PacketPile::from(cert_b).into_children()
            .collect::<Vec<_>>();
        for p in packets.iter_mut() {
            if let Packet::Signature(sig) = p {
                assert_eq!(sig.hashed_area().subpackets(
                    SubpacketTag::IssuerFingerprint).count(),
                           1);
                sig.unhashed_area_mut().add(Subpacket::new(
                    SubpacketValue::Issuer("AAAA BBBB CCCC DDDD".parse()?),
                    false)?)?;
            }
        }
        let cert_b = Cert::from_packets(packets.into_iter())?;
        let cert = cert.merge_public_and_secret(cert_b)?;
        assert_eq!(cert.userids().count(), 1);
        assert_eq!(cert.subkeys().count(), 2);
        assert_eq!(cert.unknowns().count(), 0);
        assert_eq!(cert.bad_signatures().count(), 0);
        assert_eq!(cert.userids().next().unwrap().self_signatures().count(), 1);
        assert_eq!(cert.subkeys().next().unwrap().self_signatures().count(), 1);
        assert_eq!(cert.subkeys().nth(1).unwrap().self_signatures().count(), 1);

        Ok(())
    }

    /// Checks that missing or bad embedded signatures cause the
    /// signature to be considered bad.
    #[test]
    fn missing_backsig_is_bad() -> Result<()> {
        use crate::packet::{
            key::Key4,
            signature::{
                SignatureBuilder,
                subpacket::{Subpacket, SubpacketValue},
            },
        };

        // We'll study this certificate, because it contains a
        // signing-capable subkey.
        let cert = crate::Cert::from_bytes(crate::tests::key(
            "emmelie-dorothea-dina-samantha-awina-ed25519.pgp"))?;
        let mut pp = crate::PacketPile::from_bytes(crate::tests::key(
            "emmelie-dorothea-dina-samantha-awina-ed25519.pgp"))?;
        assert_eq!(pp.children().count(), 5);

        if let Some(Packet::Signature(sig)) = pp.path_ref_mut(&[4]) {
            // Add a bogus but plausible embedded signature subpacket.
            let key: key::SecretKey
                = Key4::generate_ecc(true, Curve::Ed25519)?.into();
            let mut pair = key.into_keypair()?;

            sig.unhashed_area_mut().replace(Subpacket::new(
                SubpacketValue::EmbeddedSignature(
                    SignatureBuilder::new(SignatureType::PrimaryKeyBinding)
                        .sign_primary_key_binding(
                            &mut pair,
                            cert.primary_key().key(),
                            cert.keys().subkeys().next().unwrap().key())?),
                false)?)?;
        } else {
            panic!("expected a signature");
        }

        // Parse into cert.
        use std::convert::TryFrom;
        let malicious_cert = Cert::try_from(pp)?;
        // The subkey binding signature should no longer check out.
        let p = &crate::policy::StandardPolicy::new();
        assert_eq!(malicious_cert.with_policy(p, None)?.keys().subkeys()
                   .for_signing().count(), 0);
        // Instead, it should be considered bad.
        assert_eq!(malicious_cert.bad_signatures().count(), 1);
        Ok(())
    }

    /// Checks that multiple embedded signatures are correctly
    /// handled.
    #[test]
    fn multiple_embedded_signatures() -> Result<()> {
        use crate::packet::{
            key::Key4,
            signature::{
                SignatureBuilder,
                subpacket::{Subpacket, SubpacketValue},
            },
        };

        // We'll study this certificate, because it contains a
        // signing-capable subkey.
        let cert = crate::Cert::from_bytes(crate::tests::key(
            "emmelie-dorothea-dina-samantha-awina-ed25519.pgp"))?;

        // Add a bogus but plausible embedded signature subpacket with
        // this key.
        let key: key::SecretKey
            = Key4::generate_ecc(true, Curve::Ed25519)?.into();
        let mut pair = key.into_keypair()?;

        // Create a malicious cert to merge in.
        let mut pp = crate::PacketPile::from_bytes(crate::tests::key(
            "emmelie-dorothea-dina-samantha-awina-ed25519.pgp"))?;
        assert_eq!(pp.children().count(), 5);

        if let Some(Packet::Signature(sig)) = pp.path_ref_mut(&[4]) {
            // Prepend a bad backsig.
            let backsig = sig.embedded_signatures().next().unwrap().clone();
            sig.unhashed_area_mut().replace(Subpacket::new(
                SubpacketValue::EmbeddedSignature(
                    SignatureBuilder::new(SignatureType::PrimaryKeyBinding)
                        .sign_primary_key_binding(
                            &mut pair,
                            cert.primary_key().key(),
                            cert.keys().subkeys().next().unwrap().key())?),
                false)?)?;
            sig.unhashed_area_mut().add(Subpacket::new(
                SubpacketValue::EmbeddedSignature(backsig), false)?)?;
        } else {
            panic!("expected a signature");
        }

        // Parse into cert.
        use std::convert::TryFrom;
        let malicious_cert = Cert::try_from(pp)?;
        // The subkey binding signature should still be fine.
        let p = &crate::policy::StandardPolicy::new();
        assert_eq!(malicious_cert.with_policy(p, None)?.keys().subkeys()
                   .for_signing().count(), 1);
        assert_eq!(malicious_cert.bad_signatures().count(), 0);

        // Now try to merge it in.
        let merged = cert.clone().merge_public_and_secret(malicious_cert.clone())?;
        // The subkey binding signature should still be fine.
        assert_eq!(merged.with_policy(p, None)?.keys().subkeys()
                   .for_signing().count(), 1);
        let sig = merged.with_policy(p, None)?.keys().subkeys()
            .for_signing().next().unwrap().binding_signature();
        assert_eq!(sig.embedded_signatures().count(), 2);

        // Now the other way around.
        let merged = malicious_cert.clone().merge_public_and_secret(cert.clone())?;
        // The subkey binding signature should still be fine.
        assert_eq!(merged.with_policy(p, None)?.keys().subkeys()
                   .for_signing().count(), 1);
        let sig = merged.with_policy(p, None)?.keys().subkeys()
            .for_signing().next().unwrap().binding_signature();
        assert_eq!(sig.embedded_signatures().count(), 2);
        Ok(())
    }

    /// Checks that Cert::merge(cert, cert) == cert.
    #[test]
    fn issue_579() -> Result<()> {
        use std::convert::TryFrom;
        use crate::packet::signature::subpacket::SubpacketTag;

        let mut pp = crate::PacketPile::from_bytes(crate::tests::key(
            "emmelie-dorothea-dina-samantha-awina-ed25519.pgp"))?;
        assert_eq!(pp.children().count(), 5);
        // Drop issuer information from the unhashed areas.
        if let Some(Packet::Signature(sig)) = pp.path_ref_mut(&[2]) {
            sig.unhashed_area_mut().remove_all(SubpacketTag::Issuer);
        } else {
            panic!("expected a signature");
        }
        if let Some(Packet::Signature(sig)) = pp.path_ref_mut(&[4]) {
            sig.unhashed_area_mut().remove_all(SubpacketTag::Issuer);
        } else {
            panic!("expected a signature");
        }

        let cert = Cert::try_from(pp)?;
        assert_eq!(cert.clone().merge_public_and_secret(cert.clone())?, cert);

        // Specifically, the issuer information should have been added
        // back by the canonicalization.
        assert_eq!(
            cert.userids().next().unwrap().self_signatures().next().unwrap()
                .unhashed_area().subpackets(SubpacketTag::Issuer).count(),
            1);
        assert_eq!(
            cert.keys().subkeys().next().unwrap().self_signatures().next().unwrap()
                .unhashed_area().subpackets(SubpacketTag::Issuer).count(),
            1);
        Ok(())
    }

    /// Checks that Cert::merge_public ignores secret key material.
    #[test]
    fn merge_public() -> Result<()> {
        let cert =
            Cert::from_bytes(crate::tests::key("testy-new.pgp"))?;
        let key =
            Cert::from_bytes(crate::tests::key("testy-new-private.pgp"))?;

        assert!(! cert.is_tsk());
        assert!(key.is_tsk());

        // Secrets are ignored in `other`.
        let merged = cert.clone().merge_public(key.clone())?;
        assert!(! merged.is_tsk());
        assert_eq!(merged, cert);

        // Secrets are retained in `self`.
        let merged = key.clone().merge_public(cert.clone())?;
        assert!(merged.is_tsk());
        assert_eq!(merged, key);

        Ok(())
    }

    /// Make sure we can parse a key where the primary key is its own
    /// subkeys.
    #[test]
    fn primary_key_is_subkey() -> Result<()> {
        let p = &crate::policy::StandardPolicy::new();

        let cert =
            Cert::from_bytes(crate::tests::key("primary-key-is-also-subkey.pgp"))?;

        // There should be three keys:
        //
        //     Fingerprint: 8E8C 33FA 4626 3379 76D9  7978 069C 0C34 8DD8 2C19
        // Public-key algo: EdDSA Edwards-curve Digital Signature Algorithm
        // Public-key size: 256 bits
        //      Secret key: Unencrypted
        //   Creation time: 2018-06-11 14:12:09 UTC
        //       Key flags: certification, signing
        //
        //          Subkey: 8E8C 33FA 4626 3379 76D9  7978 069C 0C34 8DD8 2C19
        // Public-key algo: EdDSA Edwards-curve Digital Signature Algorithm
        // Public-key size: 256 bits
        //      Secret key: Unencrypted
        //   Creation time: 2018-06-11 14:12:09 UTC
        //       Key flags: certification, signing
        //
        //          Subkey: 061C 3CA4 4AFF 0EC5 8DC6  6E95 22E3 FAFE 96B5 6C32
        // Public-key algo: EdDSA Edwards-curve Digital Signature Algorithm
        // Public-key size: 256 bits
        //      Secret key: Unencrypted
        //   Creation time: 2018-08-27 10:55:43 UTC
        //       Key flags: signing
        //
        //          UserID: Emmelie Dorothea Dina Samantha Awina Ed25519
        assert_eq!(cert.keys().count(), 3);

        // Make sure there is a subkey with the same fingerprint as
        // the primary key.
        assert!(cert.keys().subkeys().any(|k| {
            k.fingerprint() == cert.primary_key().fingerprint()
        }));

        // Make sure the self sig is valid, too.
        assert_eq!(cert.keys().count(), 3);

        let vc = cert.with_policy(p, None)?;
        assert!(vc.keys().subkeys().any(|k| {
            k.fingerprint() == vc.primary_key().fingerprint()
        }));

        Ok(())
    }

    /// Makes sure that attested key signatures are correctly handled.
    #[test]
    fn attested_key_signatures() -> Result<()> {
        use crate::{
            packet::signature::SignatureBuilder,
            types::*,
        };
        let p = &crate::policy::StandardPolicy::new();

        let (alice, _) = CertBuilder::new()
            .add_userid("alice@foo.com")
            .generate()?;
        let mut alice_signer =
            alice.primary_key().key().clone().parts_into_secret()?
            .into_keypair()?;

        let (bob, _) = CertBuilder::new()
            .add_userid("bob@bar.com")
            .generate()?;
        let mut bob_signer =
            bob.primary_key().key().clone().parts_into_secret()?
            .into_keypair()?;
        let bob_pristine = bob.clone();

        // Have Alice certify the binding between "bob@bar.com" and
        // Bob's key.
        let alice_certifies_bob
            = bob.userids().next().unwrap().userid().bind(
                &mut alice_signer, &bob,
                SignatureBuilder::new(SignatureType::GenericCertification))?;
        let bob = bob.insert_packets(vec![
            alice_certifies_bob.clone(),
        ])?;

        assert_eq!(bob.with_policy(p, None)?.userids().next().unwrap()
                   .certifications().count(), 1);
        assert_eq!(bob.with_policy(p, None)?.userids().next().unwrap()
                   .attested_certifications().count(), 0);

        // Have Bob attest that certification.
        let attestations =
            bob.userids().next().unwrap().attest_certifications(
                p,
                &mut bob_signer,
                vec![&alice_certifies_bob])?;
        assert_eq!(attestations.len(), 1);
        let attestation = attestations[0].clone();

        let bob = bob.insert_packets(vec![
            attestation.clone(),
        ])?;

        assert_eq!(bob.bad_signatures().count(), 0);
        assert_eq!(bob.userids().next().unwrap().certifications().next(),
                   Some(&alice_certifies_bob));
        assert_eq!(&bob.userids().next().unwrap().bundle().attestations[0],
                   &attestation);
        assert_eq!(bob.with_policy(p, None)?.userids().next().unwrap()
                   .certifications().count(), 1);
        assert_eq!(bob.with_policy(p, None)?.userids().next().unwrap()
                   .attested_certifications().count(), 1);

        // Check that attested key signatures are kept over merges.
        let bob_ = bob.clone().merge_public(bob_pristine.clone())?;
        assert_eq!(bob_.bad_signatures().count(), 0);
        assert_eq!(bob_.userids().next().unwrap().certifications().next(),
                   Some(&alice_certifies_bob));
        assert_eq!(&bob_.userids().next().unwrap().bundle().attestations[0],
                   &attestation);
        assert_eq!(bob_.with_policy(p, None)?.userids().next().unwrap()
                   .attested_certifications().count(), 1);

        // And the other way around.
        let bob_ = bob_pristine.clone().merge_public(bob.clone())?;
        assert_eq!(bob_.bad_signatures().count(), 0);
        assert_eq!(bob_.userids().next().unwrap().certifications().next(),
                   Some(&alice_certifies_bob));
        assert_eq!(&bob_.userids().next().unwrap().bundle().attestations[0],
                   &attestation);
        assert_eq!(bob_.with_policy(p, None)?.userids().next().unwrap()
                   .attested_certifications().count(), 1);

        // Have Bob withdraw any prior attestations.

        let attestations =
            bob.userids().next().unwrap().attest_certifications(
                p,
                &mut bob_signer,
                &[])?;
        assert_eq!(attestations.len(), 1);
        let attestation = attestations[0].clone();

        let bob = bob.insert_packets(vec![
            attestation.clone(),
        ])?;

        assert_eq!(bob.bad_signatures().count(), 0);
        assert_eq!(bob.userids().next().unwrap().certifications().next(),
                   Some(&alice_certifies_bob));
        assert_eq!(&bob.userids().next().unwrap().bundle().attestations[0],
                   &attestation);
        assert_eq!(bob.with_policy(p, None)?.userids().next().unwrap()
                   .certifications().count(), 1);
        assert_eq!(bob.with_policy(p, None)?.userids().next().unwrap()
                   .attested_certifications().count(), 0);


        Ok(())
    }

    /// Makes sure that attested key signatures are correctly handled.
    #[test]
    fn attested_key_signatures_dkgpg() -> Result<()> {
        const DUMP: bool = false;
        use crate::{
            crypto::hash::Digest,
        };
        let p = &crate::policy::StandardPolicy::new();

        let test = Cert::from_bytes(crate::tests::key("1pa3pc-dkgpg.pgp"))?;
        assert_eq!(test.bad_signatures().count(), 0);
        assert_eq!(test.userids().next().unwrap().certifications().count(),
                   1);
        assert_eq!(test.userids().next().unwrap().bundle().attestations.len(),
                   1);

        let attestation =
            &test.userids().next().unwrap().bundle().attestations[0];

        if DUMP {
            for (i, d) in attestation.attested_certifications()?.enumerate() {
                crate::fmt::hex::Dumper::new(std::io::stderr(), "")
                    .write(d, format!("expected digest {}", i))?;
            }
        }

        let digests: std::collections::HashSet<_> =
            attestation.attested_certifications()?.collect();

        for (i, certification) in
            test.userids().next().unwrap().certifications().enumerate()
        {
            // Hash the certification.
            let mut h = attestation.hash_algo().context()?;
            certification.hash_for_confirmation(&mut h);
            let digest = h.into_digest()?;

            if DUMP {
                crate::fmt::hex::Dumper::new(std::io::stderr(), "")
                    .write(&digest, format!("computed digest {}", i))?;
            }

            assert!(digests.contains(&digest[..]));
        }

        assert_eq!(test.with_policy(p, None)?.userids().next().unwrap()
                   .certifications().count(), 1);
        assert_eq!(test.with_policy(p, None)?.userids().next().unwrap()
                   .attested_certifications().count(), 1);

        Ok(())
    }

    /// Makes sure that marker packets are ignored when parsing certs.
    #[test]
    fn marker_packets() -> Result<()> {
        let cert = Cert::from_bytes(crate::tests::key("neal.pgp"))?;
        let mut buf = Vec::new();
        Packet::Marker(Default::default()).serialize(&mut buf)?;
        cert.serialize(&mut buf)?;

        let cert_ = Cert::from_bytes(&buf)?;
        assert_eq!(cert, cert_);
        Ok(())
    }

    /// Checks that messing with a revocation signature merely
    /// invalidates the signature and keeps the cert's revocation
    /// status unchanged.
    #[test]
    fn issue_486() -> Result<()> {
        use crate::{
            crypto::mpi,
            types::RevocationStatus::*,
            packet::signature::Signature4,
            policy::StandardPolicy,
        };
        let p = &StandardPolicy::new();

        let (cert, revocation) = CertBuilder::new().generate()?;

        // Base case.
        let c = cert.clone().insert_packets(Some(revocation.clone()))?;
        if let Revoked(_) = c.revocation_status(p, None) {
            // cert is considered revoked
        } else {
            panic!("Should be revoked, but is not: {:?}",
                   c.revocation_status(p, None));
        }

        // Breaking the revocation signature by changing the MPIs.
        let c = cert.clone().insert_packets(Some(
            Signature4::new(
                revocation.typ(),
                revocation.pk_algo(),
                revocation.hash_algo(),
                revocation.hashed_area().clone(),
                revocation.unhashed_area().clone(),
                *revocation.digest_prefix(),
                // MPI is replaced with a dummy one
                mpi::Signature::RSA {
                    s: mpi::MPI::from(vec![1, 2, 3])
                })))?;
        if let NotAsFarAsWeKnow = c.revocation_status(p, None) {
            assert_eq!(c.bad_signatures().count(), 1);
        } else {
            panic!("Should not be revoked, but is: {:?}",
                   c.revocation_status(p, None));
        }

        // Breaking the revocation signature by changing the MPIs and
        // the digest prefix.
        let c = cert.clone().insert_packets(Some(
            Signature4::new(
                revocation.typ(),
                revocation.pk_algo(),
                revocation.hash_algo(),
                revocation.hashed_area().clone(),
                revocation.unhashed_area().clone(),
                // Prefix replaced with a dummy one
                [0, 1],
                // MPI is replaced with a dummy one
                mpi::Signature::RSA {
                    s: mpi::MPI::from(vec![1, 2, 3])
                })))?;
        if let NotAsFarAsWeKnow = c.revocation_status(p, None) {
            assert_eq!(c.bad_signatures().count(), 1);
        } else {
            panic!("Should not be revoked, but is: {:?}",
                   c.revocation_status(p, None));
        }

        Ok(())
    }
}
