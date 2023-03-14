//! Signature-related functionality.
//!
//! Signatures are one of the central data structures in OpenPGP.
//! They are used to protect the integrity of both structured
//! documents (e.g., timestamps) and unstructured documents (arbitrary
//! byte sequences) as well as cryptographic data structures.
//!
//! The use of signatures to protect cryptographic data structures is
//! central to making it easy to change an OpenPGP certificate.
//! Consider how a certificate is initially authenticated.  A user,
//! say Alice, securely communicates her certificate's fingerprint to
//! another user, say Bob.  Alice might do this by personally handing
//! Bob a business card with her fingerprint on it.  When Bob is in
//! front of his computer, he may then record that Alice uses the
//! specified key.  Technically, the fingerprint that he used only
//! identifies the primary key: a fingerprint is the hash of the
//! primary key; it does not say anything about any of the rest of the
//! certificate---the subkeys, the User IDs, and the User Attributes.
//! But, because these components are signed by the primary key, we
//! know that the controller of the key intended that they be
//! associated with the certificate.  This mechanism makes it not only
//! possible to add and revoke components, but also to change
//! meta-data, such as a key's expiration time.  If the fingerprint
//! were instead computed over the whole OpenPGP certificate, then
//! changing the certificate would result in a new fingerprint.  In
//! that case, the fingerprint could not be used as a long-term,
//! unique, and stable identifier.
//!
//! Signatures are described in [Section 5.2 of RFC 4880].
//!
//! # Data Types
//!
//! The main signature-related data type is the [`Signature`] enum.
//! This enum abstracts away the differences between the signature
//! formats (the deprecated [version 3], the current [version 4], and
//! the proposed [version 5] formats).  Nevertheless some
//! functionality remains format specific.  For instance, version 4
//! signatures introduced support for storing arbitrary key-value
//! pairs (so-called [notations]).
//!
//! This version of Sequoia only supports version 4 signatures
//! ([`Signature4`]).  However, future versions may include limited
//! support for version 3 signatures to allow working with archived
//! messages, and we intend to add support for version 5 signatures
//! once the new version of the specification has been finalized.
//!
//! When signing a document, a `Signature` is typically created
//! indirectly by the [streaming `Signer`].  Similarly, a `Signature`
//! packet is created as a side effect of parsing a signed message
//! using the [`PacketParser`].
//!
//! The [`SignatureBuilder`] can be used to create a binding
//! signature, a certification, etc.  The motivation for having a
//! separate data structure for creating signatures is that it
//! decreases the chance that a half-constructed signature is
//! accidentally exported.  When modifying an existing signature, you
//! can use, for instance, `SignatureBuilder::from` to convert a
//! `Signature` into a `SignatureBuilder`:
//!
//! ```
//! use sequoia_openpgp as openpgp;
//! use openpgp::policy::StandardPolicy;
//! # use openpgp::cert::prelude::*;
//! # use openpgp::packet::prelude::*;
//!
//! # fn main() -> openpgp::Result<()> {
//! let p = &StandardPolicy::new();
//!
//! # // Generate a new certificate.  It has secret key material.
//! # let (cert, _) = CertBuilder::new()
//! #     .generate()?;
//! #
//! // Create a new direct key signature using the current one as a template.
//! let pk = cert.with_policy(p, None)?.primary_key();
//! let sig = pk.direct_key_signature()?;
//! let builder: SignatureBuilder = sig.clone().into();
//! # Ok(())
//! # }
//! ```
//!
//! For version 4 signatures, attributes are set using so-called
//! subpackets.  Subpackets can be stored in two places: either in the
//! so-called hashed area or in the so-called unhashed area.  Whereas
//! the hashed area's integrity is protected by the signature, the
//! unhashed area is not.  Because an attacker can modify the unhashed
//! area without detection, the unhashed area should only be used for
//! storing self-authenticating data, e.g., the issuer, or a back
//! signature.  It is also sometimes used for [hints].
//! [`Signature::normalize`] removes unexpected subpackets from the
//! unhashed area.  However, due to a lack of context, it does not
//! validate the remaining subpackets.
//!
//! In Sequoia, each subpacket area is represented by a
//! [`SubpacketArea`] data structure.  The two subpacket areas are
//! unified by the [`SubpacketAreas`] data structure, which implements
//! a reasonable policy for looking up subpackets.  In particular, it
//! prefers subpackets from the hashed subpacket area, and only
//! consults the unhashed subpacket area for certain packets.  See
//! [its documentation] for details.
//!
//! [Section 5.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2
//! [`Signature`]: super::Signature
//! [version 3]: https://tools.ietf.org/html/rfc1991#section-5.2.2
//! [version 4]: https://tools.ietf.org/html/rfc4880#section-5.2.3
//! [version 5]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#name-version-4-and-5-signature-p
//! [notations]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
//! [streaming `Signer`]: crate::serialize::stream::Signer
//! [`PacketParser`]: crate::parse::PacketParser
//! [hints]: https://tools.ietf.org/html/rfc4880#section-5.13
//! [`Signature::normalize`]: super::Signature::normalize()
//! [`SubpacketArea`]: subpacket::SubpacketArea
//! [`SubpacketAreas`]: subpacket::SubpacketAreas
//! [its documentation]: subpacket::SubpacketAreas

use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::hash::Hasher;
use std::ops::{Deref, DerefMut};
use std::time::SystemTime;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::Error;
use crate::Result;
use crate::crypto::{
    mpi,
    hash::{self, Hash, Digest},
    Signer,
};
use crate::KeyID;
use crate::KeyHandle;
use crate::HashAlgorithm;
use crate::PublicKeyAlgorithm;
use crate::SignatureType;
use crate::packet::Signature;
use crate::packet::{
    key,
    Key,
};
use crate::packet::UserID;
use crate::packet::UserAttribute;
use crate::Packet;
use crate::packet;
use crate::packet::signature::subpacket::{
    Subpacket,
    SubpacketArea,
    SubpacketAreas,
    SubpacketTag,
    SubpacketValue,
};
use crate::types::Timestamp;

#[cfg(test)]
/// Like quickcheck::Arbitrary, but bounded.
trait ArbitraryBounded {
    /// Generates an arbitrary value, but only recurses if `depth >
    /// 0`.
    fn arbitrary_bounded(g: &mut Gen, depth: usize) -> Self;
}

#[cfg(test)]
/// Default depth when implementing Arbitrary using ArbitraryBounded.
const DEFAULT_ARBITRARY_DEPTH: usize = 2;

#[cfg(test)]
macro_rules! impl_arbitrary_with_bound {
    ($typ:path) => {
        impl Arbitrary for $typ {
            fn arbitrary(g: &mut Gen) -> Self {
                Self::arbitrary_bounded(
                    g,
                    crate::packet::signature::DEFAULT_ARBITRARY_DEPTH)
            }
        }
    }
}

pub mod subpacket;

/// How many seconds to backdate signatures.
///
/// When creating certificates (more specifically, binding
/// signatures), and when updating binding signatures (creating
/// signatures from templates), we backdate the signatures by this
/// amount if no creation time is explicitly given.  Backdating the
/// certificate by a minute has the advantage that the certificate can
/// immediately be customized:
///
/// In order to reliably override a binding signature, the
/// overriding binding signature must be newer than the existing
/// signature.  If, however, the existing signature is created
/// `now`, any newer signature must have a future creation time,
/// and is considered invalid by Sequoia.  To avoid this, we
/// backdate certificate creation times (and hence binding
/// signature creation times), so that there is "space" between
/// the creation time and now for signature updates.
pub(crate) const SIG_BACKDATE_BY: u64 = 60;

/// The data stored in a `Signature` packet.
///
/// This data structure contains exactly those fields that appear in a
/// [`Signature` packet].  It is used by both the [`Signature4`] and
/// the [`SignatureBuilder`] data structures, which include other
/// auxiliary information.  This data structure is public so that
/// `Signature4` and `SignatureBuilder` can deref to it.
///
/// A `SignatureField` derefs to a [`SubpacketAreas`].
///
/// [`Signature`]: https://tools.ietf.org/html/rfc4880#section-5.2
/// [`SubpacketAreas`]: subpacket::SubpacketAreas
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct SignatureFields {
    /// Version of the signature packet. Must be 4.
    version: u8,
    /// Type of signature.
    typ: SignatureType,
    /// Public-key algorithm used for this signature.
    pk_algo: PublicKeyAlgorithm,
    /// Hash algorithm used to compute the signature.
    hash_algo: HashAlgorithm,
    /// Subpackets.
    subpackets: SubpacketAreas,
}
assert_send_and_sync!(SignatureFields);

#[cfg(test)]
impl ArbitraryBounded for SignatureFields {
    fn arbitrary_bounded(g: &mut Gen, depth: usize) -> Self {
        SignatureFields {
            // XXX: Make this more interesting once we dig other
            // versions.
            version: 4,
            typ: Arbitrary::arbitrary(g),
            pk_algo: PublicKeyAlgorithm::arbitrary_for_signing(g),
            hash_algo: Arbitrary::arbitrary(g),
            subpackets: ArbitraryBounded::arbitrary_bounded(g, depth),
        }
    }
}

#[cfg(test)]
impl_arbitrary_with_bound!(SignatureFields);

impl Deref for SignatureFields {
    type Target = SubpacketAreas;

    fn deref(&self) -> &Self::Target {
        &self.subpackets
    }
}

impl DerefMut for SignatureFields {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.subpackets
    }
}

impl SignatureFields {
    /// Gets the version.
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Gets the signature type.
    ///
    /// This function is called `typ` and not `type`, because `type`
    /// is a reserved word.
    pub fn typ(&self) -> SignatureType {
        self.typ
    }

    /// Gets the public key algorithm.
    ///
    /// This is `pub(crate)`, because it shouldn't be exported by
    /// `SignatureBuilder` where it is only set at the end.
    pub(crate) fn pk_algo(&self) -> PublicKeyAlgorithm {
        self.pk_algo
    }

    /// Gets the hash algorithm.
    pub fn hash_algo(&self) -> HashAlgorithm {
        self.hash_algo
    }
}

/// A Signature builder.
///
/// The `SignatureBuilder` is used to create [`Signature`]s.  Although
/// it can be used to generate a signature over a document (using
/// [`SignatureBuilder::sign_message`]), it is usually better to use
/// the [streaming `Signer`] for that.
///
///   [`Signature`]: super::Signature
///   [streaming `Signer`]: crate::serialize::stream::Signer
///   [`SignatureBuilder::sign_message`]: SignatureBuilder::sign_message()
///
/// Oftentimes, you won't want to create a new signature from scratch,
/// but modify a copy of an existing signature.  This is
/// straightforward to do since `SignatureBuilder` implements [`From`]
/// for Signature.
///
///   [`From`]: std::convert::From
///
/// When converting a `Signature` to a `SignatureBuilder`, the hash
/// algorithm is reset to the default hash algorithm
/// (`HashAlgorithm::Default()`).  This ensures that a recommended
/// hash algorithm is used even when an old signature is used as a
/// template, which is often the case when updating self signatures,
/// and binding signatures.
///
/// According to [Section 5.2.3.4 of RFC 4880], `Signatures` must
/// include a [`Signature Creation Time`] subpacket.  Since this
/// should usually be set to the current time, and is easy to forget
/// to update, we remove any `Signature Creation Time` subpackets
/// from both the hashed subpacket area and the unhashed subpacket
/// area when converting a `Signature` to a `SignatureBuilder`, and
/// when the `SignatureBuilder` is finalized, we automatically insert
/// a `Signature Creation Time` subpacket into the hashed subpacket
/// area unless the `Signature Creation Time` subpacket has been set
/// using the [`set_signature_creation_time`] method or preserved
/// using the [`preserve_signature_creation_time`] method or
/// suppressed using the [`suppress_signature_creation_time`] method.
///
/// If the `SignatureBuilder` has been created from scratch, the
/// current time is used as signature creation time.  If it has been
/// created from a template, we make sure that the generated signature
/// is newer.  If that is not possible (i.e. the generated signature
/// would have a future creation time), the signing operation fails.
/// This ensures that binding signatures can be updated by deriving a
/// `SignatureBuilder` from the existing binding.  To disable this,
/// explicitly set a signature creation time, or preserve the original
/// one, or suppress the insertion of a timestamp.
///
///   [Section 5.2.3.4 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
///   [`set_signature_creation_time`]: SignatureBuilder::set_signature_creation_time()
///   [`preserve_signature_creation_time`]: SignatureBuilder::preserve_signature_creation_time()
///   [`suppress_signature_creation_time`]: SignatureBuilder::suppress_signature_creation_time()
///
/// Similarly, most OpenPGP implementations cannot verify a signature
/// if neither the [`Issuer`] subpacket nor the [`Issuer Fingerprint`]
/// subpacket has been correctly set.  To avoid subtle bugs due to the
/// use of a stale `Issuer` subpacket or a stale `Issuer Fingerprint`
/// subpacket, we remove any `Issuer` subpackets, and `Issuer
/// Fingerprint` subpackets from both the hashed and unhashed areas
/// when converting a `Signature` to a `SigantureBuilder`.  Since the
/// [`Signer`] passed to the finalization routine contains the
/// required information, we also automatically add appropriate
/// `Issuer` and `Issuer Fingerprint` subpackets to the hashed
/// subpacket area when the `SignatureBuilder` is finalized unless an
/// `Issuer` subpacket or an `IssuerFingerprint` subpacket has been
/// added to either of the subpacket areas (which can be done using
/// the [`set_issuer`] method and the [`set_issuer_fingerprint`]
/// method, respectively).
///
///   [`Issuer`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
///   [`Issuer Fingerprint`]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
///   [`Signer`]: super::super::crypto::Signer
///   [`set_issuer`]: SignatureBuilder::set_issuer()
///   [`set_issuer_fingerprint`]: SignatureBuilder::set_issuer_fingerprint()
///
/// To finalize the builder, call [`sign_hash`], [`sign_message`],
/// [`sign_direct_key`], [`sign_subkey_binding`],
/// [`sign_primary_key_binding`], [`sign_userid_binding`],
/// [`sign_user_attribute_binding`], [`sign_standalone`], or
/// [`sign_timestamp`], as appropriate.  These functions turn the
/// `SignatureBuilder` into a valid `Signature`.
///
///   [`sign_hash`]: SignatureBuilder::sign_hash()
///   [`sign_message`]: SignatureBuilder::sign_message()
///   [`sign_direct_key`]: SignatureBuilder::sign_direct_key()
///   [`sign_subkey_binding`]: SignatureBuilder::sign_subkey_binding()
///   [`sign_primary_key_binding`]: SignatureBuilder::sign_primary_key_binding()
///   [`sign_userid_binding`]: SignatureBuilder::sign_userid_binding()
///   [`sign_user_attribute_binding`]: SignatureBuilder::sign_user_attribute_binding()
///   [`sign_standalone`]: SignatureBuilder::sign_standalone()
///   [`sign_timestamp`]: SignatureBuilder::sign_timestamp()
///
/// This structure `Deref`s to its containing [`SignatureFields`]
/// structure, which in turn `Deref`s to its subpacket areas
/// (a [`SubpacketAreas`]).
///
///   [`SubpacketAreas`]: subpacket::SubpacketAreas
///
/// # Examples
///
/// Update a certificate's feature set by updating the `Features`
/// subpacket on any direct key signature, and any User ID binding
/// signatures.  See the [`Preferences`] trait for how preferences
/// like these are looked up.
///
/// [`Preferences`]: crate::cert::Preferences
///
/// ```
/// use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
/// use openpgp::packet::prelude::*;
/// use openpgp::packet::signature::subpacket::{Subpacket, SubpacketValue};
/// use openpgp::policy::StandardPolicy;
/// use openpgp::types::Features;
///
/// # fn main() -> openpgp::Result<()> {
/// let p = &StandardPolicy::new();
///
/// let (cert, _) = CertBuilder::new().add_userid("Alice").generate()?;
///
/// // Derive a signer (the primary key is always certification capable).
/// let pk = cert.primary_key().key();
/// let mut signer = pk.clone().parts_into_secret()?.into_keypair()?;
///
/// let mut sigs = Vec::new();
///
/// let vc = cert.with_policy(p, None)?;
///
/// if let Ok(sig) = vc.direct_key_signature() {
///     sigs.push(SignatureBuilder::from(sig.clone())
///         .modify_hashed_area(|mut a| {
///             a.replace(Subpacket::new(
///                 SubpacketValue::Features(Features::sequoia().set(10)),
///                 false)?)?;
///             Ok(a)
///         })?
///         // Update the direct key signature.
///         .sign_direct_key(&mut signer, None)?);
/// }
///
/// for ua in vc.userids() {
///     sigs.push(SignatureBuilder::from(ua.binding_signature().clone())
///         .modify_hashed_area(|mut a| {
///             a.replace(Subpacket::new(
///                 SubpacketValue::Features(Features::sequoia().set(10)),
///                 false)?)?;
///             Ok(a)
///         })?
///         // Update the binding signature.
///         .sign_userid_binding(&mut signer, pk, ua.userid())?);
/// }
///
/// // Merge in the new signatures.
/// let cert = cert.insert_packets(sigs.into_iter().map(Packet::from))?;
/// # assert_eq!(cert.bad_signatures().count(), 0);
/// # Ok(())
/// # }
/// ```
// IMPORTANT: If you add fields to this struct, you need to explicitly
// IMPORTANT: implement PartialEq, Eq, and Hash.
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct SignatureBuilder {
    reference_time: Option<SystemTime>,
    overrode_creation_time: bool,
    original_creation_time: Option<SystemTime>,
    fields: SignatureFields,
}
assert_send_and_sync!(SignatureBuilder);

impl Deref for SignatureBuilder {
    type Target = SignatureFields;

    fn deref(&self) -> &Self::Target {
        &self.fields
    }
}

impl DerefMut for SignatureBuilder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fields
    }
}

impl SignatureBuilder {
    /// Returns a new `SignatureBuilder` object.
    pub fn new(typ: SignatureType) ->  Self {
        SignatureBuilder {
            reference_time: None,
            overrode_creation_time: false,
            original_creation_time: None,
            fields: SignatureFields {
                version: 4,
                typ,
                pk_algo: PublicKeyAlgorithm::Unknown(0),
                hash_algo: HashAlgorithm::default(),
                subpackets: SubpacketAreas::default(),
            }
        }
    }

    /// Sets the signature type.
    pub fn set_type(mut self, t: SignatureType) -> Self {
        self.typ = t;
        self
    }

    /// Sets the hash algorithm.
    pub fn set_hash_algo(mut self, h: HashAlgorithm) -> Self {
        self.hash_algo = h;
        self
    }

    /// Generates a standalone signature.
    ///
    /// A [Standalone Signature] ([`SignatureType::Standalone`]) is a
    /// self-contained signature, which is only over the signature
    /// packet.
    ///
    ///   [Standalone Signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///   [`SignatureType::Standalone`]: crate::types::SignatureType::Standalone
    ///
    /// This function checks that the [signature type] (passed to
    /// [`SignatureBuilder::new`], set via
    /// [`SignatureBuilder::set_type`], or copied when using
    /// `SignatureBuilder::From`) is [`SignatureType::Standalone`] or
    /// [`SignatureType::Unknown`].
    ///
    ///   [signature type]: crate::types::SignatureType
    ///   [`SignatureBuilder::new`]: SignatureBuilder::new()
    ///   [`SignatureBuilder::set_type`]: SignatureBuilder::set_type()
    ///   [`SignatureType::Timestamp`]: crate::types::SignatureType::Timestamp
    ///   [`SignatureType::Unknown`]: crate::types::SignatureType::Unknown
    ///
    /// The [`Signature`]'s public-key algorithm field is set to the
    /// algorithm used by `signer`.
    ///
    ///   [`Signature`]: super::Signature
    ///
    /// If neither an [`Issuer`] subpacket (set using
    /// [`SignatureBuilder::set_issuer`], for instance) nor an
    /// [`Issuer Fingerprint`] subpacket (set using
    /// [`SignatureBuilder::set_issuer_fingerprint`], for instance) is
    /// set, they are both added to the new `Signature`'s hashed
    /// subpacket area and set to the `signer`'s `KeyID` and
    /// `Fingerprint`, respectively.
    ///
    ///   [`Issuer`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///   [`SignatureBuilder::set_issuer`]: SignatureBuilder::set_issuer()
    ///   [`Issuer Fingerprint`]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///   [`SignatureBuilder::set_issuer_fingerprint`]: SignatureBuilder::set_issuer_fingerprint()
    ///
    /// Likewise, a [`Signature Creation Time`] subpacket set to the
    /// current time is added to the hashed area if the `Signature
    /// Creation Time` subpacket hasn't been set using, for instance,
    /// the [`set_signature_creation_time`] method or the
    /// [`preserve_signature_creation_time`] method.
    ///
    ///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///   [`set_signature_creation_time`]: SignatureBuilder::set_signature_creation_time()
    ///   [`preserve_signature_creation_time`]: SignatureBuilder::preserve_signature_creation_time()
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().add_signing_subkey().generate()?;
    ///
    /// // Get a usable (alive, non-revoked) signing key.
    /// let key : &Key<_, _> = cert
    ///     .keys().with_policy(p, None)
    ///     .for_signing().alive().revoked(false).nth(0).unwrap().key();
    /// // Derive a signer.
    /// let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let mut sig = SignatureBuilder::new(SignatureType::Standalone)
    ///     .sign_standalone(&mut signer)?;
    ///
    /// // Verify it.
    /// sig.verify_standalone(signer.public())?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn sign_standalone(mut self, signer: &mut dyn Signer)
                           -> Result<Signature>
    {
        match self.typ {
            SignatureType::Standalone => (),
            SignatureType::Unknown(_) => (),
            _ => return Err(Error::UnsupportedSignatureType(self.typ).into()),
        }

        self = self.pre_sign(signer)?;

        let mut hash = self.hash_algo().context()?;
        self.hash_standalone(&mut hash);
        self.sign(signer, hash.into_digest()?)
    }

    /// Generates a Timestamp Signature.
    ///
    /// Like a [Standalone Signature] (created using
    /// [`SignatureBuilder::sign_standalone`]), a [Timestamp
    /// Signature] is a self-contained signature, but its emphasis in
    /// on the contained timestamp, specifically, the timestamp stored
    /// in the [`Signature Creation Time`] subpacket.  This type of
    /// signature is primarily used by [timestamping services].  To
    /// timestamp a signature, you can include either a [Signature
    /// Target subpacket] (set using
    /// [`SignatureBuilder::set_signature_target`]), or an [Embedded
    /// Signature] (set using
    /// [`SignatureBuilder::set_embedded_signature`]) in the hashed
    /// area.
    ///
    ///
    ///   [Standalone Signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///   [`SignatureBuilder::sign_standalone`]: SignatureBuilder::sign_standalone()
    ///   [Timestamp Signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///   [timestamping services]: https://en.wikipedia.org/wiki/Trusted_timestamping
    ///   [Signature Target subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.25
    ///   [`SignatureBuilder::set_signature_target`]: SignatureBuilder::set_signature_target()
    ///   [Embedded Signature]: https://tools.ietf.org/html/rfc4880#section-5.2.3.26
    ///   [`SignatureBuilder::set_embedded_signature`]: SignatureBuilder::set_embedded_signature()
    ///
    /// This function checks that the [signature type] (passed to
    /// [`SignatureBuilder::new`], set via
    /// [`SignatureBuilder::set_type`], or copied when using
    /// `SignatureBuilder::From`) is [`SignatureType::Timestamp`] or
    /// [`SignatureType::Unknown`].
    ///
    ///   [signature type]: crate::types::SignatureType
    ///   [`SignatureBuilder::new`]: SignatureBuilder::new()
    ///   [`SignatureBuilder::set_type`]: SignatureBuilder::set_type()
    ///   [`SignatureType::Timestamp`]: crate::types::SignatureType::Timestamp
    ///   [`SignatureType::Unknown`]: crate::types::SignatureType::Unknown
    ///
    /// The [`Signature`]'s public-key algorithm field is set to the
    /// algorithm used by `signer`.
    ///
    ///   [`Signature`]: super::Signature
    ///
    /// If neither an [`Issuer`] subpacket (set using
    /// [`SignatureBuilder::set_issuer`], for instance) nor an
    /// [`Issuer Fingerprint`] subpacket (set using
    /// [`SignatureBuilder::set_issuer_fingerprint`], for instance) is
    /// set, they are both added to the new `Signature`'s hashed
    /// subpacket area and set to the `signer`'s `KeyID` and
    /// `Fingerprint`, respectively.
    ///
    ///   [`Issuer`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///   [`SignatureBuilder::set_issuer`]: SignatureBuilder::set_issuer()
    ///   [`Issuer Fingerprint`]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///   [`SignatureBuilder::set_issuer_fingerprint`]: SignatureBuilder::set_issuer_fingerprint()
    ///
    /// Likewise, a [`Signature Creation Time`] subpacket set to the
    /// current time is added to the hashed area if the `Signature
    /// Creation Time` subpacket hasn't been set using, for instance,
    /// the [`set_signature_creation_time`] method or the
    /// [`preserve_signature_creation_time`] method.
    ///
    ///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///   [`set_signature_creation_time`]: SignatureBuilder::set_signature_creation_time()
    ///   [`preserve_signature_creation_time`]: SignatureBuilder::preserve_signature_creation_time()
    ///
    /// # Examples
    ///
    /// Create a timestamp signature:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().add_signing_subkey().generate()?;
    ///
    /// // Get a usable (alive, non-revoked) signing key.
    /// let key : &Key<_, _> = cert
    ///     .keys().with_policy(p, None)
    ///     .for_signing().alive().revoked(false).nth(0).unwrap().key();
    /// // Derive a signer.
    /// let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let mut sig = SignatureBuilder::new(SignatureType::Timestamp)
    ///     .sign_timestamp(&mut signer)?;
    ///
    /// // Verify it.
    /// sig.verify_timestamp(signer.public())?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn sign_timestamp(mut self, signer: &mut dyn Signer)
                          -> Result<Signature>
    {
        match self.typ {
            SignatureType::Timestamp => (),
            SignatureType::Unknown(_) => (),
            _ => return Err(Error::UnsupportedSignatureType(self.typ).into()),
        }

        self = self.pre_sign(signer)?;

        let mut hash = self.hash_algo().context()?;
        self.hash_timestamp(&mut hash);
        self.sign(signer, hash.into_digest()?)
    }

    /// Generates a Direct Key Signature.
    ///
    /// A [Direct Key Signature] is a signature over the primary key.
    /// It is primarily used to hold fallback [preferences].  For
    /// instance, when addressing the Certificate by a User ID, the
    /// OpenPGP implementation is supposed to look for preferences
    /// like the [Preferred Symmetric Algorithms] on the User ID, and
    /// only if there is no such packet, look on the direct key
    /// signature.
    ///
    /// This function is also used to create a [Key Revocation
    /// Signature], which revokes the certificate.
    ///
    ///   [preferences]: crate::cert::Preferences
    ///   [Direct Key Signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///   [Preferred Symmetric Algorithms]: https://tools.ietf.org/html/rfc4880#section-5.2.3.7
    ///   [Key Revocation Signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///
    /// This function checks that the [signature type] (passed to
    /// [`SignatureBuilder::new`], set via
    /// [`SignatureBuilder::set_type`], or copied when using
    /// `SignatureBuilder::From`) is [`SignatureType::DirectKey`],
    /// [`SignatureType::KeyRevocation`], or
    /// [`SignatureType::Unknown`].
    ///
    ///   [signature type]: crate::types::SignatureType
    ///   [`SignatureBuilder::new`]: SignatureBuilder::new()
    ///   [`SignatureBuilder::set_type`]: SignatureBuilder::set_type()
    ///   [`SignatureType::DirectKey`]: crate::types::SignatureType::DirectKey
    ///   [`SignatureType::KeyRevocation`]: crate::types::SignatureType::KeyRevocation
    ///   [`SignatureType::Unknown`]: crate::types::SignatureType::Unknown
    ///
    /// The [`Signature`]'s public-key algorithm field is set to the
    /// algorithm used by `signer`.
    ///
    ///   [`Signature`]: super::Signature
    ///
    /// If neither an [`Issuer`] subpacket (set using
    /// [`SignatureBuilder::set_issuer`], for instance) nor an
    /// [`Issuer Fingerprint`] subpacket (set using
    /// [`SignatureBuilder::set_issuer_fingerprint`], for instance) is
    /// set, they are both added to the new `Signature`'s hashed
    /// subpacket area and set to the `signer`'s `KeyID` and
    /// `Fingerprint`, respectively.
    ///
    ///   [`Issuer`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///   [`SignatureBuilder::set_issuer`]: SignatureBuilder::set_issuer()
    ///   [`Issuer Fingerprint`]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///   [`SignatureBuilder::set_issuer_fingerprint`]: SignatureBuilder::set_issuer_fingerprint()
    ///
    /// Likewise, a [`Signature Creation Time`] subpacket set to the
    /// current time is added to the hashed area if the `Signature
    /// Creation Time` subpacket hasn't been set using, for instance,
    /// the [`set_signature_creation_time`] method or the
    /// [`preserve_signature_creation_time`] method.
    ///
    ///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///   [`set_signature_creation_time`]: SignatureBuilder::set_signature_creation_time()
    ///   [`preserve_signature_creation_time`]: SignatureBuilder::preserve_signature_creation_time()
    ///
    /// If `pk` is set to `None` the signature will be computed over the public key
    /// retrieved from the `signer` parameter, i.e. a self-signature will be created.
    ///  To create a third-party-signature provide an explicit public key as the
    /// `pk` parameter.
    ///
    /// # Examples
    ///
    /// Set the default value for the [Preferred Symmetric Algorithms
    /// subpacket]:
    ///
    /// [Preferred Symmetric Algorithms subpacket]: SignatureBuilder::set_preferred_symmetric_algorithms()
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    /// use openpgp::types::SymmetricAlgorithm;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().add_signing_subkey().generate()?;
    ///
    /// // Get a usable (alive, non-revoked) certification key.
    /// let key : &Key<_, _> = cert
    ///     .keys().with_policy(p, None)
    ///     .for_certification().alive().revoked(false).nth(0).unwrap().key();
    /// // Derive a signer.
    /// let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
    ///
    /// // A direct key signature is always over the primary key.
    /// let pk = cert.primary_key().key();
    ///
    /// // Modify the existing direct key signature.
    /// let mut sig = SignatureBuilder::from(
    ///         cert.with_policy(p, None)?.direct_key_signature()?.clone())
    ///     .set_preferred_symmetric_algorithms(
    ///         vec![ SymmetricAlgorithm::AES256,
    ///               SymmetricAlgorithm::AES128,
    ///         ])?
    ///     .sign_direct_key(&mut signer, None)?;
    ///
    /// // Verify it.
    /// sig.verify_direct_key(signer.public(), pk)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn sign_direct_key<'a, PK>(mut self, signer: &mut dyn Signer,
                              pk: PK)
        -> Result<Signature>
    where PK: Into<Option<&'a Key<key::PublicParts, key::PrimaryRole>>>
    {
        match self.typ {
            SignatureType::DirectKey => (),
            SignatureType::KeyRevocation => (),
            SignatureType::Unknown(_) => (),
            _ => return Err(Error::UnsupportedSignatureType(self.typ).into()),
        }

        self = self.pre_sign(signer)?;

        let mut hash = self.hash_algo().context()?;
        let pk = pk.into().unwrap_or_else(|| signer.public().role_as_primary());
        self.hash_direct_key(&mut hash, pk);

        self.sign(signer, hash.into_digest()?)
    }

    /// Generates a User ID binding signature.
    ///
    /// A User ID binding signature (a self signature) or a [User ID
    /// certification] (a third-party signature) is a signature over a
    /// `User ID` and a `Primary Key` made by a certification-capable
    /// key.  It asserts that the signer is convinced that the `User
    /// ID` should be associated with the `Certificate`, i.e., that
    /// the binding is authentic.
    ///
    ///   [User ID certification]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///
    /// OpenPGP has four types of `User ID` certifications.  They are
    /// intended to express the degree of the signer's conviction,
    /// i.e., how well the signer authenticated the binding.  In
    /// practice, the `Positive Certification` type is used for
    /// self-signatures, and the `Generic Certification` is used for
    /// third-party certifications; the other types are not normally
    /// used.
    ///
    /// This function is also used to create [Certification
    /// Revocations].
    ///
    ///   [Certification Revocations]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///
    /// This function checks that the [signature type] (passed to
    /// [`SignatureBuilder::new`], set via
    /// [`SignatureBuilder::set_type`], or copied when using
    /// `SignatureBuilder::From`) is [`GenericCertification`],
    /// [`PersonaCertification`], [`CasualCertification`],
    /// [`PositiveCertification`], [`CertificationRevocation`], or
    /// [`SignatureType::Unknown`].
    ///
    ///   [signature type]: crate::types::SignatureType
    ///   [`SignatureBuilder::new`]: SignatureBuilder::new()
    ///   [`SignatureBuilder::set_type`]: SignatureBuilder::set_type()
    ///   [`GenericCertification`]: crate::types::SignatureType::GenericCertification
    ///   [`PersonaCertification`]: crate::types::SignatureType::PersonaCertification
    ///   [`CasualCertification`]: crate::types::SignatureType::CasualCertification
    ///   [`PositiveCertification`]: crate::types::SignatureType::PositiveCertification
    ///   [`CertificationRevocation`]: crate::types::SignatureType::CertificationRevocation
    ///   [`SignatureType::Unknown`]: crate::types::SignatureType::Unknown
    ///
    /// The [`Signature`]'s public-key algorithm field is set to the
    /// algorithm used by `signer`.
    ///
    ///   [`Signature`]: super::Signature
    ///
    /// If neither an [`Issuer`] subpacket (set using
    /// [`SignatureBuilder::set_issuer`], for instance) nor an
    /// [`Issuer Fingerprint`] subpacket (set using
    /// [`SignatureBuilder::set_issuer_fingerprint`], for instance) is
    /// set, they are both added to the new `Signature`'s hashed
    /// subpacket area and set to the `signer`'s `KeyID` and
    /// `Fingerprint`, respectively.
    ///
    ///   [`Issuer`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///   [`SignatureBuilder::set_issuer`]: SignatureBuilder::set_issuer()
    ///   [`Issuer Fingerprint`]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///   [`SignatureBuilder::set_issuer_fingerprint`]: SignatureBuilder::set_issuer_fingerprint()
    ///
    /// Likewise, a [`Signature Creation Time`] subpacket set to the
    /// current time is added to the hashed area if the `Signature
    /// Creation Time` subpacket hasn't been set using, for instance,
    /// the [`set_signature_creation_time`] method or the
    /// [`preserve_signature_creation_time`] method.
    ///
    ///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///   [`set_signature_creation_time`]: SignatureBuilder::set_signature_creation_time()
    ///   [`preserve_signature_creation_time`]: SignatureBuilder::preserve_signature_creation_time()
    ///
    /// If `pk` is set to `None` the signature will be computed over the public key
    /// retrieved from the `signer` parameter, i.e. a self-signature will be created.
    ///  To create a third-party-signature provide an explicit public key as the
    /// `pk` parameter.
    ///
    /// # Examples
    ///
    /// Set the [Preferred Symmetric Algorithms subpacket], which will
    /// be used when addressing the certificate via the associated
    /// User ID:
    ///
    /// [Preferred Symmetric Algorithms subpacket]: SignatureBuilder::set_preferred_symmetric_algorithms()
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SymmetricAlgorithm;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().add_userid("Alice").generate()?;
    ///
    /// // Get a usable (alive, non-revoked) certification key.
    /// let key : &Key<_, _> = cert
    ///     .keys().with_policy(p, None)
    ///     .for_certification().alive().revoked(false).nth(0).unwrap().key();
    /// // Derive a signer.
    /// let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
    ///
    /// // Update the User ID's binding signature.
    /// let ua = cert.with_policy(p, None)?.userids().nth(0).unwrap();
    /// let mut new_sig = SignatureBuilder::from(
    ///         ua.binding_signature().clone())
    ///     .set_preferred_symmetric_algorithms(
    ///         vec![ SymmetricAlgorithm::AES256,
    ///               SymmetricAlgorithm::AES128,
    ///         ])?
    ///     .sign_userid_binding(&mut signer, None, ua.userid())?;
    ///
    /// // Verify it.
    /// let pk = cert.primary_key().key();
    ///
    /// new_sig.verify_userid_binding(signer.public(), pk, ua.userid())?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn sign_userid_binding<'a, PK>(mut self, signer: &mut dyn Signer,
                                  key: PK, userid: &UserID)
        -> Result<Signature>
        where PK: Into<Option<&'a Key<key::PublicParts, key::PrimaryRole>>>
    {
        match self.typ {
            SignatureType::GenericCertification => (),
            SignatureType::PersonaCertification => (),
            SignatureType::CasualCertification => (),
            SignatureType::PositiveCertification => (),
            SignatureType::CertificationRevocation => (),
            SignatureType::Unknown(_) => (),
            _ => return Err(Error::UnsupportedSignatureType(self.typ).into()),
        }

        self = self.pre_sign(signer)?;

        let key = key.into().unwrap_or_else(|| signer.public().role_as_primary());

        let mut hash = self.hash_algo().context()?;
        self.hash_userid_binding(&mut hash, key, userid);
        self.sign(signer, hash.into_digest()?)
    }

    /// Generates a subkey binding signature.
    ///
    /// A [subkey binding signature] is a signature over the primary
    /// key and a subkey, which is made by the primary key.  It is an
    /// assertion by the certificate that the subkey really belongs to
    /// the certificate.  That is, it binds the subkey to the
    /// certificate.
    ///
    /// Note: this function does not create a back signature, which is
    /// needed by certification-capable, signing-capable, and
    /// authentication-capable subkeys.  A back signature can be
    /// created using [`SignatureBuilder::sign_primary_key_binding`].
    ///
    /// This function is also used to create subkey revocations.
    ///
    ///   [subkey binding signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///   [`SignatureBuilder::sign_primary_key_binding`]: SignatureBuilder::sign_primary_key_binding()
    ///
    /// This function checks that the [signature type] (passed to
    /// [`SignatureBuilder::new`], set via
    /// [`SignatureBuilder::set_type`], or copied when using
    /// `SignatureBuilder::From`) is
    /// [`SignatureType::SubkeyBinding`], [`SignatureType::SubkeyRevocation`], or
    /// [`SignatureType::Unknown`].
    ///
    ///   [signature type]: crate::types::SignatureType
    ///   [`SignatureBuilder::new`]: SignatureBuilder::new()
    ///   [`SignatureBuilder::set_type`]: SignatureBuilder::set_type()
    ///   [`SignatureType::SubkeyBinding`]: crate::types::SignatureType::SubkeyBinding
    ///   [`SignatureType::SubkeyRevocation`]: crate::types::SignatureType::SubkeyRevocation
    ///   [`SignatureType::Unknown`]: crate::types::SignatureType::Unknown
    ///
    /// The [`Signature`]'s public-key algorithm field is set to the
    /// algorithm used by `signer`.
    ///
    ///   [`Signature`]: super::Signature
    ///
    /// If neither an [`Issuer`] subpacket (set using
    /// [`SignatureBuilder::set_issuer`], for instance) nor an
    /// [`Issuer Fingerprint`] subpacket (set using
    /// [`SignatureBuilder::set_issuer_fingerprint`], for instance) is
    /// set, they are both added to the new `Signature`'s hashed
    /// subpacket area and set to the `signer`'s `KeyID` and
    /// `Fingerprint`, respectively.
    ///
    ///   [`Issuer`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///   [`SignatureBuilder::set_issuer`]: SignatureBuilder::set_issuer()
    ///   [`Issuer Fingerprint`]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///   [`SignatureBuilder::set_issuer_fingerprint`]: SignatureBuilder::set_issuer_fingerprint()
    ///
    /// Likewise, a [`Signature Creation Time`] subpacket set to the
    /// current time is added to the hashed area if the `Signature
    /// Creation Time` subpacket hasn't been set using, for instance,
    /// the [`set_signature_creation_time`] method or the
    /// [`preserve_signature_creation_time`] method.
    ///
    ///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///   [`set_signature_creation_time`]: SignatureBuilder::set_signature_creation_time()
    ///   [`preserve_signature_creation_time`]: SignatureBuilder::preserve_signature_creation_time()
    ///
    /// If `pk` is set to `None` the signature will be computed over the public key
    /// retrieved from the `signer` parameter.
    ///
    /// # Examples
    ///
    /// Add a new subkey intended for encrypting data in motion to an
    /// existing certificate:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::{Curve, KeyFlags, SignatureType};
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().generate()?;
    /// # assert_eq!(cert.keys().count(), 1);
    ///
    /// let pk = cert.primary_key().key().clone().parts_into_secret()?;
    /// // Derive a signer.
    /// let mut pk_signer = pk.clone().into_keypair()?;
    ///
    /// // Generate an encryption subkey.
    /// let mut subkey: Key<_, _> =
    ///     Key4::generate_ecc(false, Curve::Cv25519)?.into();
    /// // Derive a signer.
    /// let mut sk_signer = subkey.clone().into_keypair()?;
    ///
    /// let sig = SignatureBuilder::new(SignatureType::SubkeyBinding)
    ///     .set_key_flags(KeyFlags::empty().set_transport_encryption())?
    ///     .sign_subkey_binding(&mut pk_signer, None, &subkey)?;
    ///
    /// let cert = cert.insert_packets(vec![Packet::SecretSubkey(subkey),
    ///                                    sig.into()])?;
    ///
    /// assert_eq!(cert.with_policy(p, None)?.keys().count(), 2);
    /// # Ok(())
    /// # }
    /// ```
    pub fn sign_subkey_binding<'a, PK, Q>(mut self, signer: &mut dyn Signer,
                                     primary: PK,
                                     subkey: &Key<Q, key::SubordinateRole>)
        -> Result<Signature>
        where Q: key::KeyParts,
              PK: Into<Option<&'a Key<key::PublicParts, key::PrimaryRole>>>,
    {
        match self.typ {
            SignatureType::SubkeyBinding => (),
            SignatureType::SubkeyRevocation => (),
            SignatureType::Unknown(_) => (),
            _ => return Err(Error::UnsupportedSignatureType(self.typ).into()),
        }

        self = self.pre_sign(signer)?;

        let primary = primary.into().unwrap_or_else(|| signer.public().role_as_primary());
        let mut hash = self.hash_algo().context()?;
        self.hash_subkey_binding(&mut hash, primary, subkey);
        self.sign(signer, hash.into_digest()?)
    }

    /// Generates a primary key binding signature.
    ///
    /// A [primary key binding signature], also referred to as a back
    /// signature or backsig, is a signature over the primary key and
    /// a subkey, which is made by the subkey.  This signature is a
    /// statement by the subkey that it belongs to the primary key.
    /// That is, it binds the certificate to the subkey.  It is
    /// normally stored in the subkey binding signature (see
    /// [`SignatureBuilder::sign_subkey_binding`]) in the [`Embedded
    /// Signature`] subpacket (set using
    /// [`SignatureBuilder::set_embedded_signature`]).
    ///
    ///   [primary key binding signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///   [`SignatureBuilder::sign_subkey_binding`]: SignatureBuilder::sign_subkey_binding()
    ///   [`Embedded Signature`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.26
    ///   [`SignatureBuilder::set_embedded_signature`]: SignatureBuilder::set_embedded_signature()
    ///
    /// All subkeys that make signatures of any sort (signature
    /// subkeys, certification subkeys, and authentication subkeys)
    /// must include this signature in their binding signature.  This
    /// signature ensures that an attacker (Mallory) can't claim
    /// someone else's (Alice's) signing key by just creating a subkey
    /// binding signature.  If that were the case, anyone who has
    /// Mallory's certificate could be tricked into thinking that
    /// Mallory made signatures that were actually made by Alice.
    /// This signature prevents this attack, because it proves that
    /// the person who controls the private key for the primary key
    /// also controls the private key for the subkey and therefore
    /// intended that the subkey be associated with the primary key.
    /// Thus, although Mallory controls his own primary key and can
    /// issue a subkey binding signature for Alice's signing key, he
    /// doesn't control her signing key, and therefore can't create a
    /// valid backsig.
    ///
    /// A primary key binding signature is not needed for
    /// encryption-capable subkeys.  This is firstly because
    /// encryption-capable keys cannot make signatures.  But also
    /// because an attacker doesn't gain anything by adopting an
    /// encryption-capable subkey: without the private key material,
    /// they still can't read the message's content.
    ///
    /// This function checks that the [signature type] (passed to
    /// [`SignatureBuilder::new`], set via
    /// [`SignatureBuilder::set_type`], or copied when using
    /// `SignatureBuilder::From`) is
    /// [`SignatureType::PrimaryKeyBinding`], or
    /// [`SignatureType::Unknown`].
    ///
    ///   [signature type]: crate::types::SignatureType
    ///   [`SignatureBuilder::new`]: SignatureBuilder::new()
    ///   [`SignatureBuilder::set_type`]: SignatureBuilder::set_type()
    ///   [`SignatureType::PrimaryKeyBinding`]: crate::types::SignatureType::PrimaryKeyBinding
    ///   [`SignatureType::Unknown`]: crate::types::SignatureType::Unknown
    ///
    /// The [`Signature`]'s public-key algorithm field is set to the
    /// algorithm used by `signer`.
    ///
    ///   [`Signature`]: super::Signature
    ///
    /// If neither an [`Issuer`] subpacket (set using
    /// [`SignatureBuilder::set_issuer`], for instance) nor an
    /// [`Issuer Fingerprint`] subpacket (set using
    /// [`SignatureBuilder::set_issuer_fingerprint`], for instance) is
    /// set, they are both added to the new `Signature`'s hashed
    /// subpacket area and set to the `signer`'s `KeyID` and
    /// `Fingerprint`, respectively.
    ///
    ///   [`Issuer`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///   [`SignatureBuilder::set_issuer`]: SignatureBuilder::set_issuer()
    ///   [`Issuer Fingerprint`]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///   [`SignatureBuilder::set_issuer_fingerprint`]: SignatureBuilder::set_issuer_fingerprint()
    ///
    /// Likewise, a [`Signature Creation Time`] subpacket set to the
    /// current time is added to the hashed area if the `Signature
    /// Creation Time` subpacket hasn't been set using, for instance,
    /// the [`set_signature_creation_time`] method or the
    /// [`preserve_signature_creation_time`] method.
    ///
    ///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///   [`set_signature_creation_time`]: SignatureBuilder::set_signature_creation_time()
    ///   [`preserve_signature_creation_time`]: SignatureBuilder::preserve_signature_creation_time()
    ///
    /// # Examples
    ///
    /// Add a new signing-capable subkey to an existing certificate.
    /// Because we are adding a signing-capable subkey, the binding
    /// signature needs to include a backsig.
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::{Curve, KeyFlags, SignatureType};
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().generate()?;
    /// # assert_eq!(cert.keys().count(), 1);
    ///
    /// let pk = cert.primary_key().key().clone().parts_into_secret()?;
    /// // Derive a signer.
    /// let mut pk_signer = pk.clone().into_keypair()?;
    ///
    /// // Generate a signing subkey.
    /// let mut subkey: Key<_, _> =
    ///     Key4::generate_ecc(true, Curve::Ed25519)?.into();
    /// // Derive a signer.
    /// let mut sk_signer = subkey.clone().into_keypair()?;
    ///
    /// let sig = SignatureBuilder::new(SignatureType::SubkeyBinding)
    ///     .set_key_flags(KeyFlags::empty().set_signing())?
    ///     // The backsig.  This is essential for subkeys that create signatures!
    ///     .set_embedded_signature(
    ///         SignatureBuilder::new(SignatureType::PrimaryKeyBinding)
    ///             .sign_primary_key_binding(&mut sk_signer, &pk, &subkey)?)?
    ///     .sign_subkey_binding(&mut pk_signer, None, &subkey)?;
    ///
    /// let cert = cert.insert_packets(vec![Packet::SecretSubkey(subkey),
    ///                                    sig.into()])?;
    ///
    /// assert_eq!(cert.with_policy(p, None)?.keys().count(), 2);
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn sign_primary_key_binding<P, Q>(mut self,
                                          subkey_signer: &mut dyn Signer,
                                          primary: &Key<P, key::PrimaryRole>,
                                          subkey: &Key<Q, key::SubordinateRole>)
        -> Result<Signature>
        where P: key::KeyParts,
              Q: key::KeyParts,
    {
        match self.typ {
            SignatureType::PrimaryKeyBinding => (),
            SignatureType::Unknown(_) => (),
            _ => return Err(Error::UnsupportedSignatureType(self.typ).into()),
        }

        self = self.pre_sign(subkey_signer)?;

        let mut hash = self.hash_algo().context()?;
        self.hash_primary_key_binding(&mut hash, primary, subkey);
        self.sign(subkey_signer, hash.into_digest()?)
    }


    /// Generates a User Attribute binding signature.
    ///
    /// A User Attribute binding signature or certification, a type of
    /// [User ID certification], is a signature over a User Attribute
    /// and a Primary Key.  It asserts that the signer is convinced
    /// that the User Attribute should be associated with the
    /// Certificate, i.e., that the binding is authentic.
    ///
    ///   [User ID certification]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///
    /// OpenPGP has four types of User Attribute certifications.  They
    /// are intended to express the degree of the signer's conviction.
    /// In practice, the `Positive Certification` type is used for
    /// self-signatures, and the `Generic Certification` is used for
    /// third-party certifications; the other types are not normally
    /// used.
    ///
    /// This function checks that the [signature type] (passed to
    /// [`SignatureBuilder::new`], set via
    /// [`SignatureBuilder::set_type`], or copied when using
    /// `SignatureBuilder::From`) is [`GenericCertification`],
    /// [`PersonaCertification`], [`CasualCertification`],
    /// [`PositiveCertification`], [`CertificationRevocation`], or
    /// [`SignatureType::Unknown`].
    ///
    ///   [signature type]: crate::types::SignatureType
    ///   [`SignatureBuilder::new`]: SignatureBuilder::new()
    ///   [`SignatureBuilder::set_type`]: SignatureBuilder::set_type()
    ///   [`GenericCertification`]: crate::types::SignatureType::GenericCertification
    ///   [`PersonaCertification`]: crate::types::SignatureType::PersonaCertification
    ///   [`CasualCertification`]: crate::types::SignatureType::CasualCertification
    ///   [`PositiveCertification`]: crate::types::SignatureType::PositiveCertification
    ///   [`CertificationRevocation`]: crate::types::SignatureType::CertificationRevocation
    ///   [`SignatureType::Unknown`]: crate::types::SignatureType::Unknown
    ///
    /// The [`Signature`]'s public-key algorithm field is set to the
    /// algorithm used by `signer`.
    ///
    ///   [`Signature`]: super::Signature
    ///
    /// If neither an [`Issuer`] subpacket (set using
    /// [`SignatureBuilder::set_issuer`], for instance) nor an
    /// [`Issuer Fingerprint`] subpacket (set using
    /// [`SignatureBuilder::set_issuer_fingerprint`], for instance) is
    /// set, they are both added to the new `Signature`'s hashed
    /// subpacket area and set to the `signer`'s `KeyID` and
    /// `Fingerprint`, respectively.
    ///
    ///   [`Issuer`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///   [`SignatureBuilder::set_issuer`]: SignatureBuilder::set_issuer()
    ///   [`Issuer Fingerprint`]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///   [`SignatureBuilder::set_issuer_fingerprint`]: SignatureBuilder::set_issuer_fingerprint()
    ///
    /// Likewise, a [`Signature Creation Time`] subpacket set to the
    /// current time is added to the hashed area if the `Signature
    /// Creation Time` subpacket hasn't been set using, for instance,
    /// the [`set_signature_creation_time`] method or the
    /// [`preserve_signature_creation_time`] method.
    ///
    ///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///   [`set_signature_creation_time`]: SignatureBuilder::set_signature_creation_time()
    ///   [`preserve_signature_creation_time`]: SignatureBuilder::preserve_signature_creation_time()
    ///
    /// If `pk` is set to `None` the signature will be computed over the public key
    /// retrieved from the `signer` parameter, i.e. a self-signature will be created.
    ///  To create a third-party-signature provide an explicit public key as the
    /// `pk` parameter.
    ///
    /// # Examples
    ///
    /// Add a new User Attribute to an existing certificate:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    /// # use openpgp::packet::user_attribute::{Subpacket, Image};
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// # // Add a bare user attribute.
    /// # let ua = UserAttribute::new(&[
    /// #     Subpacket::Image(
    /// #         Image::Private(100, vec![0, 1, 2].into_boxed_slice())),
    /// # ])?;
    /// #
    /// let (cert, _) = CertBuilder::new().generate()?;
    /// # assert_eq!(cert.user_attributes().count(), 0);
    ///
    /// // Add a user attribute.
    ///
    /// // Get a usable (alive, non-revoked) certification key.
    /// let key : &Key<_, _> = cert
    ///     .keys().with_policy(p, None)
    ///     .for_certification().alive().revoked(false).nth(0).unwrap().key();
    /// // Derive a signer.
    /// let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let pk = cert.primary_key().key();
    ///
    /// let mut sig =
    ///     SignatureBuilder::new(SignatureType::PositiveCertification)
    ///     .sign_user_attribute_binding(&mut signer, None, &ua)?;
    ///
    /// // Verify it.
    /// sig.verify_user_attribute_binding(signer.public(), pk, &ua)?;
    ///
    /// let cert = cert.insert_packets(vec![Packet::from(ua), sig.into()])?;
    /// assert_eq!(cert.with_policy(p, None)?.user_attributes().count(), 1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn sign_user_attribute_binding<'a, PK>(mut self, signer: &mut dyn Signer,
                                          key: PK, ua: &UserAttribute)
        -> Result<Signature>
        where PK: Into<Option<&'a Key<key::PublicParts, key::PrimaryRole>>>
    {
        match self.typ {
            SignatureType::GenericCertification => (),
            SignatureType::PersonaCertification => (),
            SignatureType::CasualCertification => (),
            SignatureType::PositiveCertification => (),
            SignatureType::CertificationRevocation => (),
            SignatureType::Unknown(_) => (),
            _ => return Err(Error::UnsupportedSignatureType(self.typ).into()),
        }

        self = self.pre_sign(signer)?;

        let key = key.into().unwrap_or_else(|| signer.public().role_as_primary());

        let mut hash = self.hash_algo().context()?;
        self.hash_user_attribute_binding(&mut hash, key, ua);
        self.sign(signer, hash.into_digest()?)
    }

    /// Generates a signature.
    ///
    /// This is a low-level function.  Normally, you'll want to use
    /// one of the higher-level functions, like
    /// [`SignatureBuilder::sign_userid_binding`].  But, this function
    /// is useful if you want to create a [`Signature`] for an
    /// unsupported signature type.
    ///
    ///   [`SignatureBuilder::sign_userid_binding`]: SignatureBuilder::sign_userid_binding()
    ///   [`Signature`]: super::Signature
    ///
    /// The `Signature`'s public-key algorithm field is set to the
    /// algorithm used by `signer`.
    ///
    /// If neither an [`Issuer`] subpacket (set using
    /// [`SignatureBuilder::set_issuer`], for instance) nor an
    /// [`Issuer Fingerprint`] subpacket (set using
    /// [`SignatureBuilder::set_issuer_fingerprint`], for instance) is
    /// set, they are both added to the new `Signature`'s hashed
    /// subpacket area and set to the `signer`'s `KeyID` and
    /// `Fingerprint`, respectively.
    ///
    ///   [`Issuer`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///   [`SignatureBuilder::set_issuer`]: SignatureBuilder::set_issuer()
    ///   [`Issuer Fingerprint`]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///   [`SignatureBuilder::set_issuer_fingerprint`]: SignatureBuilder::set_issuer_fingerprint()
    ///
    /// Likewise, a [`Signature Creation Time`] subpacket set to the
    /// current time is added to the hashed area if the `Signature
    /// Creation Time` subpacket hasn't been set using, for instance,
    /// the [`set_signature_creation_time`] method or the
    /// [`preserve_signature_creation_time`] method.
    ///
    ///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///   [`set_signature_creation_time`]: SignatureBuilder::set_signature_creation_time()
    ///   [`preserve_signature_creation_time`]: SignatureBuilder::preserve_signature_creation_time()
    pub fn sign_hash(mut self, signer: &mut dyn Signer,
                     mut hash: Box<dyn hash::Digest>)
        -> Result<Signature>
    {
        self.hash_algo = hash.algo();

        self = self.pre_sign(signer)?;

        self.hash(&mut hash);
        let mut digest = vec![0u8; hash.digest_size()];
        hash.digest(&mut digest)?;

        self.sign(signer, digest)
    }

    /// Signs a message.
    ///
    /// Normally, you'll want to use the [streaming `Signer`] to sign
    /// a message.
    ///
    ///  [streaming `Signer`]: crate::serialize::stream::Signer
    ///
    /// OpenPGP supports two types of signatures over messages: binary
    /// and text.  The text version normalizes line endings.  But,
    /// since nearly all software today can deal with both Unix and
    /// DOS line endings, it is better to just use the binary version
    /// even when dealing with text.  This avoids any possible
    /// ambiguity.
    ///
    /// This function checks that the [signature type] (passed to
    /// [`SignatureBuilder::new`], set via
    /// [`SignatureBuilder::set_type`], or copied when using
    /// `SignatureBuilder::From`) is [`Binary`], [`Text`], or
    /// [`SignatureType::Unknown`].
    ///
    ///   [signature type]: crate::types::SignatureType
    ///   [`SignatureBuilder::new`]: SignatureBuilder::new()
    ///   [`SignatureBuilder::set_type`]: SignatureBuilder::set_type()
    ///   [`Binary`]: crate::types::SignatureType::Binary
    ///   [`Text`]: crate::types::SignatureType::Text
    ///   [`SignatureType::Unknown`]: crate::types::SignatureType::Unknown
    ///
    /// The [`Signature`]'s public-key algorithm field is set to the
    /// algorithm used by `signer`.
    ///
    ///   [`Signature`]: super::Signature
    ///
    /// If neither an [`Issuer`] subpacket (set using
    /// [`SignatureBuilder::set_issuer`], for instance) nor an
    /// [`Issuer Fingerprint`] subpacket (set using
    /// [`SignatureBuilder::set_issuer_fingerprint`], for instance) is
    /// set, they are both added to the new `Signature`'s hashed
    /// subpacket area and set to the `signer`'s `KeyID` and
    /// `Fingerprint`, respectively.
    ///
    ///   [`Issuer`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///   [`SignatureBuilder::set_issuer`]: SignatureBuilder::set_issuer()
    ///   [`Issuer Fingerprint`]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///   [`SignatureBuilder::set_issuer_fingerprint`]: SignatureBuilder::set_issuer_fingerprint()
    ///
    /// Likewise, a [`Signature Creation Time`] subpacket set to the
    /// current time is added to the hashed area if the `Signature
    /// Creation Time` subpacket hasn't been set using, for instance,
    /// the [`set_signature_creation_time`] method or the
    /// [`preserve_signature_creation_time`] method.
    ///
    ///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///   [`set_signature_creation_time`]: SignatureBuilder::set_signature_creation_time()
    ///   [`preserve_signature_creation_time`]: SignatureBuilder::preserve_signature_creation_time()
    ///
    /// # Examples
    ///
    /// Signs a document.  For large messages, you should use the
    /// [streaming `Signer`], which streams the message's content.
    ///
    ///  [streaming `Signer`]: crate::serialize::stream::Signer
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().generate()?;
    ///
    /// // Get a usable (alive, non-revoked) certification key.
    /// let key : &Key<_, _> = cert
    ///     .keys().with_policy(p, None)
    ///     .for_certification().alive().revoked(false).nth(0).unwrap().key();
    /// // Derive a signer.
    /// let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
    ///
    /// // For large messages, you should use openpgp::serialize::stream::Signer,
    /// // which streams the message's content.
    /// let msg = b"Hello, world!";
    /// let mut sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .sign_message(&mut signer, msg)?;
    ///
    /// // Verify it.
    /// sig.verify_message(signer.public(), msg)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn sign_message<M>(mut self, signer: &mut dyn Signer, msg: M)
        -> Result<Signature>
        where M: AsRef<[u8]>
    {
        match self.typ {
            SignatureType::Binary => (),
            SignatureType::Text => (),
            SignatureType::Unknown(_) => (),
            _ => return Err(Error::UnsupportedSignatureType(self.typ).into()),
        }

        // Hash the message
        let mut hash = self.hash_algo.context()?;
        hash.update(msg.as_ref());

        self = self.pre_sign(signer)?;

        self.hash(&mut hash);
        let mut digest = vec![0u8; hash.digest_size()];
        hash.digest(&mut digest)?;

        self.sign(signer, digest)
    }

    /// Sets the signature builder's default reference time.
    ///
    /// The reference time is used when no time is specified.  The
    /// reference time is the current time by default and is evaluated
    /// on demand.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::{Duration, SystemTime};
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::SignatureType;
    /// use openpgp::packet::prelude::*;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// // If we don't set a reference time, then the current time is used
    /// // when the signature is created.
    /// let sig = SignatureBuilder::new(SignatureType::PositiveCertification);
    /// let ct = sig.effective_signature_creation_time()?.expect("creation time");
    /// assert!(SystemTime::now().duration_since(ct).expect("ct is in the past")
    ///         < Duration::new(1, 0));
    ///
    /// // If we set a reference time and don't set a creation time,
    /// // then that time is used for the creation time.
    /// let t = std::time::UNIX_EPOCH + Duration::new(1646660000, 0);
    /// let sig = sig.set_reference_time(t);
    /// assert_eq!(sig.effective_signature_creation_time()?, Some(t));
    /// # Ok(()) }
    /// ```
    pub fn set_reference_time<T>(mut self, reference_time: T) -> Self
    where
        T: Into<Option<SystemTime>>,
    {
        self.reference_time = reference_time.into();
        self
    }

    /// Returns the signature creation time that would be used if a
    /// signature were created now.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::{Duration, SystemTime};
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::SignatureType;
    /// use openpgp::packet::prelude::*;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// // If we don't set a creation time, then the current time is used.
    /// let sig = SignatureBuilder::new(SignatureType::PositiveCertification);
    /// let ct = sig.effective_signature_creation_time()?.expect("creation time");
    /// assert!(SystemTime::now().duration_since(ct).expect("ct is in the past")
    ///         < Duration::new(1, 0));
    ///
    /// // If we set a signature creation time, then we should get it back.
    /// let t = SystemTime::now() - Duration::new(24 * 60 * 60, 0);
    /// let sig = sig.set_signature_creation_time(t)?;
    /// assert!(t.duration_since(
    ///             sig.effective_signature_creation_time()?.unwrap()).unwrap()
    ///         < Duration::new(1, 0));
    /// # Ok(()) }
    /// ```
    pub fn effective_signature_creation_time(&self)
        -> Result<Option<SystemTime>>
    {
        use std::time;

        let now = || self.reference_time.unwrap_or_else(crate::now);

        if ! self.overrode_creation_time {
            // See if we want to backdate the signature.
            if let Some(orig) = self.original_creation_time {
                let now = now();
                let t =
                    (orig + time::Duration::new(1, 0)).max(
                        now - time::Duration::new(SIG_BACKDATE_BY, 0));

                if t > now {
                    return Err(Error::InvalidOperation(
                        "Cannot create valid signature newer than SignatureBuilder template"
                            .into()).into());
                }

                Ok(Some(t))
            } else {
                Ok(Some(now()))
            }
        } else {
            Ok(self.signature_creation_time())
        }
    }

    /// Adjusts signature prior to signing.
    ///
    /// This function is called implicitly when a signature is created
    /// (e.g. using [`SignatureBuilder::sign_message`]).  Usually,
    /// there is no need to call it explicitly.
    ///
    /// This function makes sure that generated signatures have a
    /// creation time, issuer information, and are not predictable by
    /// including a salt.  Then, it sorts the subpackets.  The
    /// function is idempotent modulo salt value.
    ///
    /// # Examples
    ///
    /// Occasionally, it is useful to determine the available space in
    /// a subpacket area.  To take the effect of this function into
    /// account, call this function explicitly:
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # fn main() -> openpgp::Result<()> {
    /// # use openpgp::packet::prelude::*;
    /// # use openpgp::types::Curve;
    /// # use openpgp::packet::signature::subpacket::SubpacketArea;
    /// # use openpgp::types::SignatureType;
    /// #
    /// # let key: Key<key::SecretParts, key::PrimaryRole>
    /// #     = Key::from(Key4::generate_ecc(true, Curve::Ed25519)?);
    /// # let mut signer = key.into_keypair()?;
    /// let sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .pre_sign(&mut signer)?; // Important for size calculation.
    ///
    /// // Compute the available space in the hashed area.  For this,
    /// // it is important that template.pre_sign has been called.
    /// use openpgp::serialize::MarshalInto;
    /// let available_space =
    ///     SubpacketArea::MAX_SIZE - sig.hashed_area().serialized_len();
    ///
    /// // Let's check whether our prediction was right.
    /// let sig = sig.sign_message(&mut signer, b"Hello World :)")?;
    /// assert_eq!(
    ///     available_space,
    ///     SubpacketArea::MAX_SIZE - sig.hashed_area().serialized_len());
    /// # Ok(()) }
    /// ```
    pub fn pre_sign(mut self, signer: &dyn Signer) -> Result<Self> {
        self.pk_algo = signer.public().pk_algo();

        // Set the creation time.
        if ! self.overrode_creation_time {
            if let Some(t) = self.effective_signature_creation_time()? {
                self = self.set_signature_creation_time(t)?;
            }
        }

        // Make sure we have an issuer packet.
        if self.issuers().next().is_none()
            && self.issuer_fingerprints().next().is_none()
        {
            self = self.set_issuer(signer.public().keyid())?
                .set_issuer_fingerprint(signer.public().fingerprint())?;
        }

        // Add a salt to make the signature unpredictable.
        let mut salt = [0; 32];
        crate::crypto::random(&mut salt);
        self = self.set_notation("salt@notations.sequoia-pgp.org",
                                 salt, None, false)?;

        self.sort();

        Ok(self)
    }

    fn sign(self, signer: &mut dyn Signer, digest: Vec<u8>)
        -> Result<Signature>
    {
        let mpis = signer.sign(self.hash_algo, &digest)?;

        Ok(Signature4 {
            common: Default::default(),
            fields: self.fields,
            digest_prefix: [digest[0], digest[1]],
            mpis,
            computed_digest: Some(digest),
            level: 0,
            additional_issuers: Vec::with_capacity(0),
        }.into())
    }
}

impl From<Signature> for SignatureBuilder {
    fn from(sig: Signature) -> Self {
        match sig {
            Signature::V3(sig) => sig.into(),
            Signature::V4(sig) => sig.into(),
        }
    }
}

impl From<Signature4> for SignatureBuilder {
    fn from(sig: Signature4) -> Self {
        let mut fields = sig.fields;

        fields.hash_algo = HashAlgorithm::default();

        let creation_time = fields.signature_creation_time();

        fields.hashed_area_mut().remove_all(SubpacketTag::SignatureCreationTime);
        fields.hashed_area_mut().remove_all(SubpacketTag::Issuer);
        fields.hashed_area_mut().remove_all(SubpacketTag::IssuerFingerprint);

        fields.unhashed_area_mut().remove_all(SubpacketTag::SignatureCreationTime);
        fields.unhashed_area_mut().remove_all(SubpacketTag::Issuer);
        fields.unhashed_area_mut().remove_all(SubpacketTag::IssuerFingerprint);

        SignatureBuilder {
            reference_time: None,
            overrode_creation_time: false,
            original_creation_time: creation_time,
            fields,
        }
    }
}

/// Holds a v4 Signature packet.
///
/// This holds a [version 4] Signature packet.  Normally, you won't
/// directly work with this data structure, but with the [`Signature`]
/// enum, which is version agnostic.  An exception is when you need to
/// do version-specific operations.  But currently, there aren't any
/// version-specific methods.
///
///   [version 4]: https://tools.ietf.org/html/rfc4880#section-5.2
///   [`Signature`]: super::Signature
#[derive(Clone)]
pub struct Signature4 {
    /// CTB packet header fields.
    pub(crate) common: packet::Common,

    /// Fields as configured using the SignatureBuilder.
    pub(crate) fields: SignatureFields,

    /// Upper 16 bits of the signed hash value.
    digest_prefix: [u8; 2],
    /// Signature MPIs.
    mpis: mpi::Signature,

    /// When used in conjunction with a one-pass signature, this is the
    /// hash computed over the enclosed message.
    computed_digest: Option<Vec<u8>>,

    /// Signature level.
    ///
    /// A level of 0 indicates that the signature is directly over the
    /// data, a level of 1 means that the signature is a notarization
    /// over all level 0 signatures and the data, and so on.
    level: usize,

    /// Additional issuer information.
    ///
    /// When we verify a signature successfully, we know the key that
    /// made the signature.  Hence, we can compute the fingerprint,
    /// either a V4 one or a later one.  If this information is
    /// missing from the signature, we can add it to the unhashed
    /// subpacket area at a convenient time.  We don't add it when
    /// verifying, because that would mean that verifying a signature
    /// would change the serialized representation, and signature
    /// verification is usually expected to be idempotent.
    additional_issuers: Vec<KeyHandle>,
}
assert_send_and_sync!(Signature4);

impl fmt::Debug for Signature4 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Signature4")
            .field("version", &self.version())
            .field("typ", &self.typ())
            .field("pk_algo", &self.pk_algo())
            .field("hash_algo", &self.hash_algo())
            .field("hashed_area", self.hashed_area())
            .field("unhashed_area", self.unhashed_area())
            .field("additional_issuers", &self.additional_issuers)
            .field("digest_prefix",
                   &crate::fmt::to_hex(&self.digest_prefix, false))
            .field(
                "computed_digest",
                &self
                    .computed_digest
                    .as_ref()
                    .map(|hash| crate::fmt::to_hex(&hash[..], false)),
            )
            .field("level", &self.level)
            .field("mpis", &self.mpis)
            .finish()
    }
}

impl PartialEq for Signature4 {
    /// This method tests for self and other values to be equal, and
    /// is used by ==.
    ///
    /// This method compares the serialized version of the two
    /// packets.  Thus, the computed values are ignored ([`level`],
    /// [`computed_digest`]).
    ///
    /// Note: because this function also compares the unhashed
    /// subpacket area, it is possible for a malicious party to take
    /// valid signatures, add subpackets to the unhashed area,
    /// yielding valid but distinct signatures.  If you want to ignore
    /// the unhashed area, you should instead use the
    /// [`Signature::normalized_eq`] method.
    ///
    /// [`level`]: Signature4::level()
    /// [`computed_digest`]: Signature4::computed_digest()
    fn eq(&self, other: &Signature4) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Signature4 {}

impl PartialOrd for Signature4 {
    fn partial_cmp(&self, other: &Signature4) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Signature4 {
    fn cmp(&self, other: &Signature4) -> Ordering {
        self.fields.cmp(&other.fields)
            .then_with(|| self.digest_prefix.cmp(&other.digest_prefix))
            .then_with(|| self.mpis.cmp(&other.mpis))
    }
}

impl std::hash::Hash for Signature4 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        use std::hash::Hash as StdHash;
        StdHash::hash(&self.mpis, state);
        StdHash::hash(&self.fields, state);
        self.digest_prefix.hash(state);
    }
}

impl Signature4 {
    /// Creates a new signature packet.
    ///
    /// If you want to sign something, consider using the [`SignatureBuilder`]
    /// interface.
    pub fn new(typ: SignatureType, pk_algo: PublicKeyAlgorithm,
               hash_algo: HashAlgorithm, hashed_area: SubpacketArea,
               unhashed_area: SubpacketArea,
               digest_prefix: [u8; 2],
               mpis: mpi::Signature) -> Self {
        Signature4 {
            common: Default::default(),
            fields: SignatureFields {
                version: 4,
                typ,
                pk_algo,
                hash_algo,
                subpackets: SubpacketAreas::new(hashed_area, unhashed_area),
            },
            digest_prefix,
            mpis,
            computed_digest: None,
            level: 0,
            additional_issuers: Vec::with_capacity(0),
        }
    }

    /// Gets the public key algorithm.
    // SigantureFields::pk_algo is private, because we don't want it
    // available on SignatureBuilder, which also derefs to
    // &SignatureFields.
    pub fn pk_algo(&self) -> PublicKeyAlgorithm {
        self.fields.pk_algo()
    }

    /// Gets the hash prefix.
    pub fn digest_prefix(&self) -> &[u8; 2] {
        &self.digest_prefix
    }

    /// Sets the hash prefix.
    #[allow(dead_code)]
    pub(crate) fn set_digest_prefix(&mut self, prefix: [u8; 2]) -> [u8; 2] {
        ::std::mem::replace(&mut self.digest_prefix, prefix)
    }

    /// Gets the signature packet's MPIs.
    pub fn mpis(&self) -> &mpi::Signature {
        &self.mpis
    }

    /// Sets the signature packet's MPIs.
    #[allow(dead_code)]
    pub(crate) fn set_mpis(&mut self, mpis: mpi::Signature) -> mpi::Signature
    {
        ::std::mem::replace(&mut self.mpis, mpis)
    }

    /// Gets the computed hash value.
    ///
    /// This is set by the [`PacketParser`] when parsing the message.
    ///
    /// [`PacketParser`]: crate::parse::PacketParser
    pub fn computed_digest(&self) -> Option<&[u8]> {
        self.computed_digest.as_ref().map(|d| &d[..])
    }

    /// Sets the computed hash value.
    pub(crate) fn set_computed_digest(&mut self, hash: Option<Vec<u8>>)
        -> Option<Vec<u8>>
    {
        ::std::mem::replace(&mut self.computed_digest, hash)
    }

    /// Gets the signature level.
    ///
    /// A level of 0 indicates that the signature is directly over the
    /// data, a level of 1 means that the signature is a notarization
    /// over all level 0 signatures and the data, and so on.
    pub fn level(&self) -> usize {
        self.level
    }

    /// Sets the signature level.
    ///
    /// A level of 0 indicates that the signature is directly over the
    /// data, a level of 1 means that the signature is a notarization
    /// over all level 0 signatures and the data, and so on.
    pub(crate) fn set_level(&mut self, level: usize) -> usize {
        ::std::mem::replace(&mut self.level, level)
    }

    /// Returns whether or not this signature should be exported.
    ///
    /// This checks whether the [`Exportable Certification`] subpacket
    /// is absent or present and 1, and that the signature does not
    /// include any sensitive [`Revocation Key`] (designated revokers)
    /// subpackets.
    ///
    ///   [`Exportable Certification`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.11
    ///   [`Revocation Key`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.15
    pub fn exportable(&self) -> Result<()> {
        if ! self.exportable_certification().unwrap_or(true) {
            return Err(Error::InvalidOperation(
                "Cannot export non-exportable certification".into()).into());
        }

        if self.revocation_keys().any(|r| r.sensitive()) {
            return Err(Error::InvalidOperation(
                "Cannot export signature with sensitive designated revoker"
                    .into()).into());
        }

        Ok(())
    }
}

impl From<Signature3> for SignatureBuilder {
    fn from(sig: Signature3) -> Self {
        SignatureBuilder::from(sig.intern)
    }
}

/// Holds a v3 Signature packet.
///
/// This holds a [version 3] Signature packet.  Normally, you won't
/// directly work with this data structure, but with the [`Signature`]
/// enum, which is version agnostic.  An exception is when you need to
/// do version-specific operations.  But currently, there aren't any
/// version-specific methods.
///
///   [version 3]: https://tools.ietf.org/html/rfc4880#section-5.2
///   [`Signature`]: super::Signature
///
/// Note: Per RFC 4880, v3 signatures should not be generated, but
/// they should be accepted.  As such, support for version 3
/// signatures is limited to verifying them, but not generating them.
#[derive(Clone)]
pub struct Signature3 {
    pub(crate) intern: Signature4,
}
assert_send_and_sync!(Signature3);

impl TryFrom<Signature> for Signature3 {
    type Error = anyhow::Error;

    fn try_from(sig: Signature) -> Result<Self> {
        match sig {
            Signature::V3(sig) => Ok(sig),
            sig => Err(
                Error::InvalidArgument(
                    format!(
                        "Got a v{}, require a v3 signature",
                        sig.version()))
                    .into()),
        }
    }
}

// Yes, Signature3 derefs to Signature4.  This is because Signature
// derefs to Signature4 so this is the only way to add support for v3
// sigs without breaking the semver.
impl Deref for Signature3 {
    type Target = Signature4;

    fn deref(&self) -> &Self::Target {
        &self.intern
    }
}

impl DerefMut for Signature3 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.intern
    }
}

impl fmt::Debug for Signature3 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Signature3")
            .field("version", &self.version())
            .field("typ", &self.typ())
            .field("pk_algo", &self.pk_algo())
            .field("hash_algo", &self.hash_algo())
            .field("hashed_area", self.hashed_area())
            .field("unhashed_area", self.unhashed_area())
            .field("additional_issuers", &self.additional_issuers)
            .field("digest_prefix",
                   &crate::fmt::to_hex(&self.digest_prefix, false))
            .field(
                "computed_digest",
                &self
                    .computed_digest
                    .as_ref()
                    .map(|hash| crate::fmt::to_hex(&hash[..], false)),
            )
            .field("level", &self.level)
            .field("mpis", &self.mpis)
            .finish()
    }
}

impl PartialEq for Signature3 {
    /// This method tests for self and other values to be equal, and
    /// is used by ==.
    ///
    /// This method compares the serialized version of the two
    /// packets.  Thus, the computed values are ignored ([`level`],
    /// [`computed_digest`]).
    ///
    /// [`level`]: Signature3::level()
    /// [`computed_digest`]: Signature3::computed_digest()
    fn eq(&self, other: &Signature3) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Signature3 {}

impl PartialOrd for Signature3 {
    fn partial_cmp(&self, other: &Signature3) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Signature3 {
    fn cmp(&self, other: &Signature3) -> Ordering {
        self.intern.cmp(&other.intern)
    }
}

impl std::hash::Hash for Signature3 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        use std::hash::Hash as StdHash;
        StdHash::hash(&self.intern, state);
    }
}

impl Signature3 {
    /// Creates a new signature packet.
    ///
    /// If you want to sign something, consider using the [`SignatureBuilder`]
    /// interface.
    ///
    pub fn new(typ: SignatureType, creation_time: Timestamp,
               issuer: KeyID,
               pk_algo: PublicKeyAlgorithm,
               hash_algo: HashAlgorithm,
               digest_prefix: [u8; 2],
               mpis: mpi::Signature) -> Self {
        let hashed_area = SubpacketArea::new(vec![
            Subpacket::new(
                SubpacketValue::SignatureCreationTime(creation_time),
                true).expect("fits"),
        ]).expect("fits");
        let unhashed_area = SubpacketArea::new(vec![
            Subpacket::new(
                SubpacketValue::Issuer(issuer),
                false).expect("fits"),
        ]).expect("fits");

        let mut sig = Signature4::new(typ,
                                      pk_algo, hash_algo,
                                      hashed_area, unhashed_area,
                                      digest_prefix, mpis);
        sig.version = 3;

        Signature3 {
            intern: sig,
        }
    }

    /// Gets the public key algorithm.
    // SigantureFields::pk_algo is private, because we don't want it
    // available on SignatureBuilder, which also derefs to
    // &SignatureFields.
    pub fn pk_algo(&self) -> PublicKeyAlgorithm {
        self.fields.pk_algo()
    }

    /// Gets the hash prefix.
    pub fn digest_prefix(&self) -> &[u8; 2] {
        &self.digest_prefix
    }

    /// Gets the signature packet's MPIs.
    pub fn mpis(&self) -> &mpi::Signature {
        &self.mpis
    }

    /// Gets the computed hash value.
    ///
    /// This is set by the [`PacketParser`] when parsing the message.
    ///
    /// [`PacketParser`]: crate::parse::PacketParser
    pub fn computed_digest(&self) -> Option<&[u8]> {
        self.computed_digest.as_ref().map(|d| &d[..])
    }

    /// Gets the signature level.
    ///
    /// A level of 0 indicates that the signature is directly over the
    /// data, a level of 1 means that the signature is a notarization
    /// over all level 0 signatures and the data, and so on.
    pub fn level(&self) -> usize {
        self.level
    }
}

impl crate::packet::Signature {
    /// Returns the value of any Issuer and Issuer Fingerprint subpackets.
    ///
    /// The [Issuer subpacket] and [Issuer Fingerprint subpacket] are
    /// used when processing a signature to identify which certificate
    /// created the signature.  Since this information is
    /// self-authenticating (the act of validating the signature
    /// authenticates the subpacket), it is typically stored in the
    /// unhashed subpacket area.
    ///
    ///   [Issuer subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///   [Issuer Fingerprint subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///
    /// This function returns all instances of the Issuer subpacket
    /// and the Issuer Fingerprint subpacket in both the hashed
    /// subpacket area and the unhashed subpacket area.
    ///
    /// The issuers are sorted so that the `Fingerprints` come before
    /// `KeyID`s.  The `Fingerprint`s and `KeyID`s are not further
    /// sorted, but are returned in the order that they are
    /// encountered.
    pub fn get_issuers(&self) -> Vec<crate::KeyHandle> {
        let mut issuers: Vec<_> =
            self.hashed_area().iter()
            .chain(self.unhashed_area().iter())
            .filter_map(|subpacket| {
                match subpacket.value() {
                    SubpacketValue::Issuer(i) => Some(i.into()),
                    SubpacketValue::IssuerFingerprint(i) => Some(i.into()),
                    _ => None,
                }
            })
            .collect();

        // Sort the issuers so that the fingerprints come first.
        issuers.sort_by(|a, b| {
            use crate::KeyHandle::*;
            use std::cmp::Ordering::*;
            match (a, b) {
                (Fingerprint(_), Fingerprint(_)) => Equal,
                (KeyID(_), Fingerprint(_)) => Greater,
                (Fingerprint(_), KeyID(_)) => Less,
                (KeyID(_), KeyID(_)) => Equal,
            }
        });

        issuers
    }

    /// Compares Signatures ignoring the unhashed subpacket area.
    ///
    /// This comparison function ignores the unhashed subpacket area
    /// when comparing two signatures.  This prevents a malicious
    /// party from taking valid signatures, adding subpackets to the
    /// unhashed area, and deriving valid but distinct signatures,
    /// which could be used to perform a denial of service attack.
    /// For instance, an attacker could create a lot of signatures,
    /// which need to be validated.  Ignoring the unhashed subpackets
    /// means that we can deduplicate signatures using this predicate.
    ///
    /// Unlike [`Signature::normalize`], this method ignores
    /// authenticated packets in the unhashed subpacket area.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::packet::signature::subpacket::{Subpacket, SubpacketValue};
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    /// use openpgp::types::Features;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().generate()?;
    ///
    /// let orig = cert.with_policy(p, None)?.direct_key_signature()?;
    ///
    /// // Add an inconspicuous subpacket to the unhashed area.
    /// let sb = Subpacket::new(SubpacketValue::Features(Features::empty()), false)?;
    /// let mut modified = orig.clone();
    /// modified.unhashed_area_mut().add(sb);
    ///
    /// // We modified the signature, but the signature is still valid.
    /// modified.verify_direct_key(cert.primary_key().key(), cert.primary_key().key());
    ///
    /// // PartialEq considers the packets to not be equal...
    /// assert!(orig != &modified);
    /// // ... but normalized_eq does.
    /// assert!(orig.normalized_eq(&modified));
    /// # Ok(())
    /// # }
    /// ```
    pub fn normalized_eq(&self, other: &Signature) -> bool {
        self.normalized_cmp(other) == Ordering::Equal
    }

    /// Compares Signatures ignoring the unhashed subpacket area.
    ///
    /// This is useful to deduplicate signatures by first sorting them
    /// using this function, and then deduplicating using the
    /// [`Signature::normalized_eq`] predicate.
    ///
    /// This comparison function ignores the unhashed subpacket area
    /// when comparing two signatures.  This prevents a malicious
    /// party from taking valid signatures, adding subpackets to the
    /// unhashed area, and deriving valid but distinct signatures,
    /// which could be used to perform a denial of service attack.
    /// For instance, an attacker could create a lot of signatures,
    /// which need to be validated.  Ignoring the unhashed subpackets
    /// means that we can deduplicate signatures using this predicate.
    ///
    /// Unlike [`Signature::normalize`], this method ignores
    /// authenticated packets in the unhashed subpacket area.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cmp::Ordering;
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::packet::signature::subpacket::{Subpacket, SubpacketValue};
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    /// use openpgp::types::Features;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().generate()?;
    ///
    /// let orig = cert.with_policy(p, None)?.direct_key_signature()?;
    ///
    /// // Add an inconspicuous subpacket to the unhashed area.
    /// let sb = Subpacket::new(SubpacketValue::Features(Features::empty()), false)?;
    /// let mut modified = orig.clone();
    /// modified.unhashed_area_mut().add(sb);
    ///
    /// // We modified the signature, but the signature is still valid.
    /// modified.verify_direct_key(cert.primary_key().key(), cert.primary_key().key());
    ///
    /// // PartialEq considers the packets to not be equal...
    /// assert!(orig != &modified);
    /// // ... but normalized_partial_cmp does.
    /// assert!(orig.normalized_cmp(&modified) == Ordering::Equal);
    /// # Ok(()) }
    /// ```
    pub fn normalized_cmp(&self, other: &Signature)
                          -> Ordering {
        self.version().cmp(&other.version())
            .then_with(|| self.typ().cmp(&other.typ()))
            .then_with(|| self.pk_algo().cmp(&other.pk_algo()))
            .then_with(|| self.hash_algo().cmp(&other.hash_algo()))
            .then_with(|| self.hashed_area().cmp(other.hashed_area()))
            .then_with(|| self.digest_prefix().cmp(other.digest_prefix()))
            .then_with(|| self.mpis().cmp(other.mpis()))
    }

    /// Hashes everything but the unhashed subpacket area into state.
    ///
    /// This is an alternate implementation of [`Hash`], which does
    /// not hash the unhashed subpacket area.
    ///
    ///   [`Hash`]: std::hash::Hash
    ///
    /// Unlike [`Signature::normalize`], this method ignores
    /// authenticated packets in the unhashed subpacket area.
    pub fn normalized_hash<H>(&self, state: &mut H)
        where H: Hasher
    {
        use std::hash::Hash;

        self.version.hash(state);
        self.typ.hash(state);
        self.pk_algo.hash(state);
        self.hash_algo.hash(state);
        self.hashed_area().hash(state);
        self.digest_prefix().hash(state);
        Hash::hash(&self.mpis(), state);
    }

    /// Normalizes the signature.
    ///
    /// This function normalizes the *unhashed* signature subpackets.
    ///
    /// First, it removes all but the following self-authenticating
    /// subpackets:
    ///
    ///   - `SubpacketValue::Issuer`
    ///   - `SubpacketValue::IssuerFingerprint`
    ///   - `SubpacketValue::EmbeddedSignature`
    ///
    /// Note: the retained subpackets are not checked for validity.
    ///
    /// Then, it adds any missing issuer information to the unhashed
    /// subpacket area that has been computed when verifying the
    /// signature.
    pub fn normalize(&self) -> Self {
        use subpacket::SubpacketTag::*;
        let mut sig = self.clone();
        {
            let area = sig.unhashed_area_mut();
            area.clear();

            for spkt in self.unhashed_area().iter()
                .filter(|s| s.tag() == Issuer
                        || s.tag() == IssuerFingerprint
                        || s.tag() == EmbeddedSignature)
            {
                area.add(spkt.clone())
                    .expect("it did fit into the old area");
            }

            // Add missing issuer information.  This is icing on the
            // cake, hence it is only a best-effort mechanism that
            // silently fails.
            let _ = sig.add_missing_issuers();

            // Normalize the order of subpackets.
            sig.unhashed_area_mut().sort();
        }
        sig
    }

    /// Adds missing issuer information.
    ///
    /// Calling this function adds any missing issuer information to
    /// the unhashed subpacket area.
    ///
    /// When a signature is verified, the identity of the signing key
    /// is computed and stored in the `Signature` struct.  This
    /// information can be used to complement the issuer information
    /// stored in the signature.  Note that we don't do this
    /// automatically when verifying signatures, because that would
    /// change the serialized representation of the signature as a
    /// side-effect of verifying the signature.
    pub fn add_missing_issuers(&mut self) -> Result<()> {
        if self.additional_issuers.is_empty() {
            return Ok(());
        }

        /// Makes an authenticated subpacket.
        fn authenticated_subpacket(v: SubpacketValue) -> Result<Subpacket> {
            let mut p = Subpacket::new(v, false)?;
            p.set_authenticated(true);
            Ok(p)
        }

        let issuers = self.get_issuers();
        for id in std::mem::replace(&mut self.additional_issuers,
                                    Vec::with_capacity(0)) {
            if ! issuers.contains(&id) {
                match id {
                    KeyHandle::KeyID(id) =>
                        self.unhashed_area_mut().add(authenticated_subpacket(
                            SubpacketValue::Issuer(id))?)?,
                    KeyHandle::Fingerprint(fp) =>
                        self.unhashed_area_mut().add(authenticated_subpacket(
                            SubpacketValue::IssuerFingerprint(fp))?)?,
                }
            }
        }

        Ok(())
    }

    /// Merges two signatures.
    ///
    /// Two signatures that are equal according to
    /// [`Signature::normalized_eq`] may differ in the contents of the
    /// unhashed subpacket areas.  This function merges two signatures
    /// trying hard to incorporate all the information into one
    /// signature while avoiding denial of service attacks by merging
    /// in bad information.
    ///
    /// The merge strategy is as follows:
    ///
    ///   - If the signatures differ according to
    ///     [`Signature::normalized_eq`], the merge fails.
    ///
    ///   - Do not consider any subpacket that does not belong into
    ///     the unhashed subpacket area.
    ///
    ///   - Consider all remaining subpackets, in the following order.
    ///     If we run out of space, all remaining subpackets are
    ///     ignored.
    ///
    ///     - Authenticated subpackets from `self`
    ///     - Authenticated subpackets from `other`
    ///     - Unauthenticated subpackets from `self` commonly found in
    ///       unhashed areas
    ///     - Unauthenticated subpackets from `other` commonly found in
    ///       unhashed areas
    ///     - Remaining subpackets from `self`
    ///     - Remaining subpackets from `other`
    ///
    ///     See [`Subpacket::authenticated`] for how subpackets are
    ///     authenticated.  Subpackets commonly found in unhashed
    ///     areas are issuer information and embedded signatures.
    pub fn merge(mut self, other: Signature) -> Result<Signature> {
        self.merge_internal(&other)?;
        Ok(self)
    }

    /// Same as Signature::merge, but non-consuming for use with
    /// Vec::dedup_by.
    pub(crate) fn merge_internal(&mut self, other: &Signature) -> Result<()>
    {
        use crate::serialize::MarshalInto;

        if ! self.normalized_eq(other) {
            return Err(Error::InvalidArgument(
                "Signatures are not equal modulo unhashed subpackets".into())
                       .into());
        }

        // Filters subpackets that plausibly could be in the unhashed
        // area.
        fn eligible(p: &Subpacket) -> bool {
            use SubpacketTag::*;
            match p.tag() {
                SignatureCreationTime
                    | SignatureExpirationTime
                    | ExportableCertification
                    | TrustSignature
                    | RegularExpression
                    | Revocable
                    | KeyExpirationTime
                    | PlaceholderForBackwardCompatibility
                    | PreferredSymmetricAlgorithms
                    | RevocationKey
                    | PreferredHashAlgorithms
                    | PreferredCompressionAlgorithms
                    | KeyServerPreferences
                    | PreferredKeyServer
                    | PrimaryUserID
                    | PolicyURI
                    | KeyFlags
                    | SignersUserID
                    | ReasonForRevocation
                    | Features
                    | SignatureTarget
                    | PreferredAEADAlgorithms
                    | IntendedRecipient
                    | AttestedCertifications
                    | Reserved(_)
                    => false,
                Issuer
                    | NotationData
                    | EmbeddedSignature
                    | IssuerFingerprint
                    | Private(_)
                    | Unknown(_)
                    => true,
            }
        }

        // Filters subpackets that usually are in the unhashed area.
        fn prefer(p: &Subpacket) -> bool {
            use SubpacketTag::*;
            matches!(p.tag(), Issuer | EmbeddedSignature | IssuerFingerprint)
        }

        // Collect subpackets keeping track of the size.
        #[allow(clippy::mutable_key_type)]
        // In general, the keys of a HashSet should not have interior mutability.
        // This particular use should be safe:  The hash set is only constructed
        // for the merge, we own all objects we put into the set, and we don't
        // modify them while they are in the set.
        let mut acc = std::collections::HashSet::new();
        let mut size = 0;

        // Start with missing issuer information.
        for id in std::mem::replace(&mut self.additional_issuers,
                                    Vec::with_capacity(0)).into_iter()
            .chain(other.additional_issuers.iter().cloned())
        {
            let p = match id {
                KeyHandle::KeyID(id) => Subpacket::new(
                    SubpacketValue::Issuer(id), false)?,
                KeyHandle::Fingerprint(fp) => Subpacket::new(
                    SubpacketValue::IssuerFingerprint(fp), false)?,
            };

            let l = p.serialized_len();
            if size + l <= std::u16::MAX as usize && acc.insert(p.clone()) {
                size += l;
            }
        }

        // Make multiple passes over the subpacket areas.  Always
        // start with self, then other.  Only consider eligible
        // packets.  Consider authenticated ones first, then plausible
        // unauthenticated ones, then the rest.  If inserting fails at
        // any moment, stop.
        for p in
            self.unhashed_area().iter()
                   .filter(|p| eligible(p) && p.authenticated())
            .chain(other.unhashed_area().iter()
                   .filter(|p| eligible(p) && p.authenticated()))
            .chain(self.unhashed_area().iter()
                   .filter(|p| eligible(p) && ! p.authenticated() && prefer(p)))
            .chain(other.unhashed_area().iter()
                   .filter(|p| eligible(p) && ! p.authenticated() && prefer(p)))
            .chain(self.unhashed_area().iter()
                   .filter(|p| eligible(p) && ! p.authenticated() && ! prefer(p)))
            .chain(other.unhashed_area().iter()
                   .filter(|p| eligible(p) && ! p.authenticated() && ! prefer(p)))
        {
            let l = p.serialized_len();
            if size + l <= std::u16::MAX as usize && acc.insert(p.clone()) {
                size += l;
            }
        }
        assert!(size <= std::u16::MAX as usize);
        let mut a = SubpacketArea::new(acc.into_iter().collect())
            .expect("must fit");
        a.sort();
        *self.unhashed_area_mut() = a;

        Ok(())
    }
}

/// Verification-related functionality.
///
/// <a id="verification-functions"></a>
impl Signature {
    /// Verifies the signature against `hash`.
    ///
    /// The `hash` should only be computed over the payload, this
    /// function hashes in the signature itself before verifying it.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature and checks that the key predates the
    /// signature.  Further constraints on the signature, like
    /// creation and expiration time, or signature revocations must be
    /// checked by the caller.
    ///
    /// Likewise, this function does not check whether `key` can made
    /// valid signatures; it is up to the caller to make sure the key
    /// is not revoked, not expired, has a valid self-signature, has a
    /// subkey binding signature (if appropriate), has the signing
    /// capability, etc.
    pub fn verify_hash<P, R>(&mut self, key: &Key<P, R>,
                             mut hash: Box<dyn hash::Digest>)
        -> Result<()>
        where P: key::KeyParts,
              R: key::KeyRole,
    {
        self.hash(&mut hash);
        let mut digest = vec![0u8; hash.digest_size()];
        hash.digest(&mut digest)?;
        self.verify_digest(key, digest)
    }

    /// Verifies the signature against `digest`.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature and checks that the key predates the
    /// signature.  Further constraints on the signature, like
    /// creation and expiration time, or signature revocations must be
    /// checked by the caller.
    ///
    /// Likewise, this function does not check whether `key` can made
    /// valid signatures; it is up to the caller to make sure the key
    /// is not revoked, not expired, has a valid self-signature, has a
    /// subkey binding signature (if appropriate), has the signing
    /// capability, etc.
    pub fn verify_digest<P, R, D>(&mut self, key: &Key<P, R>, digest: D)
        -> Result<()>
        where P: key::KeyParts,
              R: key::KeyRole,
              D: AsRef<[u8]>,
    {
        if let Some(creation_time) = self.signature_creation_time() {
            if creation_time < key.creation_time() {
                return Err(Error::BadSignature(
                    format!("Signature (created {:?}) predates key ({:?})",
                            creation_time, key.creation_time())).into());
            }
        } else {
            return Err(Error::BadSignature(
                "Signature has no creation time subpacket".into()).into());
        }

        let result = key.verify(self.mpis(), self.hash_algo(), digest.as_ref());
        if result.is_ok() {
            // Mark information in this signature as authenticated.

            // The hashed subpackets are authenticated by the
            // signature.
            self.hashed_area_mut().iter_mut().for_each(|p| {
                p.set_authenticated(true);
            });

            // The self-authenticating unhashed subpackets are
            // authenticated by the key's identity.
            self.unhashed_area_mut().iter_mut().for_each(|p| {
                let authenticated = match p.value() {
                    SubpacketValue::Issuer(id) =>
                        id == &key.keyid(),
                    SubpacketValue::IssuerFingerprint(fp) =>
                        fp == &key.fingerprint(),
                    _ => false,
                };
                p.set_authenticated(authenticated);
            });

            // Compute and record any issuer information not yet
            // contained in the signature.
            let issuers = self.get_issuers();
            let id = KeyHandle::from(key.keyid());
            if ! (issuers.contains(&id)
                  || self.additional_issuers.contains(&id)) {
                self.additional_issuers.push(id);
            }

            let fp = KeyHandle::from(key.fingerprint());
            if ! (issuers.contains(&fp)
                  || self.additional_issuers.contains(&fp)) {
                self.additional_issuers.push(fp);
            }
        }
        result
    }

    /// Verifies the signature over text or binary documents using
    /// `key`.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `key` can make
    /// valid signatures; it is up to the caller to make sure the key
    /// is not revoked, not expired, has a valid self-signature, has a
    /// subkey binding signature (if appropriate), has the signing
    /// capability, etc.
    pub fn verify<P, R>(&mut self, key: &Key<P, R>) -> Result<()>
        where P: key::KeyParts,
              R: key::KeyRole,
    {
        if !(self.typ() == SignatureType::Binary
             || self.typ() == SignatureType::Text) {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        if let Some(hash) = self.computed_digest.take() {
            let result = self.verify_digest(key, &hash);
            self.computed_digest = Some(hash);
            result
        } else {
            Err(Error::BadSignature("Hash not computed.".to_string()).into())
        }
    }

    /// Verifies the standalone signature using `key`.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `key` can make
    /// valid signatures; it is up to the caller to make sure the key
    /// is not revoked, not expired, has a valid self-signature, has a
    /// subkey binding signature (if appropriate), has the signing
    /// capability, etc.
    pub fn verify_standalone<P, R>(&mut self, key: &Key<P, R>) -> Result<()>
        where P: key::KeyParts,
              R: key::KeyRole,
    {
        if self.typ() != SignatureType::Standalone {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        // Standalone signatures are like binary-signatures over the
        // zero-sized string.
        let mut hash = self.hash_algo().context()?;
        self.hash_standalone(&mut hash);
        self.verify_digest(key, &hash.into_digest()?[..])
    }

    /// Verifies the timestamp signature using `key`.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `key` can make
    /// valid signatures; it is up to the caller to make sure the key
    /// is not revoked, not expired, has a valid self-signature, has a
    /// subkey binding signature (if appropriate), has the signing
    /// capability, etc.
    pub fn verify_timestamp<P, R>(&mut self, key: &Key<P, R>) -> Result<()>
        where P: key::KeyParts,
              R: key::KeyRole,
    {
        if self.typ() != SignatureType::Timestamp {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        // Timestamp signatures are like binary-signatures over the
        // zero-sized string.
        let mut hash = self.hash_algo().context()?;
        self.hash_timestamp(&mut hash);
        self.verify_digest(key, &hash.into_digest()?[..])
    }

    /// Verifies the direct key signature.
    ///
    /// `self` is the direct key signature, `signer` is the
    /// key that allegedly made the signature, and `pk` is the primary
    /// key.
    ///
    /// For a self-signature, `signer` and `pk` will be the same.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `signer` can
    /// made valid signatures; it is up to the caller to make sure the
    /// key is not revoked, not expired, has a valid self-signature,
    /// has a subkey binding signature (if appropriate), has the
    /// signing capability, etc.
    pub fn verify_direct_key<P, Q, R>(&mut self,
                                      signer: &Key<P, R>,
                                      pk: &Key<Q, key::PrimaryRole>)
        -> Result<()>
        where P: key::KeyParts,
              Q: key::KeyParts,
              R: key::KeyRole,
    {
        if self.typ() != SignatureType::DirectKey {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        let mut hash = self.hash_algo().context()?;
        self.hash_direct_key(&mut hash, pk);
        self.verify_digest(signer, &hash.into_digest()?[..])
    }

    /// Verifies the primary key revocation certificate.
    ///
    /// `self` is the primary key revocation certificate, `signer` is
    /// the key that allegedly made the signature, and `pk` is the
    /// primary key,
    ///
    /// For a self-signature, `signer` and `pk` will be the same.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `signer` can
    /// made valid signatures; it is up to the caller to make sure the
    /// key is not revoked, not expired, has a valid self-signature,
    /// has a subkey binding signature (if appropriate), has the
    /// signing capability, etc.
    pub fn verify_primary_key_revocation<P, Q, R>(&mut self,
                                                  signer: &Key<P, R>,
                                                  pk: &Key<Q, key::PrimaryRole>)
        -> Result<()>
        where P: key::KeyParts,
              Q: key::KeyParts,
              R: key::KeyRole,
    {
        if self.typ() != SignatureType::KeyRevocation {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        let mut hash = self.hash_algo().context()?;
        self.hash_direct_key(&mut hash, pk);
        self.verify_digest(signer, &hash.into_digest()?[..])
    }

    /// Verifies the subkey binding.
    ///
    /// `self` is the subkey key binding signature, `signer` is the
    /// key that allegedly made the signature, `pk` is the primary
    /// key, and `subkey` is the subkey.
    ///
    /// For a self-signature, `signer` and `pk` will be the same.
    ///
    /// If the signature indicates that this is a `Signing` capable
    /// subkey, then the back signature is also verified.  If it is
    /// missing or can't be verified, then this function returns
    /// false.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `signer` can
    /// made valid signatures; it is up to the caller to make sure the
    /// key is not revoked, not expired, has a valid self-signature,
    /// has a subkey binding signature (if appropriate), has the
    /// signing capability, etc.
    pub fn verify_subkey_binding<P, Q, R, S>(
        &mut self,
        signer: &Key<P, R>,
        pk: &Key<Q, key::PrimaryRole>,
        subkey: &Key<S, key::SubordinateRole>)
        -> Result<()>
        where P: key::KeyParts,
              Q: key::KeyParts,
              R: key::KeyRole,
              S: key::KeyParts,
    {
        if self.typ() != SignatureType::SubkeyBinding {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        let mut hash = self.hash_algo().context()?;
        self.hash_subkey_binding(&mut hash, pk, subkey);
        self.verify_digest(signer, &hash.into_digest()?[..])?;

        // The signature is good, but we may still need to verify the
        // back sig.
        if self.key_flags().map(|kf| kf.for_signing()).unwrap_or(false) {
            let mut last_result = Err(Error::BadSignature(
                "Primary key binding signature missing".into()).into());

            for backsig in self.subpackets_mut(SubpacketTag::EmbeddedSignature)
            {
                let result =
                    if let SubpacketValue::EmbeddedSignature(sig) =
                        backsig.value_mut()
                {
                    sig.verify_primary_key_binding(pk, subkey)
                } else {
                    unreachable!("subpackets_mut(EmbeddedSignature) returns \
                                  EmbeddedSignatures");
                };
                if result.is_ok() {
                    // Mark the subpacket as authenticated by the
                    // embedded signature.
                    backsig.set_authenticated(true);
                    return result;
                }
                last_result = result;
            }
            last_result
        } else {
            // No backsig required.
            Ok(())
        }
    }

    /// Verifies the primary key binding.
    ///
    /// `self` is the primary key binding signature, `pk` is the
    /// primary key, and `subkey` is the subkey.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `subkey` can
    /// made valid signatures; it is up to the caller to make sure the
    /// key is not revoked, not expired, has a valid self-signature,
    /// has a subkey binding signature (if appropriate), has the
    /// signing capability, etc.
    pub fn verify_primary_key_binding<P, Q>(
        &mut self,
        pk: &Key<P, key::PrimaryRole>,
        subkey: &Key<Q, key::SubordinateRole>)
        -> Result<()>
        where P: key::KeyParts,
              Q: key::KeyParts,
    {
        if self.typ() != SignatureType::PrimaryKeyBinding {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        let mut hash = self.hash_algo().context()?;
        self.hash_primary_key_binding(&mut hash, pk, subkey);
        self.verify_digest(subkey, &hash.into_digest()?[..])
    }

    /// Verifies the subkey revocation.
    ///
    /// `self` is the subkey key revocation certificate, `signer` is
    /// the key that allegedly made the signature, `pk` is the primary
    /// key, and `subkey` is the subkey.
    ///
    /// For a self-revocation, `signer` and `pk` will be the same.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `signer` can
    /// made valid signatures; it is up to the caller to make sure the
    /// key is not revoked, not expired, has a valid self-signature,
    /// has a subkey binding signature (if appropriate), has the
    /// signing capability, etc.
    pub fn verify_subkey_revocation<P, Q, R, S>(
        &mut self,
        signer: &Key<P, R>,
        pk: &Key<Q, key::PrimaryRole>,
        subkey: &Key<S, key::SubordinateRole>)
        -> Result<()>
        where P: key::KeyParts,
              Q: key::KeyParts,
              R: key::KeyRole,
              S: key::KeyParts,
    {
        if self.typ() != SignatureType::SubkeyRevocation {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        let mut hash = self.hash_algo().context()?;
        self.hash_subkey_binding(&mut hash, pk, subkey);
        self.verify_digest(signer, &hash.into_digest()?[..])
    }

    /// Verifies the user id binding.
    ///
    /// `self` is the user id binding signature, `signer` is the key
    /// that allegedly made the signature, `pk` is the primary key,
    /// and `userid` is the user id.
    ///
    /// For a self-signature, `signer` and `pk` will be the same.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `signer` can
    /// made valid signatures; it is up to the caller to make sure the
    /// key is not revoked, not expired, has a valid self-signature,
    /// has a subkey binding signature (if appropriate), has the
    /// signing capability, etc.
    pub fn verify_userid_binding<P, Q, R>(&mut self,
                                          signer: &Key<P, R>,
                                          pk: &Key<Q, key::PrimaryRole>,
                                          userid: &UserID)
        -> Result<()>
        where P: key::KeyParts,
              Q: key::KeyParts,
              R: key::KeyRole,
    {
        if !(self.typ() == SignatureType::GenericCertification
             || self.typ() == SignatureType::PersonaCertification
             || self.typ() == SignatureType::CasualCertification
             || self.typ() == SignatureType::PositiveCertification) {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        let mut hash = self.hash_algo().context()?;
        self.hash_userid_binding(&mut hash, pk, userid);
        self.verify_digest(signer, &hash.into_digest()?[..])
    }

    /// Verifies the user id revocation certificate.
    ///
    /// `self` is the revocation certificate, `signer` is the key
    /// that allegedly made the signature, `pk` is the primary key,
    /// and `userid` is the user id.
    ///
    /// For a self-signature, `signer` and `pk` will be the same.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `signer` can
    /// made valid signatures; it is up to the caller to make sure the
    /// key is not revoked, not expired, has a valid self-signature,
    /// has a subkey binding signature (if appropriate), has the
    /// signing capability, etc.
    pub fn verify_userid_revocation<P, Q, R>(&mut self,
                                             signer: &Key<P, R>,
                                             pk: &Key<Q, key::PrimaryRole>,
                                             userid: &UserID)
        -> Result<()>
        where P: key::KeyParts,
              Q: key::KeyParts,
              R: key::KeyRole,
    {
        if self.typ() != SignatureType::CertificationRevocation {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        let mut hash = self.hash_algo().context()?;
        self.hash_userid_binding(&mut hash, pk, userid);
        self.verify_digest(signer, &hash.into_digest()?[..])
    }

    /// Verifies an attested key signature on a user id.
    ///
    /// This feature is [experimental](crate#experimental-features).
    ///
    /// Allows the certificate owner to attest to third party
    /// certifications. See [Section 5.2.3.30 of RFC 4880bis] for
    /// details.
    ///
    /// `self` is the attested key signature, `signer` is the key that
    /// allegedly made the signature, `pk` is the primary key, and
    /// `userid` is the user id.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `signer` can
    /// made valid signatures; it is up to the caller to make sure the
    /// key is not revoked, not expired, has a valid self-signature,
    /// has a subkey binding signature (if appropriate), has the
    /// signing capability, etc.
    ///
    ///   [Section 5.2.3.30 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-10.html#section-5.2.3.30
    pub fn verify_userid_attestation<P, Q, R>(
        &mut self,
        signer: &Key<P, R>,
        pk: &Key<Q, key::PrimaryRole>,
        userid: &UserID)
        -> Result<()>
        where P: key::KeyParts,
              Q: key::KeyParts,
              R: key::KeyRole,
    {
        if self.typ() != SignatureType::AttestationKey {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        let mut hash = self.hash_algo().context()?;

        if self.attested_certifications()?
            .any(|d| d.len() != hash.digest_size())
        {
            return Err(Error::BadSignature(
                "Wrong number of bytes in certification subpacket".into())
                       .into());
        }

        self.hash_userid_binding(&mut hash, pk, userid);
        self.verify_digest(signer, &hash.into_digest()?[..])
    }

    /// Verifies the user attribute binding.
    ///
    /// `self` is the user attribute binding signature, `signer` is
    /// the key that allegedly made the signature, `pk` is the primary
    /// key, and `ua` is the user attribute.
    ///
    /// For a self-signature, `signer` and `pk` will be the same.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `signer` can
    /// made valid signatures; it is up to the caller to make sure the
    /// key is not revoked, not expired, has a valid self-signature,
    /// has a subkey binding signature (if appropriate), has the
    /// signing capability, etc.
    pub fn verify_user_attribute_binding<P, Q, R>(&mut self,
                                                  signer: &Key<P, R>,
                                                  pk: &Key<Q, key::PrimaryRole>,
                                                  ua: &UserAttribute)
        -> Result<()>
        where P: key::KeyParts,
              Q: key::KeyParts,
              R: key::KeyRole,
    {
        if !(self.typ() == SignatureType::GenericCertification
             || self.typ() == SignatureType::PersonaCertification
             || self.typ() == SignatureType::CasualCertification
             || self.typ() == SignatureType::PositiveCertification) {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        let mut hash = self.hash_algo().context()?;
        self.hash_user_attribute_binding(&mut hash, pk, ua);
        self.verify_digest(signer, &hash.into_digest()?[..])
    }

    /// Verifies the user attribute revocation certificate.
    ///
    /// `self` is the user attribute binding signature, `signer` is
    /// the key that allegedly made the signature, `pk` is the primary
    /// key, and `ua` is the user attribute.
    ///
    /// For a self-signature, `signer` and `pk` will be the same.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `signer` can
    /// made valid signatures; it is up to the caller to make sure the
    /// key is not revoked, not expired, has a valid self-signature,
    /// has a subkey binding signature (if appropriate), has the
    /// signing capability, etc.
    pub fn verify_user_attribute_revocation<P, Q, R>(
        &mut self,
        signer: &Key<P, R>,
        pk: &Key<Q, key::PrimaryRole>,
        ua: &UserAttribute)
        -> Result<()>
        where P: key::KeyParts,
              Q: key::KeyParts,
              R: key::KeyRole,
    {
        if self.typ() != SignatureType::CertificationRevocation {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        let mut hash = self.hash_algo().context()?;
        self.hash_user_attribute_binding(&mut hash, pk, ua);
        self.verify_digest(signer, &hash.into_digest()?[..])
    }

    /// Verifies an attested key signature on a user attribute.
    ///
    /// This feature is [experimental](crate#experimental-features).
    ///
    /// Allows the certificate owner to attest to third party
    /// certifications. See [Section 5.2.3.30 of RFC 4880bis] for
    /// details.
    ///
    /// `self` is the attested key signature, `signer` is the key that
    /// allegedly made the signature, `pk` is the primary key, and
    /// `ua` is the user attribute.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `signer` can
    /// made valid signatures; it is up to the caller to make sure the
    /// key is not revoked, not expired, has a valid self-signature,
    /// has a subkey binding signature (if appropriate), has the
    /// signing capability, etc.
    ///
    ///   [Section 5.2.3.30 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-10.html#section-5.2.3.30
    pub fn verify_user_attribute_attestation<P, Q, R>(
        &mut self,
        signer: &Key<P, R>,
        pk: &Key<Q, key::PrimaryRole>,
        ua: &UserAttribute)
        -> Result<()>
        where P: key::KeyParts,
              Q: key::KeyParts,
              R: key::KeyRole,
    {
        if self.typ() != SignatureType::AttestationKey {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        let mut hash = self.hash_algo().context()?;

        if self.attested_certifications()?
            .any(|d| d.len() != hash.digest_size())
        {
            return Err(Error::BadSignature(
                "Wrong number of bytes in certification subpacket".into())
                       .into());
        }

        self.hash_user_attribute_binding(&mut hash, pk, ua);
        self.verify_digest(signer, &hash.into_digest()?[..])
    }

    /// Verifies a signature of a message.
    ///
    /// `self` is the message signature, `signer` is
    /// the key that allegedly made the signature and `msg` is the message.
    ///
    /// This function is for short messages, if you want to verify larger files
    /// use `Verifier`.
    ///
    /// Note: Due to limited context, this only verifies the
    /// cryptographic signature, checks the signature's type, and
    /// checks that the key predates the signature.  Further
    /// constraints on the signature, like creation and expiration
    /// time, or signature revocations must be checked by the caller.
    ///
    /// Likewise, this function does not check whether `signer` can
    /// made valid signatures; it is up to the caller to make sure the
    /// key is not revoked, not expired, has a valid self-signature,
    /// has a subkey binding signature (if appropriate), has the
    /// signing capability, etc.
    pub fn verify_message<M, P, R>(&mut self, signer: &Key<P, R>,
                                   msg: M)
        -> Result<()>
        where M: AsRef<[u8]>,
              P: key::KeyParts,
              R: key::KeyRole,
    {
        if self.typ() != SignatureType::Binary &&
            self.typ() != SignatureType::Text {
            return Err(Error::UnsupportedSignatureType(self.typ()).into());
        }

        // Compute the digest.
        let mut hash = self.hash_algo().context()?;
        let mut digest = vec![0u8; hash.digest_size()];

        hash.update(msg.as_ref());
        self.hash(&mut hash);
        hash.digest(&mut digest)?;

        self.verify_digest(signer, &digest[..])
    }
}

impl From<Signature3> for Packet {
    fn from(s: Signature3) -> Self {
        Packet::Signature(s.into())
    }
}

impl From<Signature3> for super::Signature {
    fn from(s: Signature3) -> Self {
        super::Signature::V3(s)
    }
}

impl From<Signature4> for Packet {
    fn from(s: Signature4) -> Self {
        Packet::Signature(s.into())
    }
}

impl From<Signature4> for super::Signature {
    fn from(s: Signature4) -> Self {
        super::Signature::V4(s)
    }
}

#[cfg(test)]
impl ArbitraryBounded for super::Signature {
    fn arbitrary_bounded(g: &mut Gen, depth: usize) -> Self {
        if bool::arbitrary(g) {
            Signature3::arbitrary_bounded(g, depth).into()
        } else {
            Signature4::arbitrary_bounded(g, depth).into()
        }
    }
}

#[cfg(test)]
impl_arbitrary_with_bound!(super::Signature);

#[cfg(test)]
impl ArbitraryBounded for Signature4 {
    fn arbitrary_bounded(g: &mut Gen, depth: usize) -> Self {
        use mpi::MPI;
        use PublicKeyAlgorithm::*;

        let fields = SignatureFields::arbitrary_bounded(g, depth);
        #[allow(deprecated)]
        let mpis = match fields.pk_algo() {
            RSAEncryptSign | RSASign => mpi::Signature::RSA  {
                s: MPI::arbitrary(g),
            },

            DSA => mpi::Signature::DSA {
                r: MPI::arbitrary(g),
                s: MPI::arbitrary(g),
            },

            EdDSA => mpi::Signature::EdDSA  {
                r: MPI::arbitrary(g),
                s: MPI::arbitrary(g),
            },

            ECDSA => mpi::Signature::ECDSA  {
                r: MPI::arbitrary(g),
                s: MPI::arbitrary(g),
            },

            _ => unreachable!(),
        };

        Signature4 {
            common: Arbitrary::arbitrary(g),
            fields,
            digest_prefix: [Arbitrary::arbitrary(g),
                            Arbitrary::arbitrary(g)],
            mpis,
            computed_digest: None,
            level: 0,
            additional_issuers: Vec::with_capacity(0),
        }
    }
}

#[cfg(test)]
impl_arbitrary_with_bound!(Signature4);

#[cfg(test)]
impl ArbitraryBounded for Signature3 {
    fn arbitrary_bounded(g: &mut Gen, _depth: usize) -> Self {
        use mpi::MPI;
        use PublicKeyAlgorithm::*;

        let pk_algo = PublicKeyAlgorithm::arbitrary_for_signing(g);

        #[allow(deprecated)]
        let mpis = match pk_algo {
            RSAEncryptSign | RSASign => mpi::Signature::RSA  {
                s: MPI::arbitrary(g),
            },

            DSA => mpi::Signature::DSA {
                r: MPI::arbitrary(g),
                s: MPI::arbitrary(g),
            },

            EdDSA => mpi::Signature::EdDSA  {
                r: MPI::arbitrary(g),
                s: MPI::arbitrary(g),
            },

            ECDSA => mpi::Signature::ECDSA  {
                r: MPI::arbitrary(g),
                s: MPI::arbitrary(g),
            },

            _ => unreachable!(),
        };

        Signature3::new(
            SignatureType::arbitrary(g),
            Timestamp::arbitrary(g),
            KeyID::arbitrary(g),
            pk_algo,
            HashAlgorithm::arbitrary(g),
            [Arbitrary::arbitrary(g), Arbitrary::arbitrary(g)],
            mpis)
    }
}

#[cfg(test)]
impl_arbitrary_with_bound!(Signature3);

#[cfg(test)]
mod test {
    use super::*;
    use crate::KeyID;
    use crate::cert::prelude::*;
    use crate::crypto;
    use crate::parse::Parse;
    use crate::packet::Key;
    use crate::packet::key::Key4;
    use crate::types::Curve;
    use crate::policy::StandardPolicy as P;

    #[cfg(feature = "compression-deflate")]
    #[test]
    fn signature_verification_test() {
        use super::*;

        use crate::Cert;
        use crate::parse::{PacketParserResult, PacketParser};

        struct Test<'a> {
            key: &'a str,
            data: &'a str,
            good: usize,
        }

        let tests = [
            Test {
                key: "neal.pgp",
                data: "signed-1.gpg",
                good: 1,
            },
            Test {
                key: "neal.pgp",
                data: "signed-1-sha1-neal.gpg",
                good: 1,
            },
            Test {
                key: "testy.pgp",
                data: "signed-1-sha256-testy.gpg",
                good: 1,
            },
            Test {
                key: "dennis-simon-anton.pgp",
                data: "signed-1-dsa.pgp",
                good: 1,
            },
            Test {
                key: "erika-corinna-daniela-simone-antonia-nistp256.pgp",
                data: "signed-1-ecdsa-nistp256.pgp",
                good: 1,
            },
            Test {
                key: "erika-corinna-daniela-simone-antonia-nistp384.pgp",
                data: "signed-1-ecdsa-nistp384.pgp",
                good: 1,
            },
            Test {
                key: "erika-corinna-daniela-simone-antonia-nistp521.pgp",
                data: "signed-1-ecdsa-nistp521.pgp",
                good: 1,
            },
            Test {
                key: "emmelie-dorothea-dina-samantha-awina-ed25519.pgp",
                data: "signed-1-eddsa-ed25519.pgp",
                good: 1,
            },
            Test {
                key: "emmelie-dorothea-dina-samantha-awina-ed25519.pgp",
                data: "signed-twice-by-ed25519.pgp",
                good: 2,
            },
            Test {
                key: "neal.pgp",
                data: "signed-1-notarized-by-ed25519.pgp",
                good: 1,
            },
            Test {
                key: "emmelie-dorothea-dina-samantha-awina-ed25519.pgp",
                data: "signed-1-notarized-by-ed25519.pgp",
                good: 1,
            },
            // Check with the wrong key.
            Test {
                key: "neal.pgp",
                data: "signed-1-sha256-testy.gpg",
                good: 0,
            },
            Test {
                key: "neal.pgp",
                data: "signed-2-partial-body.gpg",
                good: 1,
            },
        ];

        for test in tests.iter() {
            eprintln!("{}, expect {} good signatures:",
                      test.data, test.good);

            let cert = Cert::from_bytes(crate::tests::key(test.key)).unwrap();

            if ! cert.keys().all(|k| k.pk_algo().is_supported()) {
                eprintln!("Skipping because one algorithm is not supported");
                continue;
            }

            if let Some(curve) = match cert.primary_key().mpis() {
                mpi::PublicKey::EdDSA { curve, .. } => Some(curve),
                mpi::PublicKey::ECDSA { curve, .. } => Some(curve),
                _ => None,
            } {
                if ! curve.is_supported() {
                    eprintln!("Skipping because we don't support {}", curve);
                    continue;
                }
            }

            let mut good = 0;
            let mut ppr = PacketParser::from_bytes(
                crate::tests::message(test.data)).unwrap();
            while let PacketParserResult::Some(mut pp) = ppr {
                if let Packet::Signature(sig) = &mut pp.packet {
                    let result = sig.verify(cert.primary_key().key())
                        .map(|_| true).unwrap_or(false);
                    eprintln!("  Primary {:?}: {:?}",
                              cert.fingerprint(), result);
                    if result {
                        good += 1;
                    }

                    for sk in cert.subkeys() {
                        let result = sig.verify(sk.key())
                            .map(|_| true).unwrap_or(false);
                        eprintln!("   Subkey {:?}: {:?}",
                                  sk.key().fingerprint(), result);
                        if result {
                            good += 1;
                        }
                    }
                }

                // Get the next packet.
                ppr = pp.recurse().unwrap().1;
            }

            assert_eq!(good, test.good, "Signature verification failed.");
        }
    }

    #[test]
    fn signature_level() {
        use crate::PacketPile;
        let p = PacketPile::from_bytes(
            crate::tests::message("signed-1-notarized-by-ed25519.pgp")).unwrap()
            .into_children().collect::<Vec<Packet>>();

        if let Packet::Signature(ref sig) = &p[3] {
            assert_eq!(sig.level(), 0);
        } else {
            panic!("expected signature")
        }

        if let Packet::Signature(ref sig) = &p[4] {
            assert_eq!(sig.level(), 1);
        } else {
            panic!("expected signature")
        }
    }

    #[test]
    fn sign_verify() {
        let hash_algo = HashAlgorithm::SHA512;
        let mut hash = vec![0; hash_algo.context().unwrap().digest_size()];
        crypto::random(&mut hash);

        for key in &[
            "testy-private.pgp",
            "dennis-simon-anton-private.pgp",
            "erika-corinna-daniela-simone-antonia-nistp256-private.pgp",
            "erika-corinna-daniela-simone-antonia-nistp384-private.pgp",
            "erika-corinna-daniela-simone-antonia-nistp521-private.pgp",
            "emmelie-dorothea-dina-samantha-awina-ed25519-private.pgp",
        ] {
            eprintln!("{}...", key);
            let cert = Cert::from_bytes(crate::tests::key(key)).unwrap();

            if ! cert.primary_key().pk_algo().is_supported() {
                eprintln!("Skipping because we don't support the algo");
                continue;
            }

            if let Some(curve) = match cert.primary_key().mpis() {
                mpi::PublicKey::EdDSA { curve, .. } => Some(curve),
                mpi::PublicKey::ECDSA { curve, .. } => Some(curve),
                _ => None,
            } {
                if ! curve.is_supported() {
                eprintln!("Skipping because we don't support {}", curve);
                    continue;
                }
            }

            let mut pair = cert.primary_key().key().clone()
                .parts_into_secret().unwrap()
                .into_keypair()
                .expect("secret key is encrypted/missing");

            let sig = SignatureBuilder::new(SignatureType::Binary);
            let hash = hash_algo.context().unwrap();

            // Make signature.
            let mut sig = sig.sign_hash(&mut pair, hash).unwrap();

            // Good signature.
            let mut hash = hash_algo.context().unwrap();
            sig.hash(&mut hash);
            let mut digest = vec![0u8; hash.digest_size()];
            hash.digest(&mut digest).unwrap();
            sig.verify_digest(pair.public(), &digest[..]).unwrap();

            // Bad signature.
            digest[0] ^= 0xff;
            sig.verify_digest(pair.public(), &digest[..]).unwrap_err();
        }
    }

    #[test]
    fn sign_message() {
        use crate::types::Curve::*;

        for curve in vec![
            Ed25519,
            NistP256,
            NistP384,
            NistP521,
        ] {
            if ! curve.is_supported() {
                eprintln!("Skipping unsupported {:?}", curve);
                continue;
            }

            let key: Key<key::SecretParts, key::PrimaryRole>
                = Key4::generate_ecc(true, curve).unwrap().into();
            let msg = b"Hello, World";
            let mut pair = key.into_keypair().unwrap();
            let mut sig = SignatureBuilder::new(SignatureType::Binary)
                .sign_message(&mut pair, msg).unwrap();

            sig.verify_message(pair.public(), msg).unwrap();
        }
    }

    #[test]
    fn verify_message() {
        let cert = Cert::from_bytes(crate::tests::key(
                "emmelie-dorothea-dina-samantha-awina-ed25519.pgp")).unwrap();
        let msg = crate::tests::manifesto();
        let p = Packet::from_bytes(
            crate::tests::message("a-cypherpunks-manifesto.txt.ed25519.sig"))
            .unwrap();
        let mut sig = if let Packet::Signature(s) = p {
            s
        } else {
            panic!("Expected a Signature, got: {:?}", p);
        };

        sig.verify_message(cert.primary_key().key(), msg).unwrap();
    }

    #[test]
    fn verify_v3_sig() {
        if ! PublicKeyAlgorithm::DSA.is_supported() {
            return;
        }

        let cert = Cert::from_bytes(crate::tests::key(
                "dennis-simon-anton-private.pgp")).unwrap();
        let msg = crate::tests::manifesto();
        let p = Packet::from_bytes(
            crate::tests::message("a-cypherpunks-manifesto.txt.dennis-simon-anton-v3.sig"))
            .unwrap();
        let mut sig = if let Packet::Signature(s) = p {
            assert_eq!(s.version(), 3);
            s
        } else {
            panic!("Expected a Signature, got: {:?}", p);
        };

        sig.verify_message(cert.primary_key().key(), msg).unwrap();
    }

    #[test]
    fn sign_with_short_ed25519_secret_key() {
        // 20 byte sec key
        let secret_key = [
            0x0,0x0,
            0x0,0x0,0x0,0x0,0x0,0x0,0x0,0x0,0x0,0x0,
            0x1,0x2,0x2,0x2,0x2,0x2,0x2,0x2,0x2,0x2,
            0x1,0x2,0x2,0x2,0x2,0x2,0x2,0x2,0x2,0x2
        ];

        let key: key::SecretKey = Key4::import_secret_ed25519(&secret_key, None)
            .unwrap().into();

        let mut pair = key.into_keypair().unwrap();
        let msg = b"Hello, World";
        let mut hash = HashAlgorithm::SHA256.context().unwrap();

        hash.update(&msg[..]);

        SignatureBuilder::new(SignatureType::Text)
            .sign_hash(&mut pair, hash).unwrap();
    }

    #[test]
    fn verify_gpg_3rd_party_cert() {
        use crate::Cert;

        let p = &P::new();

        let test1 = Cert::from_bytes(
            crate::tests::key("test1-certification-key.pgp")).unwrap();
        let cert_key1 = test1.keys().with_policy(p, None)
            .for_certification()
            .next()
            .map(|ka| ka.key())
            .unwrap();
        let test2 = Cert::from_bytes(
            crate::tests::key("test2-signed-by-test1.pgp")).unwrap();
        let uid = test2.userids().with_policy(p, None).next().unwrap();
        let mut cert = uid.certifications().next().unwrap().clone();

        cert.verify_userid_binding(cert_key1,
                                   test2.primary_key().key(),
                                   uid.userid()).unwrap();
    }

    #[test]
    fn normalize() {
        use crate::Fingerprint;
        use crate::packet::signature::subpacket::*;

        let key : key::SecretKey
            = Key4::generate_ecc(true, Curve::Ed25519).unwrap().into();
        let mut pair = key.into_keypair().unwrap();
        let msg = b"Hello, World";
        let mut hash = HashAlgorithm::SHA256.context().unwrap();
        hash.update(&msg[..]);

        let fp = Fingerprint::from_bytes(b"bbbbbbbbbbbbbbbbbbbb");
        let keyid = KeyID::from(&fp);

        // First, make sure any superfluous subpackets are removed,
        // yet the Issuer, IssuerFingerprint and EmbeddedSignature
        // ones are kept.
        let mut builder = SignatureBuilder::new(SignatureType::Text);
        builder.unhashed_area_mut().add(Subpacket::new(
            SubpacketValue::IssuerFingerprint(fp.clone()), false).unwrap())
            .unwrap();
        builder.unhashed_area_mut().add(Subpacket::new(
            SubpacketValue::Issuer(keyid.clone()), false).unwrap())
            .unwrap();
        // This subpacket does not belong there, and should be
        // removed.
        builder.unhashed_area_mut().add(Subpacket::new(
            SubpacketValue::PreferredSymmetricAlgorithms(Vec::new()),
            false).unwrap()).unwrap();

        // Build and add an embedded sig.
        let embedded_sig = SignatureBuilder::new(SignatureType::PrimaryKeyBinding)
            .sign_hash(&mut pair, hash.clone()).unwrap();
        builder.unhashed_area_mut().add(Subpacket::new(
            SubpacketValue::EmbeddedSignature(embedded_sig), false).unwrap())
            .unwrap();
        let sig = builder.sign_hash(&mut pair,
                                    hash.clone()).unwrap().normalize();
        assert_eq!(sig.unhashed_area().iter().count(), 3);
        assert_eq!(*sig.unhashed_area().iter().next().unwrap(),
                   Subpacket::new(SubpacketValue::Issuer(keyid.clone()),
                                  false).unwrap());
        assert_eq!(sig.unhashed_area().iter().nth(1).unwrap().tag(),
                   SubpacketTag::EmbeddedSignature);
        assert_eq!(*sig.unhashed_area().iter().nth(2).unwrap(),
                   Subpacket::new(SubpacketValue::IssuerFingerprint(fp.clone()),
                                  false).unwrap());
    }

    #[test]
    fn standalone_signature_roundtrip() {
        let key : key::SecretKey
            = Key4::generate_ecc(true, Curve::Ed25519).unwrap().into();
        let mut pair = key.into_keypair().unwrap();

        let mut sig = SignatureBuilder::new(SignatureType::Standalone)
            .sign_standalone(&mut pair)
            .unwrap();

        sig.verify_standalone(pair.public()).unwrap();
    }

    #[test]
    fn timestamp_signature() {
        if ! PublicKeyAlgorithm::DSA.is_supported() {
            eprintln!("Skipping test, algorithm is not supported.");
            return;
        }

        let alpha = Cert::from_bytes(crate::tests::file(
            "contrib/gnupg/keys/alpha.pgp")).unwrap();
        let p = Packet::from_bytes(crate::tests::file(
            "contrib/gnupg/timestamp-signature-by-alice.asc")).unwrap();
        if let Packet::Signature(mut sig) = p {
            let mut hash = sig.hash_algo().context().unwrap();
            sig.hash_standalone(&mut hash);
            let digest = hash.into_digest().unwrap();
            eprintln!("{}", crate::fmt::hex::encode(&digest));
            sig.verify_timestamp(alpha.primary_key().key()).unwrap();
        } else {
            panic!("expected a signature packet");
        }
    }

    #[test]
    fn timestamp_signature_roundtrip() {
        let key : key::SecretKey
            = Key4::generate_ecc(true, Curve::Ed25519).unwrap().into();
        let mut pair = key.into_keypair().unwrap();

        let mut sig = SignatureBuilder::new(SignatureType::Timestamp)
            .sign_timestamp(&mut pair)
            .unwrap();

        sig.verify_timestamp(pair.public()).unwrap();
    }

    #[test]
    fn get_issuers_prefers_fingerprints() -> Result<()> {
        use crate::KeyHandle;
        for f in [
            // This has Fingerprint in the hashed, Issuer in the
            // unhashed area.
            "messages/sig.gpg",
            // This has [Issuer, Fingerprint] in the hashed area.
            "contrib/gnupg/timestamp-signature-by-alice.asc",
        ].iter() {
            let p = Packet::from_bytes(crate::tests::file(f))?;
            if let Packet::Signature(sig) = p {
                let issuers = sig.get_issuers();
                assert_match!(KeyHandle::Fingerprint(_) = &issuers[0]);
                assert_match!(KeyHandle::KeyID(_) = &issuers[1]);
            } else {
                panic!("expected a signature packet");
            }
        }
        Ok(())
    }

    /// Checks that binding signatures of newly created certificates
    /// can be conveniently and robustly be overwritten without
    /// fiddling with creation timestamps.
    #[test]
    fn binding_signatures_are_overrideable() -> Result<()> {
        use crate::packet::signature::subpacket::NotationDataFlags;
        let notation_key = "override-test@sequoia-pgp.org";
        let p = &P::new();

        // Create a certificate and try to update the userid's binding
        // signature.
        let (mut alice, _) =
            CertBuilder::general_purpose(None, Some("alice@example.org"))
            .generate()?;
        let mut primary_signer = alice.primary_key().key().clone()
            .parts_into_secret()?.into_keypair()?;
        assert_eq!(alice.userids().len(), 1);
        assert_eq!(alice.userids().next().unwrap().self_signatures().count(), 1);

        const TRIES: u64 = 5;
        assert!(TRIES * 10 < SIG_BACKDATE_BY);
        for i in 0..TRIES {
            assert_eq!(alice.userids().next().unwrap().self_signatures().count(),
                       1 + i as usize);

            // Get the binding signature so that we can modify it.
            let sig = alice.with_policy(p, None)?.userids().next().unwrap()
                .binding_signature().clone();

            let new_sig = match
                SignatureBuilder::from(sig)
                .set_notation(notation_key,
                              i.to_string().as_bytes(),
                              NotationDataFlags::empty().set_human_readable(),
                              false)?
                .sign_userid_binding(&mut primary_signer,
                                     alice.primary_key().component(),
                                     &alice.userids().next().unwrap()) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Failed to make {} signatures on top of \
                                   the original one.", i);
                        return Err(e); // Not cool.
                    },
                };

            // Merge it and check that the new binding signature is
            // the current one.
            alice = alice.insert_packets(new_sig.clone())?;
            let sig = alice.with_policy(p, None)?.userids().next().unwrap()
                .binding_signature();
            assert_eq!(sig, &new_sig);
        }

        Ok(())
    }

    /// Checks that subpackets are marked as authentic on signature
    /// verification.
    #[test]
    fn subpacket_authentication() -> Result<()> {
        use subpacket::{Subpacket, SubpacketValue};

        // We'll study this certificate, because it contains a
        // signing-capable subkey.
        let mut pp = crate::PacketPile::from_bytes(crate::tests::key(
            "emmelie-dorothea-dina-samantha-awina-ed25519.pgp"))?;
        assert_eq!(pp.children().count(), 5);

        // The signatures have not been verified, hence no subpacket
        // is authenticated.
        if let Some(Packet::Signature(sig)) = pp.path_ref_mut(&[4]) {
            assert!(sig.hashed_area().iter().all(|p| ! p.authenticated()));
            assert!(sig.unhashed_area().iter().all(|p| ! p.authenticated()));

            // Add a bogus issuer subpacket.
            sig.unhashed_area_mut().add(Subpacket::new(
                SubpacketValue::Issuer("AAAA BBBB CCCC DDDD".parse()?),
                false)?)?;
        } else {
            panic!("expected a signature");
        }

        // Break the userid binding signature.
        if let Some(Packet::Signature(sig)) = pp.path_ref_mut(&[2]) {
            assert!(sig.hashed_area().iter().all(|p| ! p.authenticated()));
            assert!(sig.unhashed_area().iter().all(|p| ! p.authenticated()));

            // Add a bogus issuer subpacket to the hashed area
            // breaking the signature.
            sig.hashed_area_mut().add(Subpacket::new(
                SubpacketValue::Issuer("AAAA BBBB CCCC DDDD".parse()?),
                false)?)?;
        } else {
            panic!("expected a signature");
        }

        // Parse into cert verifying the signatures.
        use std::convert::TryFrom;
        let cert = Cert::try_from(pp)?;
        assert_eq!(cert.bad_signatures().count(), 1);
        assert_eq!(cert.keys().subkeys().count(), 1);
        let subkey = cert.keys().subkeys().next().unwrap();
        assert_eq!(subkey.self_signatures().count(), 1);

        // All the authentic information in the self signature has
        // been authenticated by the verification process.
        let sig = &subkey.self_signatures().next().unwrap();
        assert!(sig.hashed_area().iter().all(|p| p.authenticated()));
        // All but our fake issuer information.
        assert!(sig.unhashed_area().iter().all(|p| {
            if let SubpacketValue::Issuer(id) = p.value() {
                if id == &"AAAA BBBB CCCC DDDD".parse().unwrap() {
                    // Our fake id...
                    true
                } else {
                    p.authenticated()
                }
            } else {
                p.authenticated()
            }
        }));
        // Check the subpackets in the embedded signature.
        let sig = sig.embedded_signatures().next().unwrap();
        assert!(sig.hashed_area().iter().all(|p| p.authenticated()));
        assert!(sig.unhashed_area().iter().all(|p| p.authenticated()));

        // No information in the bad signature has been authenticated.
        let sig = cert.bad_signatures().next().unwrap();
        assert!(sig.hashed_area().iter().all(|p| ! p.authenticated()));
        assert!(sig.unhashed_area().iter().all(|p| ! p.authenticated()));
        Ok(())
    }

    /// Checks that signature normalization adds missing issuer
    /// information.
    #[test]
    fn normalization_adds_missing_issuers() -> Result<()> {
        use subpacket::SubpacketTag;

        let mut pp = crate::PacketPile::from_bytes(crate::tests::key(
            "emmelie-dorothea-dina-samantha-awina-ed25519.pgp"))?;
        assert_eq!(pp.children().count(), 5);

        // Remove the issuer subpacket from a binding signature.
        if let Some(Packet::Signature(sig)) = pp.path_ref_mut(&[4]) {
            sig.unhashed_area_mut().remove_all(SubpacketTag::Issuer);
            assert_eq!(sig.get_issuers().len(), 1);
        } else {
            panic!("expected a signature");
        }

        // Verify the subkey binding without parsing into cert.
        let primary_key =
            if let Some(Packet::PublicKey(key)) = pp.path_ref(&[0]) {
                key
            } else {
                panic!("Expected a primary key");
            };
        let subkey =
            if let Some(Packet::PublicSubkey(key)) = pp.path_ref(&[3]) {
                key
            } else {
                panic!("Expected a subkey");
            };
        let mut sig =
            if let Some(Packet::Signature(sig)) = pp.path_ref(&[4]) {
                sig.clone()
            } else {
                panic!("expected a signature");
            };


        // The signature has only an issuer fingerprint.
        assert_eq!(sig.get_issuers().len(), 1);
        assert_eq!(sig.subpackets(SubpacketTag::Issuer).count(), 0);
        // But normalization after verification adds the missing
        // information.
        sig.verify_subkey_binding(primary_key, primary_key, subkey)?;
        let normalized_sig = sig.normalize();
        assert_eq!(normalized_sig.subpackets(SubpacketTag::Issuer).count(), 1);
        Ok(())
    }

    /// Tests signature merging.
    #[test]
    fn merging() -> Result<()> {
        use crate::packet::signature::subpacket::*;

        let key: key::SecretKey
            = Key4::generate_ecc(true, Curve::Ed25519)?.into();
        let mut pair = key.into_keypair()?;
        let msg = b"Hello, World";
        let mut hash = HashAlgorithm::SHA256.context()?;
        hash.update(&msg[..]);

        let fp = pair.public().fingerprint();
        let keyid = KeyID::from(&fp);

        // Make a feeble signature with issuer information in the
        // unhashed area.
        let sig = SignatureBuilder::new(SignatureType::Text)
            .modify_unhashed_area(|mut a| {
                a.add(Subpacket::new(
                    SubpacketValue::IssuerFingerprint(fp.clone()), false)?)?;
                a.add(Subpacket::new(
                    SubpacketValue::Issuer(keyid.clone()), false)?)?;
                Ok(a)
            })?
            .sign_hash(&mut pair, hash.clone())?;

        // Try to displace the issuer information.
        let dummy: crate::KeyID = "AAAA BBBB CCCC DDDD".parse()?;
        let mut malicious = sig.clone();
        malicious.unhashed_area_mut().clear();
        loop {
            let r = malicious.unhashed_area_mut().add(Subpacket::new(
                SubpacketValue::Issuer(dummy.clone()), false)?);
            if r.is_err() {
                break;
            }
        }

        // Merge and check that the issuer information is intact.
        // This works without any issuer being authenticated because
        // of the deduplicating nature of the merge.
        let merged = sig.clone().merge(malicious.clone())?;
        let issuers = merged.get_issuers();
        assert_eq!(issuers.len(), 3);
        assert!(issuers.contains(&KeyHandle::from(&fp)));
        assert!(issuers.contains(&KeyHandle::from(&keyid)));
        assert!(issuers.contains(&KeyHandle::from(&dummy)));

        // Same, but the other way around.
        let merged = malicious.clone().merge(sig.clone())?;
        let issuers = merged.get_issuers();
        assert_eq!(issuers.len(), 3);
        assert!(issuers.contains(&KeyHandle::from(&fp)));
        assert!(issuers.contains(&KeyHandle::from(&keyid)));
        assert!(issuers.contains(&KeyHandle::from(&dummy)));

        // Try to displace the issuer information using garbage
        // packets.
        let mut malicious = sig.clone();
        malicious.unhashed_area_mut().clear();
        let mut i: u64 = 0;
        loop {
            let r = malicious.unhashed_area_mut().add(Subpacket::new(
                SubpacketValue::Unknown {
                    tag: SubpacketTag::Unknown(231),
                    body: i.to_be_bytes().iter().cloned().collect(),
                }, false)?);
            if r.is_err() {
                break;
            }
            i += 1;
        }

        // Merge and check that the issuer information is intact.
        // This works without any issuer being authenticated because
        // the merge prefers plausible packets.
        let merged = sig.clone().merge(malicious.clone())?;
        let issuers = merged.get_issuers();
        assert_eq!(issuers.len(), 2);
        assert!(issuers.contains(&KeyHandle::from(&fp)));
        assert!(issuers.contains(&KeyHandle::from(&keyid)));

        // Same, but the other way around.
        let merged = malicious.clone().merge(sig.clone())?;
        let issuers = merged.get_issuers();
        assert_eq!(issuers.len(), 2);
        assert!(issuers.contains(&KeyHandle::from(&fp)));
        assert!(issuers.contains(&KeyHandle::from(&keyid)));

        // Try to displace the issuer information by using random keyids.
        let mut malicious = sig.clone();
        malicious.unhashed_area_mut().clear();
        let mut i: u64 = 1;
        loop {
            let r = malicious.unhashed_area_mut().add(Subpacket::new(
                SubpacketValue::Issuer(i.into()), false)?);
            if r.is_err() {
                break;
            }
            i += 1;
        }

        // Merge and check that the issuer information is intact.
        // This works because the issuer information is being
        // authenticated by the verification, and the merge process
        // prefers authenticated information.
        let mut verified = sig.clone();
        verified.verify_hash(pair.public(), hash.clone())?;

        let merged = verified.clone().merge(malicious.clone())?;
        let issuers = merged.get_issuers();
        assert!(issuers.contains(&KeyHandle::from(&fp)));
        assert!(issuers.contains(&KeyHandle::from(&keyid)));

        // Same, but the other way around.
        let merged = malicious.clone().merge(verified.clone())?;
        let issuers = merged.get_issuers();
        assert!(issuers.contains(&KeyHandle::from(&fp)));
        assert!(issuers.contains(&KeyHandle::from(&keyid)));

        Ok(())
    }
}
