//! Signature subpackets.
//!
//! OpenPGP signature packets include a set of key-value attributes
//! called subpackets.  These subpackets are used to indicate when a
//! signature was created, who created the signature, user &
//! implementation preferences, etc.  The full details are in [Section
//! 5.2.3.1 of RFC 4880].
//!
//! [Section 5.2.3.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.1
//!
//! The standard assigns each subpacket a numeric id, and describes
//! the format of its value.  One subpacket is called Notation Data
//! and is intended as a generic key-value store.  The combined size
//! of the subpackets (including notation data) is limited to 64 KB.
//!
//! Subpackets and notations can be marked as critical.  If an OpenPGP
//! implementation processes a packet that includes critical
//! subpackets or notations that it does not understand, it is
//! required to abort processing.  This allows for forwards compatible
//! changes by indicating whether it is safe to ignore an unknown
//! subpacket or notation.
//!
//! A number of methods are defined on [`Signature`] for working with
//! subpackets.
//!
//! [`Signature`]: super::super::Signature
//!
//! # Examples
//!
//! Print any Issuer Fingerprint subpackets:
//!
//! ```rust
//! # use sequoia_openpgp as openpgp;
//! # use openpgp::Result;
//! # use openpgp::Packet;
//! # use openpgp::parse::{Parse, PacketParserResult, PacketParser};
//! #
//! # f(include_bytes!("../../../tests/data/messages/signed.gpg"));
//! #
//! # fn f(message_data: &[u8]) -> Result<()> {
//! let mut ppr = PacketParser::from_bytes(message_data)?;
//! while let PacketParserResult::Some(mut pp) = ppr {
//!     if let Packet::Signature(ref sig) = pp.packet {
//!         for fp in sig.issuer_fingerprints() {
//!             eprintln!("Signature allegedly issued by: {}", fp.to_string());
//!         }
//!     }
//!
//!     // Get the next packet.
//!     ppr  = pp.recurse()?.1;
//! }
//! # Ok(())
//! # }
//! ```

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::{TryInto, TryFrom};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::ops::{Deref, DerefMut};
use std::fmt;
use std::cmp;
use std::time;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};
#[cfg(test)]
use crate::packet::signature::ArbitraryBounded;

use crate::{
    Error,
    Result,
    packet::header::BodyLength,
    packet::Signature,
    packet::signature::{self, Signature4},
    packet::key,
    packet::Key,
    Fingerprint,
    KeyID,
    serialize::MarshalInto,
};
use crate::types::{
    AEADAlgorithm,
    CompressionAlgorithm,
    Duration,
    Features,
    HashAlgorithm,
    KeyFlags,
    KeyServerPreferences,
    PublicKeyAlgorithm,
    ReasonForRevocation,
    RevocationKey,
    SymmetricAlgorithm,
    Timestamp,
};

lazy_static::lazy_static!{
    /// The default amount of tolerance to use when comparing
    /// some timestamps.
    ///
    /// Used by `Subpacket::signature_alive`.
    ///
    /// When determining whether a timestamp generated on another
    /// machine is valid *now*, we need to account for clock skew.
    /// (Note: you don't normally need to consider clock skew when
    /// evaluating a signature's validity at some time in the past.)
    ///
    /// We tolerate half an hour of skew based on the following
    /// anecdote: In 2019, a developer using Sequoia in a Windows VM
    /// running inside of Virtual Box on Mac OS X reported that he
    /// typically observed a few minutes of clock skew and
    /// occasionally saw over 20 minutes of clock skew.
    ///
    /// Note: when new messages override older messages, and their
    /// signatures are evaluated at some arbitrary point in time, an
    /// application may not see a consistent state if it uses a
    /// tolerance.  Consider an application that has two messages and
    /// wants to get the current message at time te:
    ///
    ///   - t0: message 0
    ///   - te: "get current message"
    ///   - t1: message 1
    ///
    /// If te is close to t1, then t1 may be considered valid, which
    /// is probably not what you want.
    pub static ref CLOCK_SKEW_TOLERANCE: time::Duration
        = time::Duration::new(30 * 60, 0);

}

/// The subpacket types.
///
/// The `SubpacketTag` enum holds a [`Subpacket`]'s identifier, the
/// so-called tag.
///
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
#[non_exhaustive]
#[derive(Debug)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Clone, Copy)]
pub enum SubpacketTag {
    /// The time the signature was made.
    ///
    /// See [Section 5.2.3.4 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.4 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    SignatureCreationTime,
    /// The validity period of the signature.
    ///
    /// The validity is relative to the time stored in the signature's
    /// Signature Creation Time subpacket.
    ///
    /// See [Section 5.2.3.10 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.10 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.10
    SignatureExpirationTime,
    /// Whether a signature should be published.
    ///
    /// See [Section 5.2.3.11 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.11 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.11
    ExportableCertification,
    /// Signer asserts that the key is not only valid but also trustworthy at
    /// the specified level.
    ///
    /// See [Section 5.2.3.13 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.13 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.13
    TrustSignature,
    /// Used in conjunction with Trust Signature packets (of level > 0) to
    /// limit the scope of trust that is extended.
    ///
    /// See [Section 5.2.3.14 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.14 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.14
    RegularExpression,
    /// Whether a signature can later be revoked.
    ///
    /// See [Section 5.2.3.12 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.12 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.12
    Revocable,
    /// The validity period of the key.
    ///
    /// The validity period is relative to the key's (not the signature's) creation time.
    ///
    /// See [Section 5.2.3.6 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.6 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    KeyExpirationTime,
    /// Deprecated
    PlaceholderForBackwardCompatibility,
    /// The Symmetric algorithms that the certificate holder prefers.
    ///
    /// See [Section 5.2.3.7 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.7 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.7
    PreferredSymmetricAlgorithms,
    /// Authorizes the specified key to issue revocation signatures for this
    /// certificate.
    ///
    /// See [Section 5.2.3.15 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.15 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.15
    RevocationKey,
    /// The OpenPGP Key ID of the key issuing the signature.
    ///
    /// See [Section 5.2.3.5 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.5 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    Issuer,
    /// A "notation" on the signature.
    ///
    /// See [Section 5.2.3.16 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.16 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    NotationData,
    /// The Hash algorithms that the certificate holder prefers.
    ///
    /// See [Section 5.2.3.8 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.8 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.8
    PreferredHashAlgorithms,
    /// The compression algorithms that the certificate holder prefers.
    ///
    /// See [Section 5.2.3.9 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.9 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.9
    PreferredCompressionAlgorithms,
    /// A list of flags that indicate preferences that the certificate
    /// holder has about how the key is handled by a key server.
    ///
    /// See [Section 5.2.3.17 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.17 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.17
    KeyServerPreferences,
    /// The URI of a key server where the certificate holder keeps
    /// their certificate up to date.
    ///
    /// See [Section 5.2.3.18 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.18 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.18
    PreferredKeyServer,
    /// A flag in a User ID's self-signature that states whether this
    /// User ID is the primary User ID for this certificate.
    ///
    /// See [Section 5.2.3.19 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.19 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.19
    PrimaryUserID,
    /// The URI of a document that describes the policy under which
    /// the signature was issued.
    ///
    /// See [Section 5.2.3.20 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.20 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.20
    PolicyURI,
    /// A list of flags that hold information about a key.
    ///
    /// See [Section 5.2.3.21 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.21 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.21
    KeyFlags,
    /// The User ID that is responsible for the signature.
    ///
    /// See [Section 5.2.3.22 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.22 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.22
    SignersUserID,
    /// The reason for a revocation, used in key revocations and
    /// certification revocation signatures.
    ///
    /// See [Section 5.2.3.23 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.23 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.23
    ReasonForRevocation,
    /// The OpenPGP features a user's implementation supports.
    ///
    /// See [Section 5.2.3.24 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.24 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.24
    Features,
    /// A signature to which this signature refers.
    ///
    /// See [Section 5.2.3.25 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.25 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.25
    SignatureTarget,
    /// A complete Signature packet body.
    ///
    /// This is used to store a backsig in a subkey binding signature.
    ///
    /// See [Section 5.2.3.26 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.26 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.26
    EmbeddedSignature,
    /// The Fingerprint of the key that issued the signature (proposed).
    ///
    /// See [Section 5.2.3.28 of RFC 4880bis] for details.
    ///
    ///  [Section 5.2.3.28 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    IssuerFingerprint,
    /// The AEAD algorithms that the certificate holder prefers (proposed).
    ///
    /// See [Section 5.2.3.8 of RFC 4880bis] for details.
    ///
    ///  [Section 5.2.3.8 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.8
    PreferredAEADAlgorithms,
    /// Who the signed message was intended for (proposed).
    ///
    /// See [Section 5.2.3.29 of RFC 4880bis] for details.
    ///
    ///  [Section 5.2.3.29 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.29
    IntendedRecipient,
    /// The Attested Certifications subpacket (proposed).
    ///
    /// Allows the certificate holder to attest to third party
    /// certifications, allowing them to be distributed with the
    /// certificate.  This can be used to address certificate flooding
    /// concerns.
    ///
    /// See [Section 5.2.3.30 of RFC 4880bis] for details.
    ///
    ///  [Section 5.2.3.30 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-10.html#section-5.2.3.30
    AttestedCertifications,
    /// Reserved subpacket tag.
    Reserved(u8),
    /// Private subpacket tag.
    Private(u8),
    /// Unknown subpacket tag.
    Unknown(u8),

    // If you add a new variant, make sure to add it to the
    // conversions and to SUBPACKET_TAG_VARIANTS.
}
assert_send_and_sync!(SubpacketTag);

impl fmt::Display for SubpacketTag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<u8> for SubpacketTag {
    fn from(u: u8) -> Self {
        match u {
            2 => SubpacketTag::SignatureCreationTime,
            3 => SubpacketTag::SignatureExpirationTime,
            4 => SubpacketTag::ExportableCertification,
            5 => SubpacketTag::TrustSignature,
            6 => SubpacketTag::RegularExpression,
            7 => SubpacketTag::Revocable,
            9 => SubpacketTag::KeyExpirationTime,
            10 => SubpacketTag::PlaceholderForBackwardCompatibility,
            11 => SubpacketTag::PreferredSymmetricAlgorithms,
            12 => SubpacketTag::RevocationKey,
            16 => SubpacketTag::Issuer,
            20 => SubpacketTag::NotationData,
            21 => SubpacketTag::PreferredHashAlgorithms,
            22 => SubpacketTag::PreferredCompressionAlgorithms,
            23 => SubpacketTag::KeyServerPreferences,
            24 => SubpacketTag::PreferredKeyServer,
            25 => SubpacketTag::PrimaryUserID,
            26 => SubpacketTag::PolicyURI,
            27 => SubpacketTag::KeyFlags,
            28 => SubpacketTag::SignersUserID,
            29 => SubpacketTag::ReasonForRevocation,
            30 => SubpacketTag::Features,
            31 => SubpacketTag::SignatureTarget,
            32 => SubpacketTag::EmbeddedSignature,
            33 => SubpacketTag::IssuerFingerprint,
            34 => SubpacketTag::PreferredAEADAlgorithms,
            35 => SubpacketTag::IntendedRecipient,
            37 => SubpacketTag::AttestedCertifications,
            0| 1| 8| 13| 14| 15| 17| 18| 19 => SubpacketTag::Reserved(u),
            100..=110 => SubpacketTag::Private(u),
            _ => SubpacketTag::Unknown(u),
        }
    }
}

impl From<SubpacketTag> for u8 {
    fn from(t: SubpacketTag) -> Self {
        match t {
            SubpacketTag::SignatureCreationTime => 2,
            SubpacketTag::SignatureExpirationTime => 3,
            SubpacketTag::ExportableCertification => 4,
            SubpacketTag::TrustSignature => 5,
            SubpacketTag::RegularExpression => 6,
            SubpacketTag::Revocable => 7,
            SubpacketTag::KeyExpirationTime => 9,
            SubpacketTag::PlaceholderForBackwardCompatibility => 10,
            SubpacketTag::PreferredSymmetricAlgorithms => 11,
            SubpacketTag::RevocationKey => 12,
            SubpacketTag::Issuer => 16,
            SubpacketTag::NotationData => 20,
            SubpacketTag::PreferredHashAlgorithms => 21,
            SubpacketTag::PreferredCompressionAlgorithms => 22,
            SubpacketTag::KeyServerPreferences => 23,
            SubpacketTag::PreferredKeyServer => 24,
            SubpacketTag::PrimaryUserID => 25,
            SubpacketTag::PolicyURI => 26,
            SubpacketTag::KeyFlags => 27,
            SubpacketTag::SignersUserID => 28,
            SubpacketTag::ReasonForRevocation => 29,
            SubpacketTag::Features => 30,
            SubpacketTag::SignatureTarget => 31,
            SubpacketTag::EmbeddedSignature => 32,
            SubpacketTag::IssuerFingerprint => 33,
            SubpacketTag::PreferredAEADAlgorithms => 34,
            SubpacketTag::IntendedRecipient => 35,
            SubpacketTag::AttestedCertifications => 37,
            SubpacketTag::Reserved(u) => u,
            SubpacketTag::Private(u) => u,
            SubpacketTag::Unknown(u) => u,
        }
    }
}

const SUBPACKET_TAG_VARIANTS: [SubpacketTag; 28] = [
    SubpacketTag::SignatureCreationTime,
    SubpacketTag::SignatureExpirationTime,
    SubpacketTag::ExportableCertification,
    SubpacketTag::TrustSignature,
    SubpacketTag::RegularExpression,
    SubpacketTag::Revocable,
    SubpacketTag::KeyExpirationTime,
    SubpacketTag::PlaceholderForBackwardCompatibility,
    SubpacketTag::PreferredSymmetricAlgorithms,
    SubpacketTag::RevocationKey,
    SubpacketTag::Issuer,
    SubpacketTag::NotationData,
    SubpacketTag::PreferredHashAlgorithms,
    SubpacketTag::PreferredCompressionAlgorithms,
    SubpacketTag::KeyServerPreferences,
    SubpacketTag::PreferredKeyServer,
    SubpacketTag::PrimaryUserID,
    SubpacketTag::PolicyURI,
    SubpacketTag::KeyFlags,
    SubpacketTag::SignersUserID,
    SubpacketTag::ReasonForRevocation,
    SubpacketTag::Features,
    SubpacketTag::SignatureTarget,
    SubpacketTag::EmbeddedSignature,
    SubpacketTag::IssuerFingerprint,
    SubpacketTag::PreferredAEADAlgorithms,
    SubpacketTag::IntendedRecipient,
    SubpacketTag::AttestedCertifications,
];

impl SubpacketTag {
    /// Returns an iterator over all valid variants.
    ///
    /// Returns an iterator over all known variants.  This does not
    /// include the [`SubpacketTag::Reserved`],
    /// [`SubpacketTag::Private`], or [`SubpacketTag::Unknown`]
    /// variants.
    pub fn variants() -> impl Iterator<Item=Self> {
        SUBPACKET_TAG_VARIANTS.iter().cloned()
    }
}

#[cfg(test)]
impl Arbitrary for SubpacketTag {
    fn arbitrary(g: &mut Gen) -> Self {
        u8::arbitrary(g).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    quickcheck! {
        fn roundtrip(tag: SubpacketTag) -> bool {
            let val: u8 = tag.into();
            tag == SubpacketTag::from(val)
        }
    }

    quickcheck! {
        fn parse(tag: SubpacketTag) -> bool {
            match tag {
                SubpacketTag::Reserved(u) =>
                    (u == 0 || u == 1 || u == 8
                     || u == 13 || u == 14 || u == 15
                     || u == 17 || u == 18 || u == 19),
                SubpacketTag::Private(u) => (100..=110).contains(&u),
                SubpacketTag::Unknown(u) => (u > 33 && u < 100) || u > 110,
                _ => true
            }
        }
    }

    #[test]
    fn subpacket_tag_variants() {
        use std::collections::HashSet;
        use std::iter::FromIterator;

        // SUBPACKET_TAG_VARIANTS is a list.  Derive it in a different way
        // to double check that nothing is missing.
        let derived_variants = (0..=u8::MAX)
            .map(SubpacketTag::from)
            .filter(|t| {
                match t {
                    SubpacketTag::Reserved(_) => false,
                    SubpacketTag::Private(_) => false,
                    SubpacketTag::Unknown(_) => false,
                    _ => true,
                }
            })
            .collect::<HashSet<_>>();

        let known_variants
            = HashSet::from_iter(SUBPACKET_TAG_VARIANTS.iter().cloned());

        let missing = known_variants
            .symmetric_difference(&derived_variants)
            .collect::<Vec<_>>();

        assert!(missing.is_empty(), "{:?}", missing);
    }
}

/// Subpacket area.
///
/// A version 4 Signature contains two areas that can stored
/// [signature subpackets]: a so-called hashed subpacket area, and a
/// so-called unhashed subpacket area.  The hashed subpacket area is
/// protected by the signature; the unhashed area is not.  This makes
/// the unhashed subpacket area only appropriate for
/// self-authenticating data, like the [`Issuer`] subpacket.  The
/// [`SubpacketAreas`] data structure understands these nuances and
/// routes lookups appropriately.  As such, it is usually better to
/// work with subpackets using that interface.
///
/// [signature subpackets]: https://tools.ietf.org/html/rfc4880#section-5.2.3.1
/// [`Issuer`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
///
/// # Examples
///
/// ```
/// # use sequoia_openpgp as openpgp;
/// # use openpgp::cert::prelude::*;
/// # use openpgp::packet::prelude::*;
/// # use openpgp::policy::StandardPolicy;
/// # use openpgp::types::SignatureType;
/// #
/// # fn main() -> openpgp::Result<()> {
/// # let p = &StandardPolicy::new();
/// #
/// # let (cert, _) = CertBuilder::new().generate()?;
/// #
/// # let key : &Key<_, _> = cert.primary_key().key();
/// # let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
/// #
/// # let msg = b"Hello, world!";
/// # let mut sig = SignatureBuilder::new(SignatureType::Binary)
/// #     .sign_message(&mut signer, msg)?;
/// #
/// # // Verify it.
/// # sig.verify_message(signer.public(), msg)?;
/// fn sig_stats(sig: &Signature) {
///     eprintln!("Hashed subpacket area has {} subpackets",
///               sig.hashed_area().iter().count());
///     eprintln!("Unhashed subpacket area has {} subpackets",
///               sig.unhashed_area().iter().count());
/// }
/// # sig_stats(&sig);
/// # Ok(())
/// # }
/// ```
pub struct SubpacketArea {
    /// The subpackets.
    packets: Vec<Subpacket>,

    // The subpacket area, but parsed so that the map is indexed by
    // the subpacket tag, and the value corresponds to the *last*
    // occurrence of that subpacket in the subpacket area.
    //
    // Since self-referential structs are a no-no, we use an index
    // to reference the content in the area.
    //
    // This is an option, because we parse the subpacket area lazily.
    parsed: Mutex<RefCell<Option<HashMap<SubpacketTag, usize>>>>,
}
assert_send_and_sync!(SubpacketArea);

#[cfg(test)]
impl ArbitraryBounded for SubpacketArea {
    fn arbitrary_bounded(g: &mut Gen, depth: usize) -> Self {
        use crate::arbitrary_helper::gen_arbitrary_from_range;

        let mut a = Self::default();
        for _ in 0..gen_arbitrary_from_range(0..32, g) {
            let _ = a.add(ArbitraryBounded::arbitrary_bounded(g, depth));
        }

        a
    }
}

#[cfg(test)]
impl_arbitrary_with_bound!(SubpacketArea);

impl Default for SubpacketArea {
    fn default() -> Self {
        Self::new(Default::default()).unwrap()
    }
}

impl Clone for SubpacketArea {
    fn clone(&self) -> Self {
        Self::new(self.packets.clone()).unwrap()
    }
}

impl PartialEq for SubpacketArea {
    fn eq(&self, other: &SubpacketArea) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for SubpacketArea {}

impl PartialOrd for SubpacketArea {
    fn partial_cmp(&self, other: &SubpacketArea) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SubpacketArea {
    fn cmp(&self, other: &SubpacketArea) -> Ordering {
        self.packets.cmp(&other.packets)
    }
}

impl Hash for SubpacketArea {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // We hash only the data, the cache is a hashmap and does not
        // implement hash.
        self.packets.hash(state);
    }
}

impl fmt::Debug for SubpacketArea {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list()
            .entries(self.iter())
            .finish()
    }
}

impl<'a> IntoIterator for &'a SubpacketArea {
    type Item = &'a Subpacket;
    type IntoIter = std::slice::Iter<'a, Subpacket>;

    fn into_iter(self) -> Self::IntoIter {
        self.packets.iter()
    }
}

impl SubpacketArea {
    /// The maximum size of a subpacket area.
    pub const MAX_SIZE: usize = (1 << 16) - 1;

    /// Returns a new subpacket area containing the given `packets`.
    pub fn new(packets: Vec<Subpacket>) -> Result<SubpacketArea> {
        let area = SubpacketArea {
            packets,
            parsed: Mutex::new(RefCell::new(None)),
        };
        if area.serialized_len() > std::u16::MAX as usize {
            Err(Error::InvalidArgument(
                format!("Subpacket area exceeds maximum size: {}",
                        area.serialized_len())).into())
        } else {
            Ok(area)
        }
    }

    // Initialize `Signature::hashed_area_parsed` from
    // `Signature::hashed_area`, if necessary.
    fn cache_init(&self) {
        if self.parsed.lock().unwrap().borrow().is_none() {
            let mut hash = HashMap::new();
            for (i, sp) in self.packets.iter().enumerate() {
                hash.insert(sp.tag(), i);
            }

            *self.parsed.lock().unwrap().borrow_mut() = Some(hash);
        }
    }

    /// Invalidates the cache.
    fn cache_invalidate(&self) {
        *self.parsed.lock().unwrap().borrow_mut() = None;
    }

    /// Iterates over the subpackets.
    ///
    /// # Examples
    ///
    /// Print the number of different types of subpackets in a
    /// Signature's hashed subpacket area:
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::prelude::*;
    /// # use openpgp::types::SignatureType;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) = CertBuilder::new().generate()?;
    /// #
    /// # let key : &Key<_, _> = cert.primary_key().key();
    /// # let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
    /// #
    /// # let msg = b"Hello, world!";
    /// # let mut sig = SignatureBuilder::new(SignatureType::Binary)
    /// #     .sign_message(&mut signer, msg)?;
    /// #
    /// # // Verify it.
    /// # sig.verify_message(signer.public(), msg)?;
    /// #
    /// let mut tags: Vec<_> = sig.hashed_area().iter().map(|sb| {
    ///     sb.tag()
    /// }).collect();
    /// tags.sort();
    /// tags.dedup();
    ///
    /// eprintln!("The hashed area contains {} types of subpackets",
    ///           tags.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &Subpacket> + Send + Sync {
        self.packets.iter()
    }

    pub(crate) fn iter_mut(&mut self)
                           -> impl Iterator<Item = &mut Subpacket> + Send + Sync
    {
        self.packets.iter_mut()
    }

    /// Returns a reference to the *last* instance of the specified
    /// subpacket, if any.
    ///
    /// A given subpacket may occur multiple times.  For some, like
    /// the [`Notation Data`] subpacket, this is reasonable.  For
    /// others, like the [`Signature Creation Time`] subpacket, this
    /// results in an ambiguity.  [Section 5.2.4.1 of RFC 4880] says:
    ///
    /// > a signature may contain multiple copies of a preference or
    /// > multiple expiration times.  In most cases, an implementation
    /// > SHOULD use the last subpacket in the signature, but MAY use
    /// > any conflict resolution scheme that makes more sense.
    ///
    ///   [`Notation Data`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    ///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///   [Section 5.2.4.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.4.1
    ///
    /// This function implements the recommended strategy of returning
    /// the last subpacket.
    ///
    /// # Examples
    ///
    /// All signatures must have a `Signature Creation Time` subpacket
    /// in the hashed subpacket area:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::prelude::*;
    /// use openpgp::packet::signature::subpacket::SubpacketTag;
    /// # use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) = CertBuilder::new().generate()?;
    /// #
    /// # let key : &Key<_, _> = cert.primary_key().key();
    /// # let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
    /// #
    /// # let msg = b"Hello, world!";
    /// # let mut sig = SignatureBuilder::new(SignatureType::Binary)
    /// #     .sign_message(&mut signer, msg)?;
    /// #
    /// # // Verify it.
    /// # sig.verify_message(signer.public(), msg)?;
    /// #
    /// if sig.hashed_area().subpacket(SubpacketTag::SignatureCreationTime).is_none() {
    ///     eprintln!("Invalid signature.");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn subpacket(&self, tag: SubpacketTag) -> Option<&Subpacket> {
        self.cache_init();

        match self.parsed.lock().unwrap().borrow().as_ref().unwrap().get(&tag) {
            Some(&n) => Some(&self.packets[n]),
            None => None,
        }
    }

    /// Returns a mutable reference to the *last* instance of the
    /// specified subpacket, if any.
    ///
    /// A given subpacket may occur multiple times.  For some, like
    /// the [`Notation Data`] subpacket, this is reasonable.  For
    /// others, like the [`Signature Creation Time`] subpacket, this
    /// results in an ambiguity.  [Section 5.2.4.1 of RFC 4880] says:
    ///
    /// > a signature may contain multiple copies of a preference or
    /// > multiple expiration times.  In most cases, an implementation
    /// > SHOULD use the last subpacket in the signature, but MAY use
    /// > any conflict resolution scheme that makes more sense.
    ///
    ///   [`Notation Data`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    ///   [`Signature Creation Time`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///   [Section 5.2.4.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.4.1
    ///
    /// This function implements the recommended strategy of returning
    /// the last subpacket.
    ///
    /// # Examples
    ///
    /// All signatures must have a `Signature Creation Time` subpacket
    /// in the hashed subpacket area:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::prelude::*;
    /// use openpgp::packet::signature::subpacket::SubpacketTag;
    /// # use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) = CertBuilder::new().generate()?;
    /// #
    /// # let key : &Key<_, _> = cert.primary_key().key();
    /// # let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
    /// #
    /// # let msg = b"Hello, world!";
    /// # let mut sig = SignatureBuilder::new(SignatureType::Binary)
    /// #     .sign_message(&mut signer, msg)?;
    /// #
    /// # // Verify it.
    /// # sig.verify_message(signer.public(), msg)?;
    /// #
    /// if sig.hashed_area().subpacket(SubpacketTag::SignatureCreationTime).is_none() {
    ///     eprintln!("Invalid signature.");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn subpacket_mut(&mut self, tag: SubpacketTag)
                         -> Option<&mut Subpacket> {
        self.cache_init();

        match self.parsed.lock().unwrap().borrow().as_ref().unwrap().get(&tag) {
            Some(&n) => Some(&mut self.packets[n]),
            None => None,
        }
    }

    /// Returns all instances of the specified subpacket.
    ///
    /// For most subpackets, only a single instance of the subpacket
    /// makes sense.  [`SubpacketArea::subpacket`] resolves this
    /// ambiguity by returning the last instance of the request
    /// subpacket type.  But, for some subpackets, like the [`Notation
    /// Data`] subpacket, multiple instances of the subpacket are
    /// reasonable.
    ///
    /// [`SubpacketArea::subpacket`]: Self::subpacket()
    /// [`Notation Data`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    ///
    /// # Examples
    ///
    /// Count the number of `Notation Data` subpackets in the hashed
    /// subpacket area:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::prelude::*;
    /// use openpgp::packet::signature::subpacket::SubpacketTag;
    /// # use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) = CertBuilder::new().generate()?;
    /// #
    /// # let key : &Key<_, _> = cert.primary_key().key();
    /// # let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
    /// #
    /// # let msg = b"Hello, world!";
    /// # let mut sig = SignatureBuilder::new(SignatureType::Binary)
    /// #     .sign_message(&mut signer, msg)?;
    /// #
    /// # // Verify it.
    /// # sig.verify_message(signer.public(), msg)?;
    /// #
    /// eprintln!("Signature has {} notations.",
    ///           sig.hashed_area().subpackets(SubpacketTag::NotationData).count());
    /// # Ok(())
    /// # }
    /// ```
    pub fn subpackets(&self, target: SubpacketTag)
        -> impl Iterator<Item = &Subpacket> + Send + Sync
    {
        self.iter().filter(move |sp| sp.tag() == target)
    }

    pub(crate) fn subpackets_mut(&mut self, target: SubpacketTag)
        -> impl Iterator<Item = &mut Subpacket> + Send + Sync
    {
        self.iter_mut().filter(move |sp| sp.tag() == target)
    }

    /// Adds the given subpacket.
    ///
    /// Adds the given subpacket to the subpacket area.  If the
    /// subpacket area already contains subpackets with the same tag,
    /// they are left in place.  If you want to replace them, you
    /// should instead use the [`SubpacketArea::replace`] method.
    ///
    /// [`SubpacketArea::replace`]: Self::replace()
    ///
    /// # Errors
    ///
    /// Returns `Error::MalformedPacket` if adding the packet makes
    /// the subpacket area exceed the size limit.
    ///
    /// # Examples
    ///
    /// Adds an additional `Issuer` subpacket to the unhashed
    /// subpacket area.  (This is useful if the key material is
    /// associated with multiple certificates, e.g., a v4 and a v5
    /// certificate.)  Because the subpacket is added to the unhashed
    /// area, the signature remains valid.
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::KeyID;
    /// # use openpgp::packet::prelude::*;
    /// use openpgp::packet::signature::subpacket::{
    ///     Subpacket,
    ///     SubpacketTag,
    ///     SubpacketValue,
    /// };
    /// # use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) = CertBuilder::new().generate()?;
    /// #
    /// # let key : &Key<_, _> = cert.primary_key().key();
    /// # let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
    /// #
    /// # let msg = b"Hello, world!";
    /// # let mut sig = SignatureBuilder::new(SignatureType::Binary)
    /// #     .sign_message(&mut signer, msg)?;
    /// #
    /// # // Verify it.
    /// # sig.verify_message(signer.public(), msg)?;
    /// #
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::Issuer)
    /// #    .count(),
    /// #    1);
    /// let mut sig: Signature = sig;
    /// sig.unhashed_area_mut().add(
    ///     Subpacket::new(
    ///         SubpacketValue::Issuer(KeyID::from_hex("AAAA BBBB CCCC DDDD")?),
    ///         false)?);
    ///
    /// sig.verify_message(signer.public(), msg)?;
    /// # assert_eq!(sig
    /// #    .unhashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::Issuer)
    /// #    .count(),
    /// #    1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add(&mut self, mut packet: Subpacket) -> Result<()> {
        if self.serialized_len() + packet.serialized_len()
            > ::std::u16::MAX as usize
        {
            return Err(Error::MalformedPacket(
                "Subpacket area exceeds maximum size".into()).into());
        }

        self.cache_invalidate();
        packet.set_authenticated(false);
        self.packets.push(packet);
        Ok(())
    }

    /// Adds the given subpacket, replacing all other subpackets with
    /// the same tag.
    ///
    /// Adds the given subpacket to the subpacket area.  If the
    /// subpacket area already contains subpackets with the same tag,
    /// they are first removed.  If you want to preserve them, you
    /// should instead use the [`SubpacketArea::add`] method.
    ///
    /// [`SubpacketArea::add`]: Self::add()
    ///
    /// # Errors
    ///
    /// Returns `Error::MalformedPacket` if adding the packet makes
    /// the subpacket area exceed the size limit.
    ///
    /// # Examples
    ///
    /// Assuming we have a signature with an additional `Issuer`
    /// subpacket in the unhashed area (see the example for
    /// [`SubpacketArea::add`], this replaces the `Issuer` subpacket
    /// in the unhashed area.  Because the unhashed area is not
    /// protected by the signature, the signature remains valid:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::KeyID;
    /// # use openpgp::packet::prelude::*;
    /// use openpgp::packet::signature::subpacket::{
    ///     Subpacket,
    ///     SubpacketTag,
    ///     SubpacketValue,
    /// };
    /// # use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) = CertBuilder::new().generate()?;
    /// #
    /// # let key : &Key<_, _> = cert.primary_key().key();
    /// # let mut signer = key.clone().parts_into_secret()?.into_keypair()?;
    /// #
    /// # let msg = b"Hello, world!";
    /// # let mut sig = SignatureBuilder::new(SignatureType::Binary)
    /// #     .sign_message(&mut signer, msg)?;
    /// #
    /// # // Verify it.
    /// # sig.verify_message(signer.public(), msg)?;
    /// #
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::Issuer)
    /// #    .count(),
    /// #    1);
    /// // First, add a subpacket to the unhashed area.
    /// let mut sig: Signature = sig;
    /// sig.unhashed_area_mut().add(
    ///     Subpacket::new(
    ///         SubpacketValue::Issuer(KeyID::from_hex("DDDD CCCC BBBB AAAA")?),
    ///         false)?);
    ///
    /// // Now, replace it.
    /// sig.unhashed_area_mut().replace(
    ///     Subpacket::new(
    ///         SubpacketValue::Issuer(KeyID::from_hex("AAAA BBBB CCCC DDDD")?),
    ///     false)?);
    ///
    /// sig.verify_message(signer.public(), msg)?;
    /// # assert_eq!(sig
    /// #    .unhashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::Issuer)
    /// #    .count(),
    /// #    1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn replace(&mut self, mut packet: Subpacket) -> Result<()> {
        if self.iter().filter_map(|sp| if sp.tag() != packet.tag() {
            Some(sp.serialized_len())
        } else {
            None
        }).sum::<usize>() + packet.serialized_len() > std::u16::MAX as usize {
            return Err(Error::MalformedPacket(
                "Subpacket area exceeds maximum size".into()).into());
        }
        self.remove_all(packet.tag());
        packet.set_authenticated(false);
        self.packets.push(packet);
        Ok(())
    }

    /// Removes all subpackets with the given tag.
    pub fn remove_all(&mut self, tag: SubpacketTag) {
        self.cache_invalidate();
        self.packets.retain(|sp| sp.tag() != tag);
    }

    /// Removes all subpackets.
    pub fn clear(&mut self) {
        self.cache_invalidate();
        self.packets.clear();
    }

    /// Sorts the subpackets by subpacket tag.
    ///
    /// This normalizes the subpacket area, and accelerates lookups in
    /// implementations that sort the in-core representation and use
    /// binary search for lookups.
    ///
    /// The subpackets are sorted by the numeric value of their tag.
    /// The sort is stable.  So, if there are multiple [`Notation Data`]
    /// subpackets, for instance, they will remain in the same order.
    ///
    /// The [`SignatureBuilder`] sorts the subpacket areas just before
    /// creating the signature.
    ///
    /// [`Notation Data`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    /// [`SignatureBuilder`]: super::SignatureBuilder
    pub fn sort(&mut self) {
        self.cache_invalidate();
        // slice::sort_by is stable.
        self.packets.sort_by(|a, b| u8::from(a.tag()).cmp(&b.tag().into()));
    }
}

/// Payload of a Notation Data subpacket.
///
/// The [`Notation Data`] subpacket provides a mechanism for a
/// message's signer to insert nearly arbitrary data into the
/// signature.  Because notations can be marked as critical, it is
/// possible to add security relevant notations, which the receiving
/// OpenPGP implementation will respect (in the sense that an
/// implementation will reject signatures that include unknown,
/// critical notations), even if they don't understand the notations
/// themselves.
///
///   [`Notation Data`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
///
/// It is possible to control how Sequoia's higher-level functionality
/// handles unknown, critical notations using a [`Policy`] object.
/// Depending on the degree of control required, it may be sufficient
/// to customize a [`StandardPolicy`] object using, for instance, the
/// [`StandardPolicy::good_critical_notations`] method.
///
/// [`Policy`]: crate::policy::Policy
/// [`StandardPolicy`]: crate::policy::StandardPolicy
/// [`StandardPolicy::good_critical_notations`]: crate::policy::StandardPolicy::good_critical_notations()
///
/// Notation names are human-readable UTF-8 strings.  There are two
/// namespaces: The user namespace and the IETF namespace.  Names in
/// the user namespace have the form `name@example.org` and are
/// managed by the owner of the domain.  Names in the IETF namespace
/// may not contain an `@` and are managed by IANA.  See [Section
/// 5.2.3.16 of RFC 4880] for details.
///
///   [Section 5.2.3.16 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NotationData {
    flags: NotationDataFlags,
    name: String,
    value: Vec<u8>,
}
assert_send_and_sync!(NotationData);

impl fmt::Display for NotationData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)?;

        let flags = format!("{:?}", self.flags);
        if ! flags.is_empty() {
            write!(f, " ({})", flags)?;
        }

        if self.flags.human_readable() {
            write!(f, ": {}", String::from_utf8_lossy(&self.value))?;
        } else {
            let hex = crate::fmt::hex::encode(&self.value);
            write!(f, ": {}", hex)?;
        }

        Ok(())
    }
}

impl fmt::Debug for NotationData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut dbg = f.debug_struct("NotationData");
        dbg.field("name", &self.name);

        let flags = format!("{:?}", self.flags);
        if ! flags.is_empty() {
            dbg.field("flags", &flags);
        }

        if self.flags.human_readable() {
            match std::str::from_utf8(&self.value) {
                Ok(s) => {
                    dbg.field("value", &s);
                },
                Err(e) => {
                    let s = format!("({}): {}", e,
                                    crate::fmt::hex::encode(&self.value));
                    dbg.field("value", &s);
                },
            }
        } else {
            let hex = crate::fmt::hex::encode(&self.value);
            dbg.field("value", &hex);
        }

        dbg.finish()
    }
}

#[cfg(test)]
impl Arbitrary for NotationData {
    fn arbitrary(g: &mut Gen) -> Self {
        NotationData {
            flags: Arbitrary::arbitrary(g),
            name: Arbitrary::arbitrary(g),
            value: Arbitrary::arbitrary(g),
        }
    }
}

impl NotationData {
    /// Creates a new Notation Data subpacket payload.
    pub fn new<N, V, F>(name: N, value: V, flags: F) -> Self
        where N: AsRef<str>,
              V: AsRef<[u8]>,
              F: Into<Option<NotationDataFlags>>,
    {
        Self {
            flags: flags.into().unwrap_or_else(NotationDataFlags::empty),
            name: name.as_ref().into(),
            value: value.as_ref().into(),
        }
    }

    /// Returns the flags.
    pub fn flags(&self) -> &NotationDataFlags {
        &self.flags
    }

    /// Returns the name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the value.
    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

/// Flags for the Notation Data subpacket.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NotationDataFlags(crate::types::Bitfield);
assert_send_and_sync!(NotationDataFlags);

#[cfg(test)]
impl Arbitrary for NotationDataFlags {
    fn arbitrary(g: &mut Gen) -> Self {
        NotationDataFlags(vec![u8::arbitrary(g), u8::arbitrary(g),
                               u8::arbitrary(g), u8::arbitrary(g)].into())
    }
}

impl fmt::Debug for NotationDataFlags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut need_comma = false;
        if self.human_readable() {
            f.write_str("human readable")?;
            need_comma = true;
        }

        for i in self.0.iter() {
            match i {
                NOTATION_DATA_FLAG_HUMAN_READABLE => (),
                i => {
                    if need_comma { f.write_str(", ")?; }
                    write!(f, "#{}", i)?;
                    need_comma = true;
                },
            }
        }

        // Don't mention padding, the bit field always has the same
        // size.

        Ok(())
    }
}

const NOTATION_DATA_FLAG_HUMAN_READABLE: usize = 7;

impl NotationDataFlags {
    /// Creates a new instance from `bits`.
    pub fn new<B: AsRef<[u8]>>(bits: B) -> Result<Self> {
        if bits.as_ref().len() == 4 {
            Ok(Self(bits.as_ref().to_vec().into()))
        } else {
            Err(Error::InvalidArgument(
                format!("Need four bytes of flags, got: {:?}", bits.as_ref()))
                .into())
        }
    }

    /// Returns an empty key server preference set.
    pub fn empty() -> Self {
        Self::new(&[0, 0, 0, 0]).unwrap()
    }

    /// Returns a slice containing the raw values.
    pub(crate) fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Returns whether the specified notation data flag is set.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// // Notation Data flags 0 and 2.
    /// let ndf = NotationDataFlags::new(&[5, 0, 0, 0])?;
    ///
    /// assert!(ndf.get(0));
    /// assert!(! ndf.get(1));
    /// assert!(ndf.get(2));
    /// assert!(! ndf.get(3));
    /// assert!(! ndf.get(8));
    /// assert!(! ndf.get(80));
    /// # assert!(! ndf.human_readable());
    /// # Ok(()) }
    /// ```
    pub fn get(&self, bit: usize) -> bool {
        self.0.get(bit)
    }

    /// Sets the specified notation data flag.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let ndf = NotationDataFlags::empty().set(0)?.set(2)?;
    ///
    /// assert!(ndf.get(0));
    /// assert!(! ndf.get(1));
    /// assert!(ndf.get(2));
    /// assert!(! ndf.get(3));
    /// # assert!(! ndf.human_readable());
    /// # Ok(()) }
    /// ```
    pub fn set(mut self, bit: usize) -> Result<Self> {
        assert_eq!(self.0.raw.len(), 4);
        let byte = bit / 8;
        if byte < 4 {
            self.0.raw[byte] |= 1 << (bit % 8);
            Ok(self)
        } else {
            Err(Error::InvalidArgument(
                format!("flag index out of bounds: {}", bit)).into())
        }
    }

    /// Clears the specified notation data flag.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let ndf = NotationDataFlags::empty().set(0)?.set(2)?.clear(2)?;
    ///
    /// assert!(ndf.get(0));
    /// assert!(! ndf.get(1));
    /// assert!(! ndf.get(2));
    /// assert!(! ndf.get(3));
    /// # assert!(! ndf.human_readable());
    /// # Ok(()) }
    /// ```
    pub fn clear(mut self, bit: usize) -> Result<Self> {
        assert_eq!(self.0.raw.len(), 4);
        let byte = bit / 8;
        if byte < 4 {
            self.0.raw[byte] &= !(1 << (bit % 8));
            Ok(self)
        } else {
            Err(Error::InvalidArgument(
                format!("flag index out of bounds: {}", bit)).into())
        }
    }

    /// Returns whether the value is human-readable.
    pub fn human_readable(&self) -> bool {
        self.get(NOTATION_DATA_FLAG_HUMAN_READABLE)
    }

    /// Asserts that the value is human-readable.
    pub fn set_human_readable(self) -> Self {
        self.set(NOTATION_DATA_FLAG_HUMAN_READABLE).unwrap()
    }

    /// Clear the assertion that the value is human-readable.
    pub fn clear_human_readable(self) -> Self {
        self.clear(NOTATION_DATA_FLAG_HUMAN_READABLE).unwrap()
    }
}

/// Holds an arbitrary, well-structured subpacket.
///
/// The `SubpacketValue` enum holds a [`Subpacket`]'s value.  The
/// values are well structured in the sense that they have been parsed
/// into Sequoia's native data types rather than just holding the raw
/// byte vector.  For instance, the [`Issuer`] variant holds a
/// [`KeyID`].
///
/// [`Issuer`]: SubpacketValue::Issuer
/// [`KeyID`]: super::super::super::KeyID
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub enum SubpacketValue {
    /// An unknown subpacket.
    Unknown {
        /// The unknown subpacket's tag.
        tag: SubpacketTag,
        /// The unknown subpacket's uninterpreted body.
        body: Vec<u8>
    },

    /// The time the signature was made.
    ///
    /// See [Section 5.2.3.4 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.4 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    SignatureCreationTime(Timestamp),
    /// The validity period of the signature.
    ///
    /// The validity is relative to the time stored in the signature's
    /// Signature Creation Time subpacket.
    ///
    /// See [Section 5.2.3.10 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.10 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.10
    SignatureExpirationTime(Duration),
    /// Whether a signature should be published.
    ///
    /// See [Section 5.2.3.11 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.11 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.11
    ExportableCertification(bool),
    /// Signer asserts that the key is not only valid but also trustworthy at
    /// the specified level.
    ///
    /// See [Section 5.2.3.13 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.13 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.13
    TrustSignature {
        /// Trust level, or depth.
        ///
        /// Level 0 has the same meaning as an ordinary validity
        /// signature.  Level 1 means that the signed key is asserted
        /// to be a valid trusted introducer, with the 2nd octet of
        /// the body specifying the degree of trust.  Level 2 means
        /// that the signed key is asserted to be trusted to issue
        /// level 1 trust signatures, i.e., that it is a "meta
        /// introducer".
        level: u8,

        /// Trust amount.
        ///
        /// This is interpreted such that values less than 120
        /// indicate partial trust and values of 120 or greater
        /// indicate complete trust.  Implementations SHOULD emit
        /// values of 60 for partial trust and 120 for complete trust.
        trust: u8,
    },
    /// Used in conjunction with Trust Signature packets (of level > 0) to
    /// limit the scope of trust that is extended.
    ///
    /// See [Section 5.2.3.14 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.14 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.14
    ///
    /// Note: The RFC requires that the serialized form includes a
    /// trailing NUL byte.  When Sequoia parses the regular expression
    /// subpacket, it strips the trailing NUL.  (If it doesn't include
    /// a NUL, then parsing fails.)  Likewise, when it serializes a
    /// regular expression subpacket, it unconditionally adds a NUL.
    RegularExpression(Vec<u8>),
    /// Whether a signature can later be revoked.
    ///
    /// See [Section 5.2.3.12 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.12 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.12
    Revocable(bool),
    /// The validity period of the key.
    ///
    /// The validity period is relative to the key's (not the signature's) creation time.
    ///
    /// See [Section 5.2.3.6 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.6 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    KeyExpirationTime(Duration),
    /// The Symmetric algorithms that the certificate holder prefers.
    ///
    /// See [Section 5.2.3.7 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.7 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.7
    PreferredSymmetricAlgorithms(Vec<SymmetricAlgorithm>),
    /// Authorizes the specified key to issue revocation signatures for this
    /// certificate.
    ///
    /// See [Section 5.2.3.15 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.15 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.15
    RevocationKey(RevocationKey),
    /// The OpenPGP Key ID of the key issuing the signature.
    ///
    /// See [Section 5.2.3.5 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.5 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    Issuer(KeyID),
    /// A "notation" on the signature.
    ///
    /// See [Section 5.2.3.16 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.16 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    NotationData(NotationData),
    /// The Hash algorithms that the certificate holder prefers.
    ///
    /// See [Section 5.2.3.8 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.8 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.8
    PreferredHashAlgorithms(Vec<HashAlgorithm>),
    /// The compression algorithms that the certificate holder prefers.
    ///
    /// See [Section 5.2.3.9 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.9 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.9
    PreferredCompressionAlgorithms(Vec<CompressionAlgorithm>),
    /// A list of flags that indicate preferences that the certificate
    /// holder has about how the key is handled by a key server.
    ///
    /// See [Section 5.2.3.17 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.17 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.17
    KeyServerPreferences(KeyServerPreferences),
    /// The URI of a key server where the certificate holder keeps
    /// their certificate up to date.
    ///
    /// See [Section 5.2.3.18 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.18 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.18
    PreferredKeyServer(Vec<u8>),
    /// A flag in a User ID's self-signature that states whether this
    /// User ID is the primary User ID for this certificate.
    ///
    /// See [Section 5.2.3.19 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.19 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.19
    PrimaryUserID(bool),
    /// The URI of a document that describes the policy under which
    /// the signature was issued.
    ///
    /// See [Section 5.2.3.20 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.20 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.20
    PolicyURI(Vec<u8>),
    /// A list of flags that hold information about a key.
    ///
    /// See [Section 5.2.3.21 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.21 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.21
    KeyFlags(KeyFlags),
    /// The User ID that is responsible for the signature.
    ///
    /// See [Section 5.2.3.22 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.22 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.22
    SignersUserID(Vec<u8>),
    /// The reason for a revocation, used in key revocations and
    /// certification revocation signatures.
    ///
    /// See [Section 5.2.3.23 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.23 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.23
    ReasonForRevocation {
        /// Machine-readable reason for revocation.
        code: ReasonForRevocation,

        /// Human-readable reason for revocation.
        reason: Vec<u8>,
    },
    /// The OpenPGP features a user's implementation supports.
    ///
    /// See [Section 5.2.3.24 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.24 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.24
    Features(Features),
    /// A signature to which this signature refers.
    ///
    /// See [Section 5.2.3.25 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.25 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.25
    SignatureTarget {
        /// Public-key algorithm of the target signature.
        pk_algo: PublicKeyAlgorithm,
        /// Hash algorithm of the target signature.
        hash_algo: HashAlgorithm,
        /// Hash digest of the target signature.
        digest: Vec<u8>,
    },
    /// A complete Signature packet body.
    ///
    /// This is used to store a backsig in a subkey binding signature.
    ///
    /// See [Section 5.2.3.26 of RFC 4880] for details.
    ///
    ///  [Section 5.2.3.26 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.26
    EmbeddedSignature(Signature),
    /// The Fingerprint of the key that issued the signature (proposed).
    ///
    /// See [Section 5.2.3.28 of RFC 4880bis] for details.
    ///
    ///  [Section 5.2.3.28 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    IssuerFingerprint(Fingerprint),
    /// The AEAD algorithms that the certificate holder prefers (proposed).
    ///
    /// See [Section 5.2.3.8 of RFC 4880bis] for details.
    ///
    ///  [Section 5.2.3.8 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.8
    PreferredAEADAlgorithms(Vec<AEADAlgorithm>),
    /// Who the signed message was intended for (proposed).
    ///
    /// See [Section 5.2.3.29 of RFC 4880bis] for details.
    ///
    ///  [Section 5.2.3.29 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.29
    IntendedRecipient(Fingerprint),
    /// The Attested Certifications subpacket (proposed).
    ///
    /// Allows the certificate holder to attest to third party
    /// certifications, allowing them to be distributed with the
    /// certificate.  This can be used to address certificate flooding
    /// concerns.
    ///
    /// See [Section 5.2.3.30 of RFC 4880bis] for details.
    ///
    ///  [Section 5.2.3.30 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-10.html#section-5.2.3.30
    AttestedCertifications(Vec<Box<[u8]>>),
}
assert_send_and_sync!(SubpacketValue);

#[cfg(test)]
impl ArbitraryBounded for SubpacketValue {
    fn arbitrary_bounded(g: &mut Gen, depth: usize) -> Self {
        use self::SubpacketValue::*;
        use crate::arbitrary_helper::gen_arbitrary_from_range;

        loop {
            break match gen_arbitrary_from_range(0..26, g) {
                0 => SignatureCreationTime(Arbitrary::arbitrary(g)),
                1 => SignatureExpirationTime(Arbitrary::arbitrary(g)),
                2 => ExportableCertification(Arbitrary::arbitrary(g)),
                3 => TrustSignature {
                    level: Arbitrary::arbitrary(g),
                    trust: Arbitrary::arbitrary(g),
                },
                4 => RegularExpression(Arbitrary::arbitrary(g)),
                5 => Revocable(Arbitrary::arbitrary(g)),
                6 => KeyExpirationTime(Arbitrary::arbitrary(g)),
                7 => PreferredSymmetricAlgorithms(Arbitrary::arbitrary(g)),
                8 => RevocationKey(Arbitrary::arbitrary(g)),
                9 => Issuer(Arbitrary::arbitrary(g)),
                10 => NotationData(Arbitrary::arbitrary(g)),
                11 => PreferredHashAlgorithms(Arbitrary::arbitrary(g)),
                12 => PreferredCompressionAlgorithms(Arbitrary::arbitrary(g)),
                13 => KeyServerPreferences(Arbitrary::arbitrary(g)),
                14 => PreferredKeyServer(Arbitrary::arbitrary(g)),
                15 => PrimaryUserID(Arbitrary::arbitrary(g)),
                16 => PolicyURI(Arbitrary::arbitrary(g)),
                17 => KeyFlags(Arbitrary::arbitrary(g)),
                18 => SignersUserID(Arbitrary::arbitrary(g)),
                19 => ReasonForRevocation {
                    code: Arbitrary::arbitrary(g),
                    reason: Arbitrary::arbitrary(g),
                },
                20 => Features(Arbitrary::arbitrary(g)),
                21 => SignatureTarget {
                    pk_algo: Arbitrary::arbitrary(g),
                    hash_algo: Arbitrary::arbitrary(g),
                    digest: Arbitrary::arbitrary(g),
                },
                22 if depth == 0 => continue, // Don't recurse, try again.
                22 => EmbeddedSignature(
                    ArbitraryBounded::arbitrary_bounded(g, depth - 1)),
                23 => IssuerFingerprint(Arbitrary::arbitrary(g)),
                24 => PreferredAEADAlgorithms(Arbitrary::arbitrary(g)),
                25 => IntendedRecipient(Arbitrary::arbitrary(g)),
                _ => unreachable!(),
            }
        }
    }
}

#[cfg(test)]
impl_arbitrary_with_bound!(SubpacketValue);

impl SubpacketValue {
    /// Returns the subpacket tag for this value.
    pub fn tag(&self) -> SubpacketTag {
        use self::SubpacketValue::*;
        match &self {
            SignatureCreationTime(_) => SubpacketTag::SignatureCreationTime,
            SignatureExpirationTime(_) =>
                SubpacketTag::SignatureExpirationTime,
            ExportableCertification(_) =>
                SubpacketTag::ExportableCertification,
            TrustSignature { .. } => SubpacketTag::TrustSignature,
            RegularExpression(_) => SubpacketTag::RegularExpression,
            Revocable(_) => SubpacketTag::Revocable,
            KeyExpirationTime(_) => SubpacketTag::KeyExpirationTime,
            PreferredSymmetricAlgorithms(_) =>
                SubpacketTag::PreferredSymmetricAlgorithms,
            RevocationKey { .. } => SubpacketTag::RevocationKey,
            Issuer(_) => SubpacketTag::Issuer,
            NotationData(_) => SubpacketTag::NotationData,
            PreferredHashAlgorithms(_) =>
                SubpacketTag::PreferredHashAlgorithms,
            PreferredCompressionAlgorithms(_) =>
                SubpacketTag::PreferredCompressionAlgorithms,
            KeyServerPreferences(_) => SubpacketTag::KeyServerPreferences,
            PreferredKeyServer(_) => SubpacketTag::PreferredKeyServer,
            PrimaryUserID(_) => SubpacketTag::PrimaryUserID,
            PolicyURI(_) => SubpacketTag::PolicyURI,
            KeyFlags(_) => SubpacketTag::KeyFlags,
            SignersUserID(_) => SubpacketTag::SignersUserID,
            ReasonForRevocation { .. } => SubpacketTag::ReasonForRevocation,
            Features(_) => SubpacketTag::Features,
            SignatureTarget { .. } => SubpacketTag::SignatureTarget,
            EmbeddedSignature(_) => SubpacketTag::EmbeddedSignature,
            IssuerFingerprint(_) => SubpacketTag::IssuerFingerprint,
            PreferredAEADAlgorithms(_) =>
                SubpacketTag::PreferredAEADAlgorithms,
            IntendedRecipient(_) => SubpacketTag::IntendedRecipient,
            AttestedCertifications(_) => SubpacketTag::AttestedCertifications,
            Unknown { tag, .. } => *tag,
        }
    }
}

/// Signature subpackets.
///
/// Most of a signature's attributes are not stored in fixed fields,
/// but in so-called subpackets.  These subpackets are stored in a
/// [`Signature`]'s so-called subpacket areas, which are effectively
/// small key-value stores.  The keys are subpacket tags
/// ([`SubpacketTag`]).  The values are well-structured
/// ([`SubpacketValue`]).
///
/// [`Signature`]: super::super::Signature
///
/// In addition to their key and value, subpackets also include a
/// critical flag.  When set, this flag indicates to the OpenPGP
/// implementation that if it doesn't understand the subpacket, it
/// must consider the signature to be invalid.  (Likewise, if it isn't
/// set, then it means that it is safe for the implementation to
/// ignore the subpacket.)  This enables forward compatibility with
/// security-relevant extensions.
///
/// It is possible to control how Sequoia's higher-level functionality
/// handles unknown, critical subpackets using a [`Policy`] object.
/// Depending on the degree of control required, it may be sufficient
/// to customize a [`StandardPolicy`] object using, for instance, the
/// [`StandardPolicy::accept_critical_subpacket`] method.
///
/// [`Policy`]: crate::policy::Policy
/// [`StandardPolicy`]: crate::policy::StandardPolicy
/// [`StandardPolicy::accept_critical_subpacket`]: crate::policy::StandardPolicy::accept_critical_subpacket()
///
/// The subpacket system is extensible in two ways.  First, although
/// limited, the subpacket name space is not exhausted.  So, it is
/// possible to introduce new packets.  Second, one of the subpackets,
/// the [`Notation Data`] subpacket ([`NotationData`]), is explicitly
/// designed for adding arbitrary data to signatures.
///
///   [`Notation Data`]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
///
/// Subpackets are described in [Section 5.2.3.1 of RFC 4880].
///
///   [Section 5.2.3.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.1
#[derive(Clone)]
pub struct Subpacket {
    /// The length.
    ///
    /// In order not to break signatures, we need to be able to
    /// roundtrip the subpackets, perfectly reproducing all the bits.
    /// To allow for suboptimal encoding of lengths, we store the
    /// length when we parse subpackets.
    pub(crate) // For serialize/mod.rs, parse/parse.rs.
    length: SubpacketLength,
    /// Critical flag.
    critical: bool,
    /// Packet value, must match packet type.
    value: SubpacketValue,
    /// Whether or not the information in this subpacket are
    /// authenticated in the context of its signature.
    authenticated: bool,
}
assert_send_and_sync!(Subpacket);

impl PartialEq for Subpacket {
    fn eq(&self, other: &Subpacket) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Subpacket {}

impl PartialOrd for Subpacket {
    fn partial_cmp(&self, other: &Subpacket) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Subpacket {
    fn cmp(&self, other: &Subpacket) -> Ordering {
        self.length.cmp(&other.length)
            .then_with(|| self.critical.cmp(&other.critical))
            .then_with(|| self.value.cmp(&other.value))
    }
}

impl Hash for Subpacket {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.length.hash(state);
        self.critical.hash(state);
        self.value.hash(state);
    }
}

#[cfg(test)]
impl ArbitraryBounded for Subpacket {
    fn arbitrary_bounded(g: &mut Gen, depth: usize) -> Self {
        use crate::arbitrary_helper::gen_arbitrary_from_range;

        fn encode_non_optimal(length: usize) -> SubpacketLength {
            // Calculate length the same way as Subpacket::new.
            let length = 1 /* Tag */ + length as u32;

            let mut len_vec = Vec::<u8>::with_capacity(5);
            len_vec.push(0xFF);
            len_vec.extend_from_slice(&length.to_be_bytes());
            SubpacketLength::new(length, Some(len_vec))
        }

        let critical = <bool>::arbitrary(g);
        let use_nonoptimal_encoding = <bool>::arbitrary(g);
        // We don't want to overrepresent large subpackets.
        let create_large_subpacket =
            gen_arbitrary_from_range(0..25, g) == 0;

        let value = if create_large_subpacket {
            // Choose a size which makes sure the subpacket length must be
            // encoded with 2 or 5 octets.
            let value_size = gen_arbitrary_from_range(7000..9000, g);
            let nd = NotationData {
                flags: Arbitrary::arbitrary(g),
                name: Arbitrary::arbitrary(g),
                value: (0..value_size)
                    .map(|_| <u8>::arbitrary(g))
                    .collect::<Vec<u8>>(),
            };
            SubpacketValue::NotationData(nd)
        } else {
            SubpacketValue::arbitrary_bounded(g, depth)
        };

        if use_nonoptimal_encoding {
            let length = encode_non_optimal(value.serialized_len());
            Subpacket::with_length(length, value, critical)
        } else {
            Subpacket::new(value, critical).unwrap()
        }
    }
}

#[cfg(test)]
impl_arbitrary_with_bound!(Subpacket);

impl fmt::Debug for Subpacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = f.debug_struct("Subpacket");
        if self.length.raw.is_some() {
            s.field("length", &self.length);
        }
        if self.critical {
            s.field("critical", &self.critical);
        }
        s.field("value", &self.value);
        s.field("authenticated", &self.authenticated);
        s.finish()
    }
}

impl Subpacket {
    /// Creates a new Subpacket.
    pub fn new(value: SubpacketValue, critical: bool)
               -> Result<Subpacket> {
        Ok(Self::with_length(
            SubpacketLength::from(1 /* Tag */ + value.serialized_len() as u32),
            value, critical))
    }

    /// Creates a new subpacket with the given length and tag.
    pub(crate) fn with_length(length: SubpacketLength,
                              value: SubpacketValue,
                              critical: bool)
                              -> Subpacket {
        Subpacket {
            length,
            critical,
            value,
            authenticated: false,
        }
    }

    /// Returns whether the critical bit is set.
    pub fn critical(&self) -> bool {
        self.critical
    }

    /// Returns the Subpacket's tag.
    pub fn tag(&self) -> SubpacketTag {
        self.value.tag()
    }

    /// Returns the Subpacket's value.
    pub fn value(&self) -> &SubpacketValue {
        &self.value
    }

    /// Returns the Subpacket's value.
    pub(crate) fn value_mut(&mut self) -> &mut SubpacketValue {
        &mut self.value
    }

    /// Returns whether the information in this subpacket has been
    /// authenticated.
    ///
    /// There are three ways a subpacket can be authenticated:
    ///
    ///   - It is in the hashed subpacket area and the signature has
    ///     been verified.
    ///   - It is in the unhashed subpacket area and the information
    ///     is self-authenticating and has been authenticated by
    ///     Sequoia.  This is can be done for issuer information and
    ///     embedded Signatures.
    ///   - The subpacket has been authenticated by the user and
    ///     marked as such using [`Subpacket::set_authenticated`].
    ///
    /// Note: The authentication is only valid in the context of the
    /// signature the subpacket is in.  If the `Subpacket` is cloned,
    /// or a `Subpacket` is added to a [`SubpacketArea`], the flag is
    /// cleared.
    ///
    ///   [`Subpacket::set_authenticated`]: Self::set_authenticated()
    pub fn authenticated(&self) -> bool {
        self.authenticated
    }

    /// Marks the information in this subpacket as authenticated or
    /// not.
    ///
    /// See [`Subpacket::authenticated`] for more information.
    ///
    ///   [`Subpacket::authenticated`]: Self::authenticated()
    pub fn set_authenticated(&mut self, authenticated: bool) -> bool {
        std::mem::replace(&mut self.authenticated, authenticated)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SubpacketLength {
    /// The length.
    pub(crate) len: u32,
    /// The length encoding used in the serialized form.
    /// If this is `None`, optimal encoding will be used.
    pub(crate) raw: Option<Vec<u8>>,
}

impl From<u32> for SubpacketLength {
    fn from(len: u32) -> Self {
        SubpacketLength {
            len, raw: None,
        }
    }
}

impl PartialEq for SubpacketLength {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for SubpacketLength {}

impl Hash for SubpacketLength {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.raw {
            Some(raw) => raw.hash(state),
            None => {
                let l = self.serialized_len();
                let mut raw = [0; 5];
                self.serialize_into(&mut raw[..l]).unwrap();
                raw[..l].hash(state);
            },
        }
    }
}

impl PartialOrd for SubpacketLength {
    fn partial_cmp(&self, other: &SubpacketLength) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SubpacketLength {
    fn cmp(&self, other: &SubpacketLength) -> Ordering {
        match (&self.raw, &other.raw) {
            (None, None) => {
                self.len.cmp(&other.len)
            },
            // Compare serialized representations if at least one is given
            (Some(self_raw), Some(other_raw)) => {
                self_raw.cmp(other_raw)
            },
            (Some(self_raw), None) => {
                let mut other_raw = [0; 5];
                other.serialize_into(&mut other_raw[..self.serialized_len()])
                    .unwrap();
                self_raw[..].cmp(&other_raw[..self.serialized_len()])
            },
            (None, Some(other_raw)) => {
                let mut self_raw = [0; 5];
                self.serialize_into(&mut self_raw[..self.serialized_len()])
                    .unwrap();
                self_raw[..self.serialized_len()].cmp(&other_raw[..])
            },
        }
    }
}

impl SubpacketLength {
    pub(crate) fn new(len: u32, raw: Option<Vec<u8>>) -> Self {
        Self { len, raw }
    }

    /// Returns the length.
    pub(crate) fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns the length of the optimal encoding of `len`.
    pub(crate) fn len_optimal_encoding(len: u32) -> usize {
        BodyLength::serialized_len(&BodyLength::Full(len))
    }
}

/// Subpacket storage.
///
/// Subpackets are stored either in a so-called hashed area or a
/// so-called unhashed area.  Packets stored in the hashed area are
/// protected by the signature's hash whereas packets stored in the
/// unhashed area are not.  Generally, two types of information are
/// stored in the unhashed area: self-authenticating data (the
/// `Issuer` subpacket, the `Issuer Fingerprint` subpacket, and the
/// `Embedded Signature` subpacket), and hints, like the features
/// subpacket.
///
/// When accessing subpackets directly via `SubpacketArea`s, the
/// subpackets are only looked up in the hashed area unless the
/// packets are self-authenticating in which case subpackets from the
/// hash area are preferred.  To return packets from a specific area,
/// use the `hashed_area` and `unhashed_area` methods to get the
/// specific methods and then use their accessors.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubpacketAreas {
    /// Subpackets that are part of the signature.
    hashed_area: SubpacketArea,
    /// Subpackets that are _not_ part of the signature.
    unhashed_area: SubpacketArea,
}
assert_send_and_sync!(SubpacketAreas);

#[cfg(test)]
impl ArbitraryBounded for SubpacketAreas {
    fn arbitrary_bounded(g: &mut Gen, depth: usize) -> Self {
        SubpacketAreas::new(ArbitraryBounded::arbitrary_bounded(g, depth),
                            ArbitraryBounded::arbitrary_bounded(g, depth))
    }
}

#[cfg(test)]
impl_arbitrary_with_bound!(SubpacketAreas);

impl SubpacketAreas {
    /// Returns a new `SubpacketAreas` object.
    pub fn new(hashed_area: SubpacketArea,
               unhashed_area: SubpacketArea) ->  Self {
        Self {
            hashed_area,
            unhashed_area,
        }
    }

    /// Gets a reference to the hashed area.
    pub fn hashed_area(&self) -> &SubpacketArea {
        &self.hashed_area
    }

    /// Gets a mutable reference to the hashed area.
    ///
    /// Note: if you modify the hashed area of a [`Signature4`], this
    /// will invalidate the signature.  Instead, you should normally
    /// convert the [`Signature4`] into a [`signature::SignatureBuilder`],
    /// modify that, and then create a new signature.
    pub fn hashed_area_mut(&mut self) -> &mut SubpacketArea {
        &mut self.hashed_area
    }

    /// Gets a reference to the unhashed area.
    pub fn unhashed_area(&self) -> &SubpacketArea {
        &self.unhashed_area
    }

    /// Gets a mutable reference to the unhashed area.
    pub fn unhashed_area_mut(&mut self) -> &mut SubpacketArea {
        &mut self.unhashed_area
    }

    /// Sorts the subpacket areas.
    ///
    /// See [`SubpacketArea::sort()`].
    ///
    pub fn sort(&mut self) {
        self.hashed_area.sort();
        self.unhashed_area.sort();
    }

    /// Returns a reference to the *last* instance of the specified
    /// subpacket, if any.
    ///
    /// This function returns the last instance of the specified
    /// subpacket in the subpacket areas in which it can occur.  Thus,
    /// when looking for the `Signature Creation Time` subpacket, this
    /// function only considers the hashed subpacket area.  But, when
    /// looking for the `Embedded Signature` subpacket, this function
    /// considers both subpacket areas.
    ///
    /// Unknown subpackets are assumed to only safely occur in the
    /// hashed subpacket area.  Thus, any instances of them in the
    /// unhashed area are ignored.
    ///
    /// For subpackets that can safely occur in both subpacket areas,
    /// this function prefers instances in the hashed subpacket area.
    pub fn subpacket(&self, tag: SubpacketTag) -> Option<&Subpacket> {
        if let Some(sb) = self.hashed_area().subpacket(tag) {
            return Some(sb);
        }

        // There are a couple of subpackets that we are willing to
        // take from the unhashed area.  The others we ignore
        // completely.
        if !(tag == SubpacketTag::Issuer
             || tag == SubpacketTag::IssuerFingerprint
             || tag == SubpacketTag::EmbeddedSignature) {
            return None;
        }

        self.unhashed_area().subpacket(tag)
    }

    /// Returns a mutable reference to the *last* instance of the
    /// specified subpacket, if any.
    ///
    /// This function returns the last instance of the specified
    /// subpacket in the subpacket areas in which it can occur.  Thus,
    /// when looking for the `Signature Creation Time` subpacket, this
    /// function only considers the hashed subpacket area.  But, when
    /// looking for the `Embedded Signature` subpacket, this function
    /// considers both subpacket areas.
    ///
    /// Unknown subpackets are assumed to only safely occur in the
    /// hashed subpacket area.  Thus, any instances of them in the
    /// unhashed area are ignored.
    ///
    /// For subpackets that can safely occur in both subpacket areas,
    /// this function prefers instances in the hashed subpacket area.
    #[allow(clippy::redundant_pattern_matching)]
    pub fn subpacket_mut(&mut self, tag: SubpacketTag)
                         -> Option<&mut Subpacket> {
        if let Some(_) = self.hashed_area().subpacket(tag) {
            return self.hashed_area_mut().subpacket_mut(tag);
        }

        // There are a couple of subpackets that we are willing to
        // take from the unhashed area.  The others we ignore
        // completely.
        if !(tag == SubpacketTag::Issuer
             || tag == SubpacketTag::IssuerFingerprint
             || tag == SubpacketTag::EmbeddedSignature) {
            return None;
        }

        self.unhashed_area_mut().subpacket_mut(tag)
    }

    /// Returns an iterator over all instances of the specified
    /// subpacket.
    ///
    /// This function returns an iterator over all instances of the
    /// specified subpacket in the subpacket areas in which it can
    /// occur.  Thus, when looking for the `Issuer` subpacket, the
    /// iterator includes instances of the subpacket from both the
    /// hashed subpacket area and the unhashed subpacket area, but
    /// when looking for the `Signature Creation Time` subpacket, the
    /// iterator only includes instances of the subpacket from the
    /// hashed subpacket area; any instances of the subpacket in the
    /// unhashed subpacket area are ignored.
    ///
    /// Unknown subpackets are assumed to only safely occur in the
    /// hashed subpacket area.  Thus, any instances of them in the
    /// unhashed area are ignored.
    pub fn subpackets(&self, tag: SubpacketTag)
        -> impl Iterator<Item = &Subpacket> + Send + Sync
    {
        // It would be nice to do:
        //
        //     let iter = self.hashed_area().subpackets(tag);
        //     if (subpacket allowed in unhashed area) {
        //         iter.chain(self.unhashed_area().subpackets(tag))
        //     } else {
        //         iter
        //     }
        //
        // but then we have different types.  Instead, we need to
        // inline SubpacketArea::subpackets, add the additional
        // constraint in the closure, and hope that the optimizer is
        // smart enough to not unnecessarily iterate over the unhashed
        // area.
        self.hashed_area().subpackets(tag).chain(
            self.unhashed_area()
                .iter()
                .filter(move |sp| {
                    (tag == SubpacketTag::Issuer
                     || tag == SubpacketTag::IssuerFingerprint
                     || tag == SubpacketTag::EmbeddedSignature)
                        && sp.tag() == tag
            }))
    }

    pub(crate) fn subpackets_mut(&mut self, tag: SubpacketTag)
        -> impl Iterator<Item = &mut Subpacket> + Send + Sync
    {
        self.hashed_area.subpackets_mut(tag).chain(
            self.unhashed_area
                .iter_mut()
                .filter(move |sp| {
                    (tag == SubpacketTag::Issuer
                     || tag == SubpacketTag::IssuerFingerprint
                     || tag == SubpacketTag::EmbeddedSignature)
                        && sp.tag() == tag
            }))
    }

    /// Returns the value of the Signature Creation Time subpacket.
    ///
    /// The [Signature Creation Time subpacket] specifies when the
    /// signature was created.  According to the standard, all
    /// signatures must include a Signature Creation Time subpacket in
    /// the signature's hashed area.  This doesn't mean that the time
    /// stamp is correct: the issuer can always forge it.
    ///
    /// [Signature Creation Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn signature_creation_time(&self) -> Option<time::SystemTime> {
        // 4-octet time field
        if let Some(sb)
                = self.subpacket(SubpacketTag::SignatureCreationTime) {
            if let SubpacketValue::SignatureCreationTime(v) = sb.value {
                Some(v.into())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Signature Expiration Time subpacket.
    ///
    /// This function is called `signature_validity_period` and not
    /// `signature_expiration_time`, which would be more consistent
    /// with the subpacket's name, because the latter suggests an
    /// absolute time, but the time is actually relative to the
    /// signature's creation time, which is stored in the signature's
    /// [Signature Creation Time subpacket].
    ///
    /// [Signature Creation Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///
    /// A [Signature Expiration Time subpacket] specifies when the
    /// signature expires.  This is different from the [Key Expiration
    /// Time subpacket], which is accessed using
    /// [`SubpacketAreas::key_validity_period`], and used to
    /// specify when an associated key expires.  The difference is
    /// that in the former case, the signature itself expires, but in
    /// the latter case, only the associated key expires.  This
    /// difference is critical: if a binding signature expires, then
    /// an OpenPGP implementation will still consider the associated
    /// key to be valid if there is another valid binding signature,
    /// even if it is older than the expired signature; if the active
    /// binding signature indicates that the key has expired, then
    /// OpenPGP implementations will not fallback to an older binding
    /// signature.
    ///
    /// [Signature Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.10
    /// [Key Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    /// [`SubpacketAreas::key_validity_period`]: SubpacketAreas::key_validity_period()
    ///
    /// There are several cases where having a signature expire is
    /// useful.  Say Alice certifies Bob's certificate for
    /// `bob@example.org`.  She can limit the lifetime of the
    /// certification to force her to reevaluate the certification
    /// shortly before it expires.  For instance, is Bob still
    /// associated with `example.org`?  Does she have reason to
    /// believe that his key has been compromised?  Using an
    /// expiration is common in the X.509 ecosystem.  For instance,
    /// [Let's Encrypt] issues certificates with 90-day lifetimes.
    ///
    /// [Let's Encrypt]: https://letsencrypt.org/2015/11/09/why-90-days.html
    ///
    /// Having signatures expire can also be useful when deploying
    /// software.  For instance, you might have a service that
    /// installs an update if it has been signed by a trusted
    /// certificate.  To prevent an adversary from coercing the
    /// service to install an older version, you could limit the
    /// signature's lifetime to just a few minutes.
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.  If this function returns `None`, or the
    /// returned period is `0`, the signature does not expire.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn signature_validity_period(&self) -> Option<time::Duration> {
        // 4-octet time field
        if let Some(sb)
                = self.subpacket(SubpacketTag::SignatureExpirationTime) {
            if let SubpacketValue::SignatureExpirationTime(v) = sb.value {
                Some(v.into())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Signature Expiration Time subpacket
    /// as an absolute time.
    ///
    /// A [Signature Expiration Time subpacket] specifies when the
    /// signature expires.  The value stored is not an absolute time,
    /// but a duration, which is relative to the Signature's creation
    /// time.  To better reflect the subpacket's name, this method
    /// returns the absolute expiry time, and the
    /// [`SubpacketAreas::signature_validity_period`] method returns
    /// the subpacket's raw value.
    ///
    /// [Signature Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.10
    /// [`SubpacketAreas::signature_validity_period`]: SubpacketAreas::signature_validity_period()
    ///
    /// The Signature Expiration Time subpacket is different from the
    /// [Key Expiration Time subpacket], which is accessed using
    /// [`SubpacketAreas::key_validity_period`], and used specifies
    /// when an associated key expires.  The difference is that in the
    /// former case, the signature itself expires, but in the latter
    /// case, only the associated key expires.  This difference is
    /// critical: if a binding signature expires, then an OpenPGP
    /// implementation will still consider the associated key to be
    /// valid if there is another valid binding signature, even if it
    /// is older than the expired signature; if the active binding
    /// signature indicates that the key has expired, then OpenPGP
    /// implementations will not fallback to an older binding
    /// signature.
    ///
    /// [Key Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    /// [`SubpacketAreas::key_validity_period`]: SubpacketAreas::key_validity_period()
    ///
    /// There are several cases where having a signature expire is
    /// useful.  Say Alice certifies Bob's certificate for
    /// `bob@example.org`.  She can limit the lifetime of the
    /// certification to force her to reevaluate the certification
    /// shortly before it expires.  For instance, is Bob still
    /// associated with `example.org`?  Does she have reason to
    /// believe that his key has been compromised?  Using an
    /// expiration is common in the X.509 ecosystem.  For instance,
    /// [Let's Encrypt] issues certificates with 90-day lifetimes.
    ///
    /// [Let's Encrypt]: https://letsencrypt.org/2015/11/09/why-90-days.html
    ///
    /// Having signatures expire can also be useful when deploying
    /// software.  For instance, you might have a service that
    /// installs an update if it has been signed by a trusted
    /// certificate.  To prevent an adversary from coercing the
    /// service to install an older version, you could limit the
    /// signature's lifetime to just a few minutes.
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.  If this function returns `None`, the
    /// signature does not expire.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn signature_expiration_time(&self) -> Option<time::SystemTime> {
        match (self.signature_creation_time(), self.signature_validity_period())
        {
            (Some(ct), Some(vp)) if vp.as_secs() > 0 => Some(ct + vp),
            _ => None,
        }
    }

    /// Returns whether or not the signature is alive at the specified
    /// time.
    ///
    /// A signature is considered to be alive if `creation time -
    /// tolerance <= time` and `time < expiration time`.
    ///
    /// This function does not check whether the key is revoked.
    ///
    /// If `time` is `None`, then this function uses the current time
    /// for `time`.
    ///
    /// If `time` is `None`, and `clock_skew_tolerance` is `None`,
    /// then this function uses [`struct@CLOCK_SKEW_TOLERANCE`] for the
    /// tolerance.  If `time` is not `None `and `clock_skew_tolerance`
    /// is `None`, it uses no tolerance.  The intuition here is that
    /// we only need a tolerance when checking if a signature is alive
    /// right now; if we are checking at a specific time, we don't
    /// want to use a tolerance.
    ///
    ///
    /// A small amount of tolerance for clock skew is necessary,
    /// because although most computers synchronize their clocks with
    /// a time server, up to a few seconds of clock skew are not
    /// unusual in practice.  And, even worse, several minutes of
    /// clock skew appear to be not uncommon on virtual machines.
    ///
    /// Not accounting for clock skew can result in signatures being
    /// unexpectedly considered invalid.  Consider: computer A sends a
    /// message to computer B at 9:00, but computer B, whose clock
    /// says the current time is 8:59, rejects it, because the
    /// signature appears to have been made in the future.  This is
    /// particularly problematic for low-latency protocols built on
    /// top of OpenPGP, e.g., when two MUAs synchronize their state
    /// via a shared IMAP folder.
    ///
    /// Being tolerant to potential clock skew is not always
    /// appropriate.  For instance, when determining a User ID's
    /// current self signature at time `t`, we don't ever want to
    /// consider a self-signature made after `t` to be valid, even if
    /// it was made just a few moments after `t`.  This goes doubly so
    /// for soft revocation certificates: the user might send a
    /// message that she is retiring, and then immediately create a
    /// soft revocation.  The soft revocation should not invalidate
    /// the message.
    ///
    /// Unfortunately, in many cases, whether we should account for
    /// clock skew or not depends on application-specific context.  As
    /// a rule of thumb, if the time and the timestamp come from
    /// different clocks, you probably want to account for clock skew.
    ///
    /// # Errors
    ///
    /// [Section 5.2.3.4 of RFC 4880] states that a Signature Creation
    /// Time subpacket "MUST be present in the hashed area."
    /// Consequently, if such a packet does not exist, this function
    /// returns [`Error::MalformedPacket`].
    ///
    ///  [Section 5.2.3.4 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///  [`Error::MalformedPacket`]: super::super::super::Error::MalformedPacket
    ///
    /// # Examples
    ///
    /// Alice's desktop computer and laptop exchange messages in real
    /// time via a shared IMAP folder.  Unfortunately, the clocks are
    /// not perfectly synchronized: the desktop computer's clock is a
    /// few seconds ahead of the laptop's clock.  When there is little
    /// or no propagation delay, this means that the laptop will
    /// consider the signatures to be invalid, because they appear to
    /// have been created in the future.  Using a tolerance prevents
    /// this from happening.
    ///
    /// ```
    /// use std::time::{SystemTime, Duration};
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::SignatureBuilder;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// let (alice, _) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///         .generate()?;
    ///
    /// // Alice's Desktop computer signs a message.  Its clock is a
    /// // few seconds fast.
    /// let now = SystemTime::now() + Duration::new(5, 0);
    ///
    /// let mut alices_signer = alice.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    /// let msg = "START PROTOCOL";
    /// let mut sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .set_signature_creation_time(now)?
    ///     .sign_message(&mut alices_signer, msg)?;
    /// # assert!(sig.verify_message(alices_signer.public(), msg).is_ok());
    ///
    /// // The desktop computer transfers the message to the laptop
    /// // via the shared IMAP folder.  Because the laptop receives a
    /// // push notification, it immediately processes it.
    /// // Unfortunately, it is considered to be invalid: the message
    /// // appears to be from the future!
    /// assert!(sig.signature_alive(None, Duration::new(0, 0)).is_err());
    ///
    /// // But, using the small default tolerance causes the laptop
    /// // to consider the signature to be alive.
    /// assert!(sig.signature_alive(None, None).is_ok());
    /// # Ok(()) }
    /// ```
    pub fn signature_alive<T, U>(&self, time: T, clock_skew_tolerance: U)
        -> Result<()>
        where T: Into<Option<time::SystemTime>>,
              U: Into<Option<time::Duration>>
    {
        let (time, tolerance)
            = match (time.into(), clock_skew_tolerance.into()) {
                (None, None) =>
                    (crate::now(),
                     *CLOCK_SKEW_TOLERANCE),
                (None, Some(tolerance)) =>
                    (crate::now(),
                     tolerance),
                (Some(time), None) =>
                    (time, time::Duration::new(0, 0)),
                (Some(time), Some(tolerance)) =>
                    (time, tolerance)
            };

        match (self.signature_creation_time(), self.signature_validity_period())
        {
            (None, _) =>
                Err(Error::MalformedPacket("no signature creation time".into())
                    .into()),
            (Some(c), Some(e)) if e.as_secs() > 0 && (c + e) <= time =>
                Err(Error::Expired(c + e).into()),
            // Be careful to avoid underflow.
            (Some(c), _) if cmp::max(c, time::UNIX_EPOCH + tolerance)
                - tolerance > time =>
                Err(Error::NotYetLive(cmp::max(c, time::UNIX_EPOCH + tolerance)
                                      - tolerance).into()),
            _ => Ok(()),
        }
    }

    /// Returns the value of the Key Expiration Time subpacket.
    ///
    /// This function is called `key_validity_period` and not
    /// `key_expiration_time`, which would be more consistent with
    /// the subpacket's name, because the latter suggests an absolute
    /// time, but the time is actually relative to the associated
    /// key's (*not* the signature's) creation time, which is stored
    /// in the [Key].
    ///
    /// [Key]: https://tools.ietf.org/html/rfc4880#section-5.5.2
    ///
    /// A [Key Expiration Time subpacket] specifies when the
    /// associated key expires.  This is different from the [Signature
    /// Expiration Time subpacket] (accessed using
    /// [`SubpacketAreas::signature_validity_period`]), which is
    /// used to specify when the signature expires.  That is, in the
    /// former case, the associated key expires, but in the latter
    /// case, the signature itself expires.  This difference is
    /// critical: if a binding signature expires, then an OpenPGP
    /// implementation will still consider the associated key to be
    /// valid if there is another valid binding signature, even if it
    /// is older than the expired signature; if the active binding
    /// signature indicates that the key has expired, then OpenPGP
    /// implementations will not fallback to an older binding
    /// signature.
    ///
    /// [Key Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    /// [Signature Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    /// [`SubpacketAreas::signature_validity_period`]: Self::signature_validity_period()
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.  If this function returns `None`, or the
    /// returned period is `0`, the key does not expire.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn key_validity_period(&self) -> Option<time::Duration> {
        // 4-octet time field
        if let Some(sb)
                = self.subpacket(SubpacketTag::KeyExpirationTime) {
            if let SubpacketValue::KeyExpirationTime(v) = sb.value {
                Some(v.into())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Key Expiration Time subpacket
    /// as an absolute time.
    ///
    /// A [Key Expiration Time subpacket] specifies when a key
    /// expires.  The value stored is not an absolute time, but a
    /// duration, which is relative to the associated [Key]'s creation
    /// time, which is stored in the Key packet, not the binding
    /// signature.  As such, the Key Expiration Time subpacket is only
    /// meaningful on a key's binding signature.  To better reflect
    /// the subpacket's name, this method returns the absolute expiry
    /// time, and the [`SubpacketAreas::key_validity_period`] method
    /// returns the subpacket's raw value.
    ///
    /// [Key Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    /// [Key]: https://tools.ietf.org/html/rfc4880#section-5.5.2
    /// [`SubpacketAreas::key_validity_period`]: Self::key_validity_period()
    ///
    /// The Key Expiration Time subpacket is different from the
    /// [Signature Expiration Time subpacket], which is accessed using
    /// [`SubpacketAreas::signature_validity_period`], and specifies
    /// when a signature expires.  The difference is that in the
    /// former case, only the associated key expires, but in the
    /// latter case, the signature itself expires.  This difference is
    /// critical: if a binding signature expires, then an OpenPGP
    /// implementation will still consider the associated key to be
    /// valid if there is another valid binding signature, even if it
    /// is older than the expired signature; if the active binding
    /// signature indicates that the key has expired, then OpenPGP
    /// implementations will not fallback to an older binding
    /// signature.
    ///
    /// [Signature Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.10
    /// [`SubpacketAreas::signature_validity_period`]: Self::signature_validity_period()
    ///
    /// Because the absolute time is relative to the key's creation
    /// time, which is stored in the key itself, this function needs
    /// the associated key.  Since there is no way to get the
    /// associated key from a signature, the key must be passed to
    /// this function.  This function does not check that the key is
    /// in fact associated with this signature.
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.  If this function returns `None`, the
    /// signature does not expire.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn key_expiration_time<P, R>(&self, key: &Key<P, R>)
                                     -> Option<time::SystemTime>
        where P: key::KeyParts,
              R: key::KeyRole,
    {
        match self.key_validity_period() {
            Some(vp) if vp.as_secs() > 0 => Some(key.creation_time() + vp),
            _ => None,
        }
    }

    /// Returns whether or not a key is alive at the specified
    /// time.
    ///
    /// A [Key] is considered to be alive if `creation time -
    /// tolerance <= time` and `time < expiration time`.
    ///
    /// [Key]: https://tools.ietf.org/html/rfc4880#section-5.5.2
    ///
    /// This function does not check whether the signature is alive
    /// (cf. [`SubpacketAreas::signature_alive`]), or whether the key
    /// is revoked (cf. [`ValidKeyAmalgamation::revoked`]).
    ///
    /// [`SubpacketAreas::signature_alive`]: Self::signature_alive()
    /// [`ValidKeyAmalgamation::revoked`]: crate::cert::amalgamation::key::ValidKeyAmalgamationIter::revoked()
    ///
    /// If `time` is `None`, then this function uses the current time
    /// for `time`.
    ///
    /// Whereas a Key's expiration time is stored in the Key's active
    /// binding signature in the [Key Expiration Time
    /// subpacket], its creation time is stored in the Key packet.  As
    /// such, the associated Key must be passed to this function.
    /// This function, however, has no way to check that the signature
    /// is actually a binding signature for the specified Key.
    ///
    /// [Key Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    ///
    /// # Examples
    ///
    /// Even keys that don't expire may not be considered alive.  This
    /// is the case if they were created after the specified time.
    ///
    /// ```
    /// use std::time::{SystemTime, Duration};
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().generate()?;
    ///
    /// let mut pk = cert.primary_key().key();
    /// let sig = cert.primary_key().with_policy(p, None)?.binding_signature();
    ///
    /// assert!(sig.key_alive(pk, None).is_ok());
    /// // A key is not considered alive prior to its creation time.
    /// let the_past = SystemTime::now() - Duration::new(300, 0);
    /// assert!(sig.key_alive(pk, the_past).is_err());
    /// # Ok(()) }
    /// ```
    pub fn key_alive<P, R, T>(&self, key: &Key<P, R>, t: T) -> Result<()>
        where P: key::KeyParts,
              R: key::KeyRole,
              T: Into<Option<time::SystemTime>>
    {
        let t = t.into().unwrap_or_else(crate::now);

        match self.key_validity_period() {
            Some(e) if e.as_secs() > 0 && key.creation_time() + e <= t =>
                Err(Error::Expired(key.creation_time() + e).into()),
            _ if key.creation_time() > t =>
                Err(Error::NotYetLive(key.creation_time()).into()),
            _ => Ok(()),
        }
    }

    /// Returns the value of the Exportable Certification subpacket.
    ///
    /// The [Exportable Certification subpacket] indicates whether the
    /// signature should be exported (e.g., published on a public key
    /// server) or not.  When using [`Serialize::export`] to export a
    /// certificate, signatures that have this subpacket present and
    /// set to false are not serialized.
    ///
    /// [Exportable Certification subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.11
    /// [`Serialize::export`]: https://docs.sequoia-pgp.org/sequoia_openpgp/serialize/trait.Serialize.html#method.export
    ///
    /// Normally, you'll want to use [`Signature4::exportable`] to
    /// check if a signature should be exported.  That function also
    /// checks whether the signature includes any sensitive
    /// [Revocation Key subpackets], which also shouldn't be exported.
    ///
    /// [`Signature4::exportable`]: super::Signature4::exportable()
    /// [Revocation Key subpackets]: https://tools.ietf.org/html/rfc4880#section-5.2.3.15
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn exportable_certification(&self) -> Option<bool> {
        // 1 octet of exportability, 0 for not, 1 for exportable
        if let Some(sb)
                = self.subpacket(SubpacketTag::ExportableCertification) {
            if let SubpacketValue::ExportableCertification(v) = sb.value {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Trust Signature subpacket.
    ///
    /// The [Trust Signature subpacket] indicates the degree to which
    /// a certificate holder is trusted to certify other keys.
    ///
    /// [Trust Signature subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.13
    ///
    /// A level of 0 means that the certificate holder is not trusted
    /// to certificate other keys, a level of 1 means that the
    /// certificate holder is a trusted introducer (a [certificate
    /// authority]) and any certifications that they make should be
    /// considered valid.  A level of 2 means the certificate holder
    /// can designate level 1 trusted introducers, etc.
    ///
    /// [certificate authority]: https://en.wikipedia.org/wiki/Certificate_authority
    ///
    /// The trust indicates the degree of confidence.  A value of 120
    /// means that a certification should be considered valid.  A
    /// value of 60 means that a certification should only be
    /// considered partially valid.  In the latter case, typically
    /// three such certifications are required for a binding to be
    /// considered authenticated.
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn trust_signature(&self) -> Option<(u8, u8)> {
        // 1 octet "level" (depth), 1 octet of trust amount
        if let Some(sb) = self.subpacket(SubpacketTag::TrustSignature) {
            if let SubpacketValue::TrustSignature{ level, trust } = sb.value {
                Some((level, trust))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the values of all Regular Expression subpackets.
    ///
    /// The [Regular Expression subpacket] is used in conjunction with
    /// a [Trust Signature subpacket], which is accessed using
    /// [`SubpacketAreas::trust_signature`], to limit the scope
    /// of a trusted introducer.  This is useful, for instance, when a
    /// company has a CA and you only want to trust them to certify
    /// their own employees.
    ///
    /// [Trust Signature subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.13
    /// [Regular Expression subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.14
    /// [`SubpacketAreas::trust_signature`]: Self::trust_signature()
    ///
    /// Note: The serialized form includes a trailing `NUL` byte.
    /// Sequoia strips the `NUL` when parsing the subpacket.
    ///
    /// This returns all instances of the Regular Expression subpacket
    /// in the hashed subpacket area.
    pub fn regular_expressions(&self) -> impl Iterator<Item=&[u8]> + Send + Sync
    {
        self.subpackets(SubpacketTag::RegularExpression).map(|sb| {
            match sb.value {
                SubpacketValue::RegularExpression(ref v) => &v[..],
                _ => unreachable!(),
            }
        })
    }

    /// Returns the value of the Revocable subpacket.
    ///
    ///
    /// The [Revocable subpacket] indicates whether a certification
    /// may be later revoked by creating a [Certification revocation
    /// signature] (0x30) that targets the signature using the
    /// [Signature Target subpacket] (accessed using the
    /// [`SubpacketAreas::signature_target`] method).
    ///
    /// [Revocable subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.12
    /// [Certification revocation signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    /// [Signature Target subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.25
    /// [`SubpacketAreas::signature_target`]: Self::signature_target()
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn revocable(&self) -> Option<bool> {
        // 1 octet of revocability, 0 for not, 1 for revocable
        if let Some(sb)
                = self.subpacket(SubpacketTag::Revocable) {
            if let SubpacketValue::Revocable(v) = sb.value {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the values of all Revocation Key subpackets.
    ///
    /// A [Revocation Key subpacket] indicates certificates (so-called
    /// designated revokers) that are allowed to revoke the signer's
    /// certificate.  For instance, if Alice trusts Bob, she can set
    /// him as a designated revoker.  This is useful if Alice loses
    /// access to her key, and therefore is unable to generate a
    /// revocation certificate on her own.  In this case, she can
    /// still Bob to generate one on her behalf.
    ///
    /// When getting a certificate's revocation keys, all valid
    /// self-signatures should be checked, not only the active
    /// self-signature.  This prevents an attacker who has gained
    /// access to the private key material from invalidating a
    /// third-party revocation by publishing a new self signature that
    /// doesn't include any revocation keys.
    ///
    /// Due to the complexity of verifying such signatures, many
    /// OpenPGP implementations do not support this feature.
    ///
    /// [Revocation Key subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.15
    ///
    /// This returns all instance of the Revocation Key subpacket in
    /// the hashed subpacket area.
    pub fn revocation_keys(&self)
                           -> impl Iterator<Item=&RevocationKey> + Send + Sync
    {
        self.subpackets(SubpacketTag::RevocationKey)
            .map(|sb| {
                match sb.value {
                    SubpacketValue::RevocationKey(ref rk) => rk,
                    _ => unreachable!(),
                }
            })
    }

    /// Returns the values of all Issuer subpackets.
    ///
    /// The [Issuer subpacket] is used when processing a signature to
    /// identify which certificate created the signature.  Since this
    /// information is self-authenticating (the act of validating the
    /// signature authenticates the subpacket), it may be stored in the
    /// unhashed subpacket area.
    ///
    ///   [Issuer subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///
    /// This returns all instances of the Issuer subpacket in both the
    /// hashed subpacket area and the unhashed subpacket area.
    pub fn issuers(&self) -> impl Iterator<Item=&KeyID> + Send + Sync {
        // 8-octet Key ID
        self.subpackets(SubpacketTag::Issuer)
            .map(|sb| {
                match sb.value {
                    SubpacketValue::Issuer(ref keyid) => keyid,
                    _ => unreachable!(),
                }
            })
    }

    /// Returns the values of all Issuer Fingerprint subpackets.
    ///
    /// The [Issuer Fingerprint subpacket] is used when processing a
    /// signature to identify which certificate created the signature.
    /// Since this information is self-authenticating (the act of
    /// validating the signature authenticates the subpacket), it is
    /// normally stored in the unhashed subpacket area.
    ///
    ///   [Issuer Fingerprint subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///
    /// This returns all instances of the Issuer Fingerprint subpacket
    /// in both the hashed subpacket area and the unhashed subpacket
    /// area.
    pub fn issuer_fingerprints(&self)
                               -> impl Iterator<Item=&Fingerprint> + Send + Sync
    {
        // 1 octet key version number, N octets of fingerprint
        self.subpackets(SubpacketTag::IssuerFingerprint)
            .map(|sb| {
                match sb.value {
                    SubpacketValue::IssuerFingerprint(ref fpr) => fpr,
                    _ => unreachable!(),
                }
            })
    }

    /// Returns all Notation Data subpackets.
    ///
    /// [Notation Data subpackets] are key-value pairs.  They can be
    /// used by applications to annotate signatures in a structured
    /// way.  For instance, they can define additional,
    /// application-specific security requirements.  Because they are
    /// functionally equivalent to subpackets, they can also be used
    /// for OpenPGP extensions.  This is how the [Intended Recipient
    /// subpacket] started life.
    ///
    /// [Notation Data subpackets]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    /// [Intended Recipient subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#name-intended-recipient-fingerpr
    ///
    /// Notation names are structured, and are divided into two
    /// namespaces: the user namespace and the IETF namespace.  Names
    /// in the user namespace have the form `name@example.org` and
    /// their meaning is defined by the owner of the domain.  The
    /// meaning of the notation `name@example.org`, for instance, is
    /// defined by whoever controls `example.org`.  Names in the IETF
    /// namespace do not contain an `@` and are managed by IANA.  See
    /// [Section 5.2.3.16 of RFC 4880] for details.
    ///
    /// [Section 5.2.3.16 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    ///
    /// This returns all instances of the Notation Data subpacket in
    /// the hashed subpacket area.
    pub fn notation_data(&self)
                         -> impl Iterator<Item=&NotationData> + Send + Sync
    {
        self.subpackets(SubpacketTag::NotationData)
            .map(|sb| {
                match sb.value {
                    SubpacketValue::NotationData(ref v) => v,
                    _ => unreachable!(),
                }
            })
    }

    /// Returns the values of all Notation Data subpackets with the
    /// given name.
    ///
    /// [Notation Data subpackets] are key-value pairs.  They can be
    /// used by applications to annotate signatures in a structured
    /// way.  For instance, they can define additional,
    /// application-specific security requirements.  Because they are
    /// functionally equivalent to subpackets, they can also be used
    /// for OpenPGP extensions.  This is how the [Intended Recipient
    /// subpacket] started life.
    ///
    /// [Notation Data subpackets]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    /// [Intended Recipient subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#name-intended-recipient-fingerpr
    ///
    /// Notation names are structured, and are divided into two
    /// namespaces: the user namespace and the IETF namespace.  Names
    /// in the user namespace have the form `name@example.org` and
    /// their meaning is defined by the owner of the domain.  The
    /// meaning of the notation `name@example.org`, for instance, is
    /// defined by whoever controls `example.org`.  Names in the IETF
    /// namespace do not contain an `@` and are managed by IANA.  See
    /// [Section 5.2.3.16 of RFC 4880] for details.
    ///
    /// [Section 5.2.3.16 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    ///
    /// This returns the values of all instances of the Notation Data
    /// subpacket with the specified name in the hashed subpacket area.
    // name needs 'a, because the closure outlives the function call.
    pub fn notation<'a, N>(&'a self, name: N)
                           -> impl Iterator<Item=&'a [u8]> + Send + Sync
        where N: 'a + AsRef<str> + Send + Sync
    {
        self.notation_data()
            .filter_map(move |n| {
                if n.name == name.as_ref() {
                    Some(&n.value[..])
                } else {
                    None
                }
            })
    }

    /// Returns the value of the Preferred Symmetric Algorithms
    /// subpacket.
    ///
    /// A [Preferred Symmetric Algorithms subpacket] lists what
    /// symmetric algorithms the user prefers.  When encrypting a
    /// message for a recipient, the OpenPGP implementation should not
    /// use an algorithm that is not on this list.
    ///
    /// [Preferred Symmetric Algorithms subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.7
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn preferred_symmetric_algorithms(&self)
                                          -> Option<&[SymmetricAlgorithm]> {
        // array of one-octet values
        if let Some(sb)
                = self.subpacket(
                    SubpacketTag::PreferredSymmetricAlgorithms) {
            if let SubpacketValue::PreferredSymmetricAlgorithms(v)
                    = &sb.value {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Preferred Hash Algorithms subpacket.
    ///
    /// A [Preferred Hash Algorithms subpacket] lists what hash
    /// algorithms the user prefers.  When signing a message that
    /// should be verified by a particular recipient, the OpenPGP
    /// implementation should not use an algorithm that is not on this
    /// list.
    ///
    /// [Preferred Hash Algorithms subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.8
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn preferred_hash_algorithms(&self) -> Option<&[HashAlgorithm]> {
        // array of one-octet values
        if let Some(sb)
                = self.subpacket(
                    SubpacketTag::PreferredHashAlgorithms) {
            if let SubpacketValue::PreferredHashAlgorithms(v) = &sb.value {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Preferred Compression Algorithms
    /// subpacket.
    ///
    /// A [Preferred Compression Algorithms subpacket] lists what
    /// compression algorithms the user prefers.  When compressing a
    /// message for a recipient, the OpenPGP implementation should not
    /// use an algorithm that is not on the list.
    ///
    /// [Preferred Compression Algorithms subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.9
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first for the
    /// subpacket on the binding signature of the User ID or the User
    /// Attribute used to locate the certificate (or the primary User
    /// ID, if it was addressed by Key ID or fingerprint).  If the
    /// binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn preferred_compression_algorithms(&self)
                                            -> Option<&[CompressionAlgorithm]>
    {
        // array of one-octet values
        if let Some(sb)
                = self.subpacket(
                    SubpacketTag::PreferredCompressionAlgorithms) {
            if let SubpacketValue::PreferredCompressionAlgorithms(v)
                    = &sb.value {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Preferred AEAD Algorithms subpacket.
    ///
    /// The [Preferred AEAD Algorithms subpacket] indicates what AEAD
    /// algorithms the key holder prefers ordered by preference.  If
    /// this is set, then the AEAD feature flag should in the
    /// [Features subpacket] should also be set.
    ///
    /// Note: because support for AEAD has not yet been standardized,
    /// we recommend not yet advertising support for it.
    ///
    /// [Preferred AEAD Algorithms subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.8
    /// [Features subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.25
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn preferred_aead_algorithms(&self)
                                     -> Option<&[AEADAlgorithm]> {
        // array of one-octet values
        if let Some(sb)
                = self.subpacket(
                    SubpacketTag::PreferredAEADAlgorithms) {
            if let SubpacketValue::PreferredAEADAlgorithms(v)
                    = &sb.value {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Key Server Preferences subpacket.
    ///
    /// The [Key Server Preferences subpacket] indicates to key
    /// servers how they should handle the certificate.
    ///
    /// [Key Server Preferences subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.17
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first for the
    /// subpacket on the binding signature of the User ID or the User
    /// Attribute used to locate the certificate (or the primary User
    /// ID, if it was addressed by Key ID or fingerprint).  If the
    /// binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn key_server_preferences(&self) -> Option<KeyServerPreferences> {
        // N octets of flags
        if let Some(sb) = self.subpacket(SubpacketTag::KeyServerPreferences) {
            if let SubpacketValue::KeyServerPreferences(v) = &sb.value {
                Some(v.clone())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Preferred Key Server subpacket.
    ///
    /// The [Preferred Key Server subpacket] contains a link to a key
    /// server where the certificate holder plans to publish updates
    /// to their certificate (e.g., extensions to the expiration time,
    /// new subkeys, revocation certificates).
    ///
    /// The Preferred Key Server subpacket should be handled
    /// cautiously, because it can be used by a certificate holder to
    /// track communication partners.
    ///
    /// [Preferred Key Server subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.18
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn preferred_key_server(&self) -> Option<&[u8]> {
        // String
        if let Some(sb)
                = self.subpacket(SubpacketTag::PreferredKeyServer) {
            if let SubpacketValue::PreferredKeyServer(v) = &sb.value {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Policy URI subpacket.
    ///
    /// The [Policy URI subpacket] contains a link to a policy document,
    /// which contains information about the conditions under which
    /// the signature was made.
    ///
    /// [Policy URI subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.20
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn policy_uri(&self) -> Option<&[u8]> {
        // String
        if let Some(sb)
                = self.subpacket(SubpacketTag::PolicyURI) {
            if let SubpacketValue::PolicyURI(v) = &sb.value {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Primary UserID subpacket.
    ///
    /// The [Primary User ID subpacket] indicates whether the
    /// associated User ID or User Attribute should be considered the
    /// primary User ID.  It is possible that this is set on multiple
    /// User IDs.  See the documentation for
    /// [`ValidCert::primary_userid`] for an explanation of how
    /// Sequoia resolves this ambiguity.
    ///
    /// [Primary User ID subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.19
    /// [`ValidCert::primary_userid`]: crate::cert::ValidCert::primary_userid()
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn primary_userid(&self) -> Option<bool> {
        // 1 octet, Boolean
        if let Some(sb)
                = self.subpacket(SubpacketTag::PrimaryUserID) {
            if let SubpacketValue::PrimaryUserID(v) = sb.value {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Key Flags subpacket.
    ///
    /// The [Key Flags subpacket] describes a key's capabilities
    /// (certification capable, signing capable, etc.).  In the case
    /// of subkeys, the Key Flags are located on the subkey's binding
    /// signature.  For primary keys, locating the correct Key Flags
    /// subpacket is more complex: First, the primary User ID is
    /// consulted.  If the primary User ID contains a Key Flags
    /// subpacket, that is used.  Otherwise, any direct key signature
    /// is considered.  If that still doesn't contain a Key Flags
    /// packet, then the primary key should be assumed to be
    /// certification capable.
    ///
    /// [Key Flags subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.21
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn key_flags(&self) -> Option<KeyFlags> {
        // N octets of flags
        if let Some(sb) = self.subpacket(SubpacketTag::KeyFlags) {
            if let SubpacketValue::KeyFlags(v) = &sb.value {
                Some(v.clone())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Signer's UserID subpacket.
    ///
    /// The [Signer's User ID subpacket] indicates, which User ID made
    /// the signature.  This is useful when a key has multiple User
    /// IDs, which correspond to different roles.  For instance, it is
    /// not uncommon to use the same certificate in private as well as
    /// for a club.
    ///
    /// [Signer's User ID subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.22
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn signers_user_id(&self) -> Option<&[u8]> {
        // String
        if let Some(sb)
                = self.subpacket(SubpacketTag::SignersUserID) {
            if let SubpacketValue::SignersUserID(v) = &sb.value {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Reason for Revocation subpacket.
    ///
    /// The [Reason For Revocation subpacket] indicates why a key,
    /// User ID, or User Attribute is being revoked.  It includes both
    /// a machine readable code, and a human-readable string.  The
    /// code is essential as it indicates to the OpenPGP
    /// implementation that reads the certificate whether the key was
    /// compromised (a hard revocation), or is no longer used (a soft
    /// revocation).  In the former case, the OpenPGP implementation
    /// must conservatively consider all past signatures as suspect
    /// whereas in the latter case, past signatures can still be
    /// considered valid.
    ///
    /// [Reason For Revocation subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.23
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn reason_for_revocation(&self)
                                 -> Option<(ReasonForRevocation, &[u8])> {
        // 1 octet of revocation code, N octets of reason string
        if let Some(sb) = self.subpacket(SubpacketTag::ReasonForRevocation) {
            if let SubpacketValue::ReasonForRevocation {
                code, reason,
            } = &sb.value {
                Some((*code, reason))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Features subpacket.
    ///
    /// A [Features subpacket] lists what OpenPGP features the user
    /// wants to use.  When creating a message, features that the
    /// intended recipients do not support should not be used.
    /// However, because this information is rarely held up to date in
    /// practice, this information is only advisory, and
    /// implementations are allowed to infer what features the
    /// recipients support from contextual clues, e.g., their past
    /// behavior.
    ///
    /// [Features subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.24
    /// [features]: crate::types::Features
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn features(&self) -> Option<Features> {
        // N octets of flags
        if let Some(sb) = self.subpacket(SubpacketTag::Features) {
            if let SubpacketValue::Features(v) = &sb.value {
                Some(v.clone())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the value of the Signature Target subpacket.
    ///
    /// The [Signature Target subpacket] is used to identify the target
    /// of a signature.  This is used when revoking a signature, and
    /// by timestamp signatures.  It contains a hash of the target
    /// signature.
    ///
    ///   [Signature Target subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.25
    ///
    /// If the subpacket is not present in the hashed subpacket area,
    /// this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.
    pub fn signature_target(&self) -> Option<(PublicKeyAlgorithm,
                                              HashAlgorithm,
                                              &[u8])> {
        // 1 octet public-key algorithm, 1 octet hash algorithm, N
        // octets hash
        if let Some(sb) = self.subpacket(SubpacketTag::SignatureTarget) {
            if let SubpacketValue::SignatureTarget {
                pk_algo, hash_algo, digest,
            } = &sb.value {
                Some((*pk_algo, *hash_algo, digest))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns references to all Embedded Signature subpackets.
    ///
    /// The [Embedded Signature subpacket] is normally used to hold a
    /// [Primary Key Binding signature], which binds a
    /// signing-capable, authentication-capable, or
    /// certification-capable subkey to the primary key.  Since this
    /// information is self-authenticating, it is usually stored in
    /// the unhashed subpacket area.
    ///
    /// [Embedded Signature subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.26
    /// [Primary Key Binding signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///
    /// If the subpacket is not present in the hashed subpacket area
    /// or in the unhashed subpacket area, this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.  Otherwise, the last one is returned from the
    /// unhashed subpacket area.
    pub fn embedded_signatures(&self)
                               -> impl Iterator<Item = &Signature> + Send + Sync
    {
        self.subpackets(SubpacketTag::EmbeddedSignature).map(|sb| {
            if let SubpacketValue::EmbeddedSignature(v) = &sb.value {
                v
            } else {
                unreachable!(
                    "subpackets(EmbeddedSignature) returns EmbeddedSignatures"
                );
            }
        })
    }

    /// Returns mutable references to all Embedded Signature subpackets.
    ///
    /// The [Embedded Signature subpacket] is normally used to hold a
    /// [Primary Key Binding signature], which binds a
    /// signing-capable, authentication-capable, or
    /// certification-capable subkey to the primary key.  Since this
    /// information is self-authenticating, it is usually stored in
    /// the unhashed subpacket area.
    ///
    /// [Embedded Signature subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.26
    /// [Primary Key Binding signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///
    /// If the subpacket is not present in the hashed subpacket area
    /// or in the unhashed subpacket area, this returns `None`.
    ///
    /// Note: if the signature contains multiple instances of this
    /// subpacket in the hashed subpacket area, the last one is
    /// returned.  Otherwise, the last one is returned from the
    /// unhashed subpacket area.
    pub fn embedded_signatures_mut(&mut self)
        -> impl Iterator<Item = &mut Signature> + Send + Sync
    {
        self.subpackets_mut(SubpacketTag::EmbeddedSignature).map(|sb| {
            if let SubpacketValue::EmbeddedSignature(v) = &mut sb.value {
                v
            } else {
                unreachable!(
                    "subpackets_mut(EmbeddedSignature) returns EmbeddedSignatures"
                );
            }
        })
    }

    /// Returns the intended recipients.
    ///
    /// The [Intended Recipient subpacket] holds the fingerprint of a
    /// certificate.
    ///
    ///   [Intended Recipient subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.29
    ///
    /// When signing a message, the message should include one such
    /// subpacket for each intended recipient.  Note: not all messages
    /// have intended recipients.  For instance, when signing an open
    /// letter, or a software release, the message is intended for
    /// anyone.
    ///
    /// When processing a signature, the application should ensure
    /// that if there are any such subpackets, then one of the
    /// subpackets identifies the recipient's certificate (or user
    /// signed the message).  If this is not the case, then an
    /// attacker may have taken the message out of its original
    /// context.  For instance, if Alice sends a signed email to Bob,
    /// with the content: "I agree to the contract", and Bob forwards
    /// that message to Carol, then Carol may think that Alice agreed
    /// to a contract with her if the signature appears to be valid!
    /// By adding an intended recipient, it is possible for Carol's
    /// mail client to warn her that although Alice signed the
    /// message, the content was intended for Bob and not for her.
    ///
    /// This returns all instances of the Intended Recipient subpacket
    /// in the hashed subpacket area.
    pub fn intended_recipients(&self)
                               -> impl Iterator<Item=&Fingerprint> + Send + Sync
    {
        self.subpackets(SubpacketTag::IntendedRecipient)
            .map(|sb| {
                match sb.value() {
                    SubpacketValue::IntendedRecipient(ref fp) => fp,
                    _ => unreachable!(),
                }
            })
    }

    /// Returns the digests of attested certifications.
    ///
    /// This feature is [experimental](crate#experimental-features).
    ///
    /// Allows the certificate holder to attest to third party
    /// certifications, allowing them to be distributed with the
    /// certificate.  This can be used to address certificate flooding
    /// concerns.
    ///
    /// Note: The maximum size of the hashed signature subpacket area
    /// constrains the number of attestations that can be stored in a
    /// signature.  If the certificate holder attested to more
    /// certifications, the digests are split across multiple attested
    /// key signatures with the same creation time.
    ///
    /// The standard strongly suggests that the digests should be
    /// sorted.  However, this function returns the digests in the
    /// order they are stored in the subpacket, which may not be
    /// sorted.
    ///
    /// To address both issues, collect all digests from all attested
    /// key signatures with the most recent creation time into a data
    /// structure that allows efficient lookups, such as [`HashSet`]
    /// or [`BTreeSet`].
    ///
    /// See [Section 5.2.3.30 of RFC 4880bis] for details.
    ///
    ///   [`HashSet`]: std::collections::HashSet
    ///   [`BTreeSet`]: std::collections::BTreeSet
    ///   [Section 5.2.3.30 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-10.html#section-5.2.3.30
    pub fn attested_certifications(&self)
        -> Result<impl Iterator<Item=&[u8]> + Send + Sync>
    {
        if self.hashed_area()
            .subpackets(SubpacketTag::AttestedCertifications).count() > 1
            || self.unhashed_area()
            .subpackets(SubpacketTag::AttestedCertifications).count() != 0
        {
            return Err(Error::BadSignature(
                "Wrong number of attested certification subpackets".into())
                       .into());
        }

        Ok(self.subpackets(SubpacketTag::AttestedCertifications)
           .flat_map(|sb| {
               match sb.value() {
                   SubpacketValue::AttestedCertifications(digests) =>
                       digests.iter().map(|d| d.as_ref()),
                   _ => unreachable!(),
               }
           }))
    }
}

impl TryFrom<Signature> for Signature4 {
    type Error = anyhow::Error;

    fn try_from(sig: Signature) -> Result<Self> {
        match sig {
            Signature::V4(sig) => Ok(sig),
            sig => Err(
                Error::InvalidArgument(
                    format!(
                        "Got a v{}, require a v4 signature",
                        sig.version()))
                    .into()),
        }
    }
}

impl Deref for Signature4 {
    type Target = signature::SignatureFields;

    fn deref(&self) -> &Self::Target {
        &self.fields
    }
}

impl DerefMut for Signature4 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fields
    }
}

impl signature::SignatureBuilder {
    /// Modifies the unhashed subpacket area.
    ///
    /// This method provides a builder-style interface for modifying
    /// the unhashed subpacket area.
    ///
    /// Normally, to modify a subpacket area in a non-standard way
    /// (that is, when there are no subpacket-specific function like
    /// [`SignatureBuilder::set_signature_validity_period`] that
    /// implement the required functionality), you need to do
    /// something like the following:
    ///
    ///   [`SignatureBuilder::set_signature_validity_period`]: super::SignatureBuilder::set_signature_validity_period()
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::types::Curve;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::{
    /// #     Subpacket,
    /// #     SubpacketTag,
    /// #     SubpacketValue,
    /// # };
    /// # use openpgp::types::SignatureType;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let key: Key<key::SecretParts, key::PrimaryRole>
    /// #     = Key4::generate_ecc(true, Curve::Ed25519)?.into();
    /// # let mut signer = key.into_keypair()?;
    /// # let msg = b"Hello, World";
    /// #
    /// let mut builder = SignatureBuilder::new(SignatureType::Binary)
    ///     // Build up the signature.
    ///     ;
    /// builder.unhashed_area_mut().add(Subpacket::new(
    ///         SubpacketValue::Unknown {
    ///             tag: SubpacketTag::Private(61),
    ///             body: [0x6D, 0x6F, 0x6F].to_vec(),
    ///         },
    ///         true)?)?;
    /// let sig = builder.sign_message(&mut signer, msg)?;
    /// # let mut sig = sig;
    /// # sig.verify_message(signer.public(), msg)?;
    /// # Ok(()) }
    /// ```
    ///
    /// This is necessary, because modifying the subpacket area
    /// doesn't follow the builder pattern like the surrounding code.
    /// Using this function, you can instead do:
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::{
    /// #     Subpacket,
    /// #     SubpacketTag,
    /// #     SubpacketValue,
    /// # };
    /// # use openpgp::types::Curve;
    /// # use openpgp::types::SignatureType;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let key: Key<key::SecretParts, key::PrimaryRole>
    /// #     = Key4::generate_ecc(true, Curve::Ed25519)?.into();
    /// # let mut signer = key.into_keypair()?;
    /// # let msg = b"Hello, World";
    /// #
    /// let sig = SignatureBuilder::new(SignatureType::Binary)
    ///     // Call some setters.
    ///     .modify_unhashed_area(|mut a| {
    ///         a.add(Subpacket::new(
    ///             SubpacketValue::Unknown {
    ///                 tag: SubpacketTag::Private(61),
    ///                 body: [0x6D, 0x6F, 0x6F].to_vec(),
    ///             },
    ///             true)?);
    ///         Ok(a)
    ///     })?
    ///    .sign_message(&mut signer, msg)?;
    /// # let mut sig = sig;
    /// # sig.verify_message(signer.public(), msg)?;
    /// # Ok(()) }
    /// ```
    ///
    /// If you are only interested in modifying an existing
    /// signature's unhashed area, it may be better to simply modify
    /// the signature in place using
    /// [`SignatureBuilder::modify_unhashed_area`] rather than to create a
    /// new signature, because modifying the unhashed area doesn't
    /// invalidate any existing signature.
    ///
    ///   [`SignatureBuilder::modify_unhashed_area`]: super::SignatureBuilder::modify_unhashed_area
    ///
    /// # Examples
    ///
    /// Create a signature with a custom, non-critical subpacket in
    /// the unhashed area:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::packet::signature::subpacket::{
    ///     Subpacket,
    ///     SubpacketTag,
    ///     SubpacketValue,
    /// };
    /// use openpgp::types::SignatureType;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    ///
    /// let (cert, _) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///     .generate()?;
    /// let mut signer = cert.primary_key().key().clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let msg = b"Hello, World";
    ///
    /// let sig = SignatureBuilder::new(SignatureType::Binary)
    ///     // Call some setters.
    ///     .modify_unhashed_area(|mut a| {
    ///         a.add(Subpacket::new(
    ///             SubpacketValue::Unknown {
    ///                 tag: SubpacketTag::Private(61),
    ///                 body: [0x6D, 0x6F, 0x6F].to_vec(),
    ///             },
    ///             true)?);
    ///         Ok(a)
    ///     })?
    ///    .sign_message(&mut signer, msg)?;
    /// # let mut sig = sig;
    /// # sig.verify_message(signer.public(), msg)?;
    /// # Ok(()) }
    /// ```
    pub fn modify_unhashed_area<F>(mut self, f: F)
        -> Result<Self>
        where F: FnOnce(SubpacketArea) -> Result<SubpacketArea>
    {
        self.fields.subpackets.unhashed_area
            = f(self.fields.subpackets.unhashed_area)?;
        Ok(self)
    }

    /// Modifies the hashed subpacket area.
    ///
    /// This method provides a builder-style interface for modifying
    /// the hashed subpacket area.
    ///
    /// Normally, to modify a subpacket area in a non-standard way
    /// (that is, when there are no subpacket-specific function like
    /// [`SignatureBuilder::set_signature_validity_period`] that
    /// implement the required functionality), you need to do
    /// something like the following:
    ///
    ///   [`SignatureBuilder::set_signature_validity_period`]: super::SignatureBuilder::set_signature_validity_period()
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::types::Curve;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::{
    /// #     Subpacket,
    /// #     SubpacketTag,
    /// #     SubpacketValue,
    /// # };
    /// # use openpgp::types::SignatureType;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let key: Key<key::SecretParts, key::PrimaryRole>
    /// #     = Key4::generate_ecc(true, Curve::Ed25519)?.into();
    /// # let mut signer = key.into_keypair()?;
    /// # let msg = b"Hello, World";
    /// #
    /// let mut builder = SignatureBuilder::new(SignatureType::Binary)
    ///     // Build up the signature.
    ///     ;
    /// builder.hashed_area_mut().add(Subpacket::new(
    ///         SubpacketValue::Unknown {
    ///             tag: SubpacketTag::Private(61),
    ///             body: [0x6D, 0x6F, 0x6F].to_vec(),
    ///         },
    ///         true)?)?;
    /// let sig = builder.sign_message(&mut signer, msg)?;
    /// # let mut sig = sig;
    /// # sig.verify_message(signer.public(), msg)?;
    /// # Ok(()) }
    /// ```
    ///
    /// This is necessary, because modifying the subpacket area
    /// doesn't follow the builder pattern like the surrounding code.
    /// Using this function, you can instead do:
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::{
    /// #     Subpacket,
    /// #     SubpacketTag,
    /// #     SubpacketValue,
    /// # };
    /// # use openpgp::types::Curve;
    /// # use openpgp::types::SignatureType;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let key: Key<key::SecretParts, key::PrimaryRole>
    /// #     = Key4::generate_ecc(true, Curve::Ed25519)?.into();
    /// # let mut signer = key.into_keypair()?;
    /// # let msg = b"Hello, World";
    /// #
    /// let sig = SignatureBuilder::new(SignatureType::Binary)
    ///     // Call some setters.
    ///     .modify_hashed_area(|mut a| {
    ///         a.add(Subpacket::new(
    ///             SubpacketValue::Unknown {
    ///                 tag: SubpacketTag::Private(61),
    ///                 body: [0x6D, 0x6F, 0x6F].to_vec(),
    ///             },
    ///             true)?);
    ///         Ok(a)
    ///     })?
    ///    .sign_message(&mut signer, msg)?;
    /// # let mut sig = sig;
    /// # sig.verify_message(signer.public(), msg)?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Examples
    ///
    /// Add a critical, custom subpacket to a certificate's direct key
    /// signature:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::packet::signature::subpacket::{
    ///     Subpacket,
    ///     SubpacketTag,
    ///     SubpacketValue,
    /// };
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
    /// let vc = cert.with_policy(p, None)?;
    ///
    /// let sig = vc.direct_key_signature().expect("direct key signature");
    /// let sig = SignatureBuilder::from(sig.clone())
    ///     .modify_hashed_area(|mut a| {
    ///         a.add(Subpacket::new(
    ///             SubpacketValue::Unknown {
    ///                 tag: SubpacketTag::Private(61),
    ///                 body: [0x6D, 0x6F, 0x6F].to_vec(),
    ///             },
    ///             true)?)?;
    ///         Ok(a)
    ///     })?
    ///     .sign_direct_key(&mut signer, None)?;
    ///
    /// // Merge in the new signature.
    /// let cert = cert.insert_packets(sig)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Update a certificate's feature set by updating the `Features`
    /// subpacket on any direct key signature, and any User ID binding
    /// signatures:
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
    ///         .sign_direct_key(&mut signer, Some(pk))?);
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
    /// let cert = cert.insert_packets(sigs)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn modify_hashed_area<F>(mut self, f: F)
        -> Result<Self>
        where F: FnOnce(SubpacketArea) -> Result<SubpacketArea>
    {
        self.fields.subpackets.hashed_area
            = f(self.fields.subpackets.hashed_area)?;
        Ok(self)
    }

    /// Sets the Signature Creation Time subpacket.
    ///
    /// Adds a [Signature Creation Time subpacket] to the hashed
    /// subpacket area.  This function first removes any Signature
    /// Creation Time subpacket from the hashed subpacket area.
    ///
    /// The Signature Creation Time subpacket specifies when the
    /// signature was created.  According to the standard, all
    /// signatures must include a Signature Creation Time subpacket in
    /// the signature's hashed area.  This doesn't mean that the time
    /// stamp is correct: the issuer can always forge it.
    ///
    /// When creating a signature using a SignatureBuilder or the
    /// [streaming `Signer`], it is not necessary to explicitly set
    /// this subpacket: those functions automatically set both the
    /// [Issuer Fingerprint subpacket] and the Issuer subpacket, if
    /// they have not been set explicitly.
    ///
    /// [Signature Creation Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    /// [streaming `Signer`]: crate::serialize::stream::Signer
    ///
    /// # Examples
    ///
    /// Create a backdated signature:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::SignatureBuilder;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let (cert, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     // We also need to backdate the certificate.
    /// #     .set_creation_time(
    /// #         std::time::SystemTime::now()
    /// #             - std::time::Duration::new(2 * 24 * 60 * 60, 0))
    /// #     .generate()?;
    /// # let mut signer = cert.primary_key().key().clone()
    /// #     .parts_into_secret()?.into_keypair()?;
    /// let msg = "hiermit kündige ich den mit Ihnen bestehenden Vertrag fristgerecht.";
    ///
    /// let mut sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .set_signature_creation_time(
    ///         std::time::SystemTime::now()
    ///         - std::time::Duration::new(24 * 60 * 60, 0))?
    ///     .sign_message(&mut signer, msg)?;
    ///
    /// assert!(sig.verify_message(signer.public(), msg).is_ok());
    /// # Ok(()) }
    /// ```
    pub fn set_signature_creation_time<T>(mut self, creation_time: T)
        -> Result<Self>
        where T: Into<time::SystemTime>
    {
        self.overrode_creation_time = true;

        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::SignatureCreationTime(
                creation_time.into().try_into()?),
            true)?)?;

        Ok(self)
    }

    /// Causes the builder to use an existing signature creation time
    /// subpacket.
    ///
    /// When converting a [`Signature`] to a `SignatureBuilder`, the
    /// [Signature Creation Time subpacket] is removed from the hashed
    /// area, and saved internally.  When creating the signature, a
    /// Signature Creation Time subpacket with the current time is
    /// normally added to the hashed area.  Calling this function
    /// instead causes the signature generation code to use the cached
    /// `Signature Creation Time` subpacket.
    ///
    /// [`Signature`]: super::Signature
    /// [Signature Creation Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///
    /// This function returns an error if there is no cached
    /// `Signature Creation Time` subpacket.
    ///
    /// # Examples
    ///
    /// Alice signs a message.  Shortly thereafter, Bob signs the
    /// message using a nearly identical Signature packet:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::SignatureBuilder;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let (alice, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// # let mut alices_signer = alice.primary_key().key().clone()
    /// #     .parts_into_secret()?.into_keypair()?;
    /// # let (bob, _) =
    /// #     CertBuilder::general_purpose(None, Some("bob@example.org"))
    /// #     .generate()?;
    /// # let mut bobs_signer = bob.primary_key().key().clone()
    /// #     .parts_into_secret()?.into_keypair()?;
    /// let msg = "Version 489 of Foo has the SHA256 sum e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    ///
    /// let siga = SignatureBuilder::new(SignatureType::Binary)
    ///     .sign_message(&mut alices_signer, msg)?;
    /// let sigb = SignatureBuilder::from(siga.clone())
    ///     .preserve_signature_creation_time()?
    ///     .sign_message(&mut bobs_signer, msg)?;
    /// #
    /// # let mut siga = siga;
    /// # let mut sigb = sigb;
    /// # assert!(siga.verify_message(alices_signer.public(), msg).is_ok());
    /// # assert!(sigb.verify_message(bobs_signer.public(), msg).is_ok());
    /// # assert_eq!(siga.signature_creation_time(),
    /// #            sigb.signature_creation_time());
    /// # Ok(()) }
    /// ```
    pub fn preserve_signature_creation_time(self)
        -> Result<Self>
    {
        if let Some(t) = self.original_creation_time {
            self.set_signature_creation_time(t)
        } else {
            Err(Error::InvalidOperation(
                "Signature does not contain a Signature Creation Time subpacket".into())
                .into())
        }
    }

    /// Causes the builder to not output a Signature Creation Time
    /// subpacket.
    ///
    /// When creating a signature, a [Signature Creation Time
    /// subpacket] is added to the hashed area if one hasn't been
    /// added already.  This function suppresses that behavior.
    ///
    /// [Signature Creation Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///
    /// [Section 5.2.3.4 of RFC 4880] says that the `Signature
    /// Creation Time` subpacket must be present in the hashed area.
    /// This function clears any `Signature Creation Time` subpackets
    /// from both the hashed area and the unhashed area, and causes
    /// the various `SignatureBuilder` finalizers to not emit a
    /// `Signature Creation Time` subpacket.  This function should
    /// only be used for generating test data.
    ///
    /// [Section 5.2.3.4 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    ///
    /// # Examples
    ///
    /// Create a signature without a Signature Creation Time
    /// subpacket.  As per the specification, Sequoia considers such
    /// signatures to be invalid:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::SignatureBuilder;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let (cert, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// # let mut signer = cert.primary_key().key().clone()
    /// #     .parts_into_secret()?.into_keypair()?;
    /// let msg = "Some things are timeless.";
    ///
    /// let mut sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .suppress_signature_creation_time()?
    ///     .sign_message(&mut signer, msg)?;
    ///
    /// assert!(sig.verify_message(signer.public(), msg).is_err());
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::SignatureCreationTime)
    /// #    .count(),
    /// #    0);
    /// # Ok(()) }
    /// ```
    pub fn suppress_signature_creation_time(mut self)
        -> Result<Self>
    {
        self.overrode_creation_time = true;

        self.hashed_area.remove_all(SubpacketTag::SignatureCreationTime);
        self.unhashed_area.remove_all(SubpacketTag::SignatureCreationTime);

        Ok(self)
    }

    /// Sets the Signature Expiration Time subpacket.
    ///
    /// Adds a [Signature Expiration Time subpacket] to the hashed
    /// subpacket area.  This function first removes any Signature
    /// Expiration Time subpacket from the hashed subpacket area.
    ///
    /// [Signature Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.10
    ///
    /// This function is called `set_signature_validity_period` and
    /// not `set_signature_expiration_time`, which would be more
    /// consistent with the subpacket's name, because the latter
    /// suggests an absolute time, but the time is actually relative
    /// to the signature's creation time, which is stored in the
    /// signature's [Signature Creation Time subpacket] and set using
    /// [`SignatureBuilder::set_signature_creation_time`].
    ///
    /// [Signature Creation Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.4
    /// [`SignatureBuilder::set_signature_creation_time`]: super::SignatureBuilder::set_signature_creation_time()
    ///
    /// A Signature Expiration Time subpacket specifies when the
    /// signature expires.  This is different from the [Key Expiration
    /// Time subpacket], which is set using
    /// [`SignatureBuilder::set_key_validity_period`], and used to
    /// specify when an associated key expires.  The difference is
    /// that in the former case, the signature itself expires, but in
    /// the latter case, only the associated key expires.  This
    /// difference is critical: if a binding signature expires, then
    /// an OpenPGP implementation will still consider the associated
    /// key to be valid if there is another valid binding signature,
    /// even if it is older than the expired signature; if the active
    /// binding signature indicates that the key has expired, then
    /// OpenPGP implementations will not fallback to an older binding
    /// signature.
    ///
    /// [Key Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    /// [`SignatureBuilder::set_key_validity_period`]: super::SignatureBuilder::set_key_validity_period()
    ///
    /// There are several cases where having a signature expire is
    /// useful.  Say Alice certifies Bob's certificate for
    /// `bob@example.org`.  She can limit the lifetime of the
    /// certification to force her to reevaluate the certification
    /// shortly before it expires.  For instance, is Bob still
    /// associated with `example.org`?  Does she have reason to
    /// believe that his key has been compromised?  Using an
    /// expiration is common in the X.509 ecosystem.  For instance,
    /// [Let's Encrypt] issues certificates with 90-day lifetimes.
    ///
    /// [Let's Encrypt]: https://letsencrypt.org/2015/11/09/why-90-days.html
    ///
    /// Having signatures expire can also be useful when deploying
    /// software.  For instance, you might have a service that
    /// installs an update if it has been signed by a trusted
    /// certificate.  To prevent an adversary from coercing the
    /// service to install an older version, you could limit the
    /// signature's lifetime to just a few minutes.
    ///
    /// # Examples
    ///
    /// Create a signature that expires in 10 minutes:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::SignatureBuilder;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// let (cert, _) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///         .generate()?;
    /// let mut signer = cert.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    ///
    /// let msg = "install e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    ///
    /// let mut sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .set_signature_validity_period(
    ///         std::time::Duration::new(10 * 60, 0))?
    ///     .sign_message(&mut signer, msg)?;
    ///
    /// assert!(sig.verify_message(signer.public(), msg).is_ok());
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::SignatureExpirationTime)
    /// #    .count(),
    /// #    1);
    /// # Ok(()) }
    /// ```
    ///
    /// Create a certification that expires at the end of the year
    /// (give or take a few seconds) unless the new year is in a
    /// month, then have it expire at the end of the following year:
    ///
    /// ```
    /// use std::time::{SystemTime, UNIX_EPOCH, Duration};
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::SignatureBuilder;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let (cert, _) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///         .generate()?;
    /// let mut signer = cert.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    ///
    /// let msg = "message.";
    ///
    /// // Average number of seconds in a year.  See:
    /// // https://en.wikipedia.org/wiki/Year .
    /// const SECONDS_IN_YEAR: u64 = (365.2425 * 24. * 60. * 60.) as u64;
    ///
    /// let now = SystemTime::now();
    /// let since_epoch = now.duration_since(UNIX_EPOCH)?.as_secs();
    /// let next_year
    ///     = (since_epoch + SECONDS_IN_YEAR) - (since_epoch % SECONDS_IN_YEAR);
    /// // Make sure the expiration is at least a month in the future.
    /// let next_year = if next_year - since_epoch < SECONDS_IN_YEAR / 12 {
    ///     next_year + SECONDS_IN_YEAR
    /// } else {
    ///     next_year
    /// };
    /// let next_year = UNIX_EPOCH + Duration::new(next_year, 0);
    /// let next_year = next_year.duration_since(now)?;
    ///
    /// let sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .set_signature_creation_time(now)?
    ///     .set_signature_validity_period(next_year)?
    ///     .sign_message(&mut signer, msg)?;
    /// #
    /// # let mut sig = sig;
    /// # assert!(sig.verify_message(signer.public(), msg).is_ok());
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::SignatureExpirationTime)
    /// #    .count(),
    /// #    1);
    /// # Ok(()) }
    /// ```
    pub fn set_signature_validity_period<D>(mut self, expires_in: D)
        -> Result<Self>
        where D: Into<time::Duration>
    {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::SignatureExpirationTime(
                Duration::try_from(expires_in.into())?),
            true)?)?;

        Ok(self)
    }

    /// Sets the Exportable Certification subpacket.
    ///
    /// Adds an [Exportable Certification subpacket] to the hashed
    /// subpacket area.  This function first removes any Exportable
    /// Certification subpacket from the hashed subpacket area.
    ///
    /// [Exportable Certification subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.11
    ///
    /// The Exportable Certification subpacket indicates whether the
    /// signature should be exported (e.g., published on a public key
    /// server) or not.  When using [`Serialize::export`] to export a
    /// certificate, signatures that have this subpacket present and
    /// set to false are not serialized.
    ///
    /// [`Serialize::export`]: https://docs.sequoia-pgp.org/sequoia_openpgp/serialize/trait.Serialize.html#method.export
    ///
    /// # Examples
    ///
    /// Alice certificates Bob's certificate, but because she doesn't
    /// want to publish it, she creates a so-called local signature by
    /// adding an Exportable Certification subpacket set to `false` to
    /// the signature:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// let p = &StandardPolicy::new();
    ///
    /// let (alice, _)
    ///     = CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///         .generate()?;
    /// let mut alices_signer = alice.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    ///
    /// let (bob, _)
    ///     = CertBuilder::general_purpose(None, Some("bob@example.org"))
    ///         .generate()?;
    /// let bobs_userid
    ///     = bob.with_policy(p, None)?.userids().nth(0).expect("Added a User ID").userid();
    ///
    /// let certification = SignatureBuilder::new(SignatureType::GenericCertification)
    ///     .set_exportable_certification(false)?
    ///     .sign_userid_binding(
    ///         &mut alices_signer, bob.primary_key().key(), bobs_userid)?;
    /// # assert_eq!(certification
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::ExportableCertification)
    /// #    .count(),
    /// #    1);
    ///
    /// // Merge in the new signature.
    /// let bob = bob.insert_packets(certification)?;
    /// # assert_eq!(bob.bad_signatures().count(), 0);
    /// # assert_eq!(bob.userids().nth(0).unwrap().certifications().count(), 1);
    /// # Ok(()) }
    /// ```
    pub fn set_exportable_certification(mut self, exportable: bool)
                                        -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::ExportableCertification(exportable),
            true)?)?;

        Ok(self)
    }

    /// Sets the Trust Signature subpacket.
    ///
    /// Adds a [Trust Signature subpacket] to the hashed subpacket
    /// area.  This function first removes any Trust Signature
    /// subpacket from the hashed subpacket area.
    ///
    /// [Trust Signature subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.13
    ///
    /// The Trust Signature subpacket indicates the degree to which a
    /// certificate holder is trusted to certify other keys.
    ///
    /// A level of 0 means that the certificate holder is not trusted
    /// to certificate other keys, a level of 1 means that the
    /// certificate holder is a trusted introducer (a [certificate
    /// authority]) and any certifications that they make should be
    /// considered valid.  A level of 2 means the certificate holder
    /// can designate level 1 trusted introducers, etc.
    ///
    /// [certificate authority]: https://en.wikipedia.org/wiki/Certificate_authority
    ///
    /// The trust indicates the degree of confidence.  A value of 120
    /// means that a certification should be considered valid.  A
    /// value of 60 means that a certification should only be
    /// considered partially valid.  In the latter case, typically
    /// three such certifications are required for a binding to be
    /// considered authenticated.
    ///
    /// # Examples
    ///
    /// Alice designates Bob as a fully trusted, trusted introducer:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// let p = &StandardPolicy::new();
    ///
    /// let (alice, _)
    ///     = CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///         .generate()?;
    /// let mut alices_signer = alice.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    ///
    /// let (bob, _)
    ///     = CertBuilder::general_purpose(None, Some("bob@example.org"))
    ///         .generate()?;
    /// let bobs_userid
    ///     = bob.with_policy(p, None)?.userids().nth(0).expect("Added a User ID").userid();
    ///
    /// let certification = SignatureBuilder::new(SignatureType::GenericCertification)
    ///     .set_trust_signature(1, 120)?
    ///     .sign_userid_binding(
    ///         &mut alices_signer, bob.primary_key().component(), bobs_userid)?;
    /// # assert_eq!(certification
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::TrustSignature)
    /// #    .count(),
    /// #    1);
    ///
    /// // Merge in the new signature.
    /// let bob = bob.insert_packets(certification)?;
    /// # assert_eq!(bob.bad_signatures().count(), 0);
    /// # assert_eq!(bob.userids().nth(0).unwrap().certifications().count(), 1);
    /// # Ok(()) }
    /// ```
    pub fn set_trust_signature(mut self, level: u8, trust: u8)
                               -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::TrustSignature {
                level,
                trust,
            },
            true)?)?;

        Ok(self)
    }

    /// Sets the Regular Expression subpacket.
    ///
    /// Adds a [Regular Expression subpacket] to the hashed subpacket
    /// area.  This function first removes any Regular Expression
    /// subpacket from the hashed subpacket area.
    ///
    /// [Regular Expression subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.14
    ///
    /// The Regular Expression subpacket is used in conjunction with a
    /// [Trust Signature subpacket], which is set using
    /// [`SignatureBuilder::set_trust_signature`], to limit the scope
    /// of a trusted introducer.  This is useful, for instance, when a
    /// company has a CA and you only want to trust them to certify
    /// their own employees.
    ///
    /// [Trust Signature subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.13
    /// [`SignatureBuilder::set_trust_signature`]: super::SignatureBuilder::set_trust_signature()
    ///
    /// GnuPG only supports [a limited form of regular expressions].
    ///
    /// [a limited form of regular expressions]: https://git.gnupg.org/cgi-bin/gitweb.cgi?p=gnupg.git;a=blob;f=g10/trustdb.c;h=c4b996a9685486b2095608f6685727022120505f;hb=refs/heads/master#l1537
    ///
    /// Note: The serialized form includes a trailing `NUL` byte.
    /// Sequoia adds this `NUL` when serializing the signature.
    /// Adding it yourself will result in two trailing NUL bytes.
    ///
    /// # Examples
    ///
    /// Alice designates ``openpgp-ca@example.com`` as a fully
    /// trusted, trusted introducer, but only for users from the
    /// ``example.com`` domain:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// let p = &StandardPolicy::new();
    ///
    /// let (alice, _)
    ///     = CertBuilder::general_purpose(None, Some("Alice <alice@example.org>"))
    ///         .generate()?;
    /// let mut alices_signer = alice.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    ///
    /// let (example_com, _)
    ///     = CertBuilder::general_purpose(None, Some("OpenPGP CA <openpgp-ca@example.com>"))
    ///         .generate()?;
    /// let example_com_userid = example_com.with_policy(p, None)?
    ///     .userids().nth(0).expect("Added a User ID").userid();
    ///
    /// let certification = SignatureBuilder::new(SignatureType::GenericCertification)
    ///     .set_trust_signature(1, 120)?
    ///     .set_regular_expression("<[^>]+[@.]example\\.com>$")?
    ///     .sign_userid_binding(
    ///         &mut alices_signer,
    ///         example_com.primary_key().component(),
    ///         example_com_userid)?;
    /// # assert_eq!(certification
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::TrustSignature)
    /// #    .count(),
    /// #    1);
    /// # assert_eq!(certification
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::RegularExpression)
    /// #    .count(),
    /// #    1);
    ///
    /// // Merge in the new signature.
    /// let example_com = example_com.insert_packets(certification)?;
    /// # assert_eq!(example_com.bad_signatures().count(), 0);
    /// # assert_eq!(example_com.userids().nth(0).unwrap().certifications().count(), 1);
    /// # Ok(()) }
    /// ```
    pub fn set_regular_expression<R>(mut self, re: R) -> Result<Self>
        where R: AsRef<[u8]>
    {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::RegularExpression(re.as_ref().to_vec()),
            true)?)?;

        Ok(self)
    }

    /// Sets a Regular Expression subpacket.
    ///
    /// Adds a [Regular Expression subpacket] to the hashed subpacket
    /// area.  Unlike [`SignatureBuilder::set_regular_expression`],
    /// this function does not first remove any Regular Expression
    /// subpacket from the hashed subpacket area, but adds an
    /// additional Regular Expression subpacket to the hashed
    /// subpacket area.
    ///
    /// [`SignatureBuilder::set_regular_expression`]: super::SignatureBuilder::set_regular_expression()
    /// [Regular Expression subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.14
    ///
    /// The Regular Expression subpacket is used in conjunction with a
    /// [Trust Signature subpacket], which is set using
    /// [`SignatureBuilder::set_trust_signature`], to limit the scope
    /// of a trusted introducer.  This is useful, for instance, when a
    /// company has a CA and you only want to trust them to certify
    /// their own employees.
    ///
    /// [Trust Signature subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.13
    /// [`SignatureBuilder::set_trust_signature`]: super::SignatureBuilder::set_trust_signature()
    ///
    /// GnuPG only supports [a limited form of regular expressions].
    ///
    /// [a limited form of regular expressions]: https://git.gnupg.org/cgi-bin/gitweb.cgi?p=gnupg.git;a=blob;f=g10/trustdb.c;h=c4b996a9685486b2095608f6685727022120505f;hb=refs/heads/master#l1537
    ///
    /// Note: The serialized form includes a trailing `NUL` byte.
    /// Sequoia adds this `NUL` when serializing the signature.
    /// Adding it yourself will result in two trailing NUL bytes.
    ///
    /// # Examples
    ///
    /// Alice designates ``openpgp-ca@example.com`` as a fully
    /// trusted, trusted introducer, but only for users from the
    /// ``example.com`` and ``example.net`` domains:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// let p = &StandardPolicy::new();
    ///
    /// let (alice, _)
    ///     = CertBuilder::general_purpose(None, Some("Alice <alice@example.org>"))
    ///         .generate()?;
    /// let mut alices_signer = alice.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    ///
    /// let (example_com, _)
    ///     = CertBuilder::general_purpose(None, Some("OpenPGP CA <openpgp-ca@example.com>"))
    ///         .generate()?;
    /// let example_com_userid = example_com.with_policy(p, None)?
    ///     .userids().nth(0).expect("Added a User ID").userid();
    ///
    /// let certification = SignatureBuilder::new(SignatureType::GenericCertification)
    ///     .set_trust_signature(1, 120)?
    ///     .set_regular_expression("<[^>]+[@.]example\\.com>$")?
    ///     .add_regular_expression("<[^>]+[@.]example\\.net>$")?
    ///     .sign_userid_binding(
    ///         &mut alices_signer,
    ///         example_com.primary_key().component(),
    ///         example_com_userid)?;
    /// # assert_eq!(certification
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::TrustSignature)
    /// #    .count(),
    /// #    1);
    /// # assert_eq!(certification
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::RegularExpression)
    /// #    .count(),
    /// #    2);
    ///
    /// // Merge in the new signature.
    /// let example_com = example_com.insert_packets(certification)?;
    /// # assert_eq!(example_com.bad_signatures().count(), 0);
    /// # assert_eq!(example_com.userids().nth(0).unwrap().certifications().count(), 1);
    /// # Ok(()) }
    /// ```
    pub fn add_regular_expression<R>(mut self, re: R) -> Result<Self>
        where R: AsRef<[u8]>
    {
        self.hashed_area.add(Subpacket::new(
            SubpacketValue::RegularExpression(re.as_ref().to_vec()),
            true)?)?;

        Ok(self)
    }

    /// Sets the Revocable subpacket.
    ///
    /// Adds a [Revocable subpacket] to the hashed subpacket area.
    /// This function first removes any Revocable subpacket from the
    /// hashed subpacket area.
    ///
    /// [Revocable subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.12
    ///
    /// The Revocable subpacket indicates whether a certification may
    /// be later revoked by creating a [Certification revocation
    /// signature] (0x30) that targets the signature using the
    /// [Signature Target subpacket] (set using the
    /// [`SignatureBuilder::set_signature_target`] method).
    ///
    /// [Certification revocation signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    /// [Signature Target subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.25
    /// [`SignatureBuilder::set_signature_target`]: super::SignatureBuilder::set_signature_target()
    ///
    /// # Examples
    ///
    /// Alice certifies Bob's key and marks the certification as
    /// irrevocable.  Since she can't revoke the signature, she limits
    /// the scope of misuse by setting the signature to expire in a
    /// year:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// let p = &StandardPolicy::new();
    ///
    /// let (alice, _)
    ///     = CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///         .generate()?;
    /// let mut alices_signer = alice.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    ///
    /// let (bob, _)
    ///     = CertBuilder::general_purpose(None, Some("bob@example.org"))
    ///         .generate()?;
    /// let bobs_userid
    ///     = bob.with_policy(p, None)?.userids().nth(0).expect("Added a User ID").userid();
    ///
    /// // Average number of seconds in a year.  See:
    /// // https://en.wikipedia.org/wiki/Year .
    /// const SECONDS_IN_YEAR: u64 = (365.2425 * 24. * 60. * 60.) as u64;
    ///
    /// let certification = SignatureBuilder::new(SignatureType::GenericCertification)
    ///     .set_revocable(false)?
    ///     .set_signature_validity_period(
    ///         std::time::Duration::new(SECONDS_IN_YEAR, 0))?
    ///     .sign_userid_binding(
    ///         &mut alices_signer, bob.primary_key().component(), bobs_userid)?;
    /// # assert_eq!(certification
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::Revocable)
    /// #    .count(),
    /// #    1);
    ///
    /// // Merge in the new signature.
    /// let bob = bob.insert_packets(certification)?;
    /// # assert_eq!(bob.bad_signatures().count(), 0);
    /// # assert_eq!(bob.userids().nth(0).unwrap().certifications().count(), 1);
    /// # Ok(()) }
    /// ```
    pub fn set_revocable(mut self, revocable: bool) -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::Revocable(revocable),
            true)?)?;

        Ok(self)
    }

    /// Sets the Key Expiration Time subpacket.
    ///
    /// Adds a [Key Expiration Time subpacket] to the hashed subpacket
    /// area.  This function first removes any Key Expiration Time
    /// subpacket from the hashed subpacket area.
    ///
    /// If `None` is given, any expiration subpacket is removed.
    ///
    /// [Key Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    ///
    /// This function is called `set_key_validity_period` and not
    /// `set_key_expiration_time`, which would be more consistent with
    /// the subpacket's name, because the latter suggests an absolute
    /// time, but the time is actually relative to the associated
    /// key's (*not* the signature's) creation time, which is stored
    /// in the [Key].
    ///
    /// [Key]: https://tools.ietf.org/html/rfc4880#section-5.5.2
    ///
    /// There is a more convenient function
    /// [`SignatureBuilder::set_key_expiration_time`] that takes an
    /// absolute expiration time.
    ///
    /// [`SignatureBuilder::set_key_expiration_time`]: super::SignatureBuilder::set_key_expiration_time()
    ///
    /// A Key Expiration Time subpacket specifies when the associated
    /// key expires.  This is different from the [Signature Expiration
    /// Time subpacket] (set using
    /// [`SignatureBuilder::set_signature_validity_period`]), which is
    /// used to specify when the signature expires.  That is, in the
    /// former case, the associated key expires, but in the latter
    /// case, the signature itself expires.  This difference is
    /// critical: if a binding signature expires, then an OpenPGP
    /// implementation will still consider the associated key to be
    /// valid if there is another valid binding signature, even if it
    /// is older than the expired signature; if the active binding
    /// signature indicates that the key has expired, then OpenPGP
    /// implementations will not fallback to an older binding
    /// signature.
    ///
    /// [Signature Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    /// [`SignatureBuilder::set_signature_validity_period`]: super::SignatureBuilder::set_signature_validity_period()
    ///
    /// # Examples
    ///
    /// Change all subkeys to expire 10 minutes after their (not the
    /// new binding signature's) creation time.
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///         .generate()?;
    /// let pk = cert.primary_key().key();
    /// let mut signer = pk.clone().parts_into_secret()?.into_keypair()?;
    ///
    /// // Create the binding signatures.
    /// let mut sigs = Vec::new();
    ///
    /// for key in cert.with_policy(p, None)?.keys().subkeys() {
    ///     // This reuses any existing backsignature.
    ///     let sig = SignatureBuilder::from(key.binding_signature().clone())
    ///         .set_key_validity_period(std::time::Duration::new(10 * 60, 0))?
    ///         .sign_subkey_binding(&mut signer, None, &key)?;
    ///     sigs.push(sig);
    /// }
    ///
    /// let cert = cert.insert_packets(sigs)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// #
    /// # // "Before"
    /// # for key in cert.with_policy(p, None)?.keys().subkeys() {
    /// #     assert_eq!(key.bundle().self_signatures().len(), 2);
    /// #     assert!(key.alive().is_ok());
    /// # }
    /// #
    /// # // "After"
    /// # for key in cert
    /// #     .with_policy(p, std::time::SystemTime::now()
    /// #         + std::time::Duration::new(20 * 60, 0))?
    /// #     .keys().subkeys()
    /// # {
    /// #     assert!(key.alive().is_err());
    /// # }
    /// # Ok(()) }
    /// ```
    pub fn set_key_validity_period<D>(mut self, expires_in: D)
        -> Result<Self>
        where D: Into<Option<time::Duration>>
    {
        if let Some(e) = expires_in.into() {
            self.hashed_area.replace(Subpacket::new(
                SubpacketValue::KeyExpirationTime(e.try_into()?),
                true)?)?;
        } else {
            self.hashed_area.remove_all(SubpacketTag::KeyExpirationTime);
        }

        Ok(self)
    }

    /// Sets the Key Expiration Time subpacket.
    ///
    /// Adds a [Key Expiration Time subpacket] to the hashed subpacket
    /// area.  This function first removes any Key Expiration Time
    /// subpacket from the hashed subpacket area.
    ///
    /// If `None` is given, any expiration subpacket is removed.
    ///
    /// [Key Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    ///
    /// This function is called `set_key_expiration_time` similar to
    /// the subpacket's name, but it takes an absolute time, whereas
    /// the subpacket stores a time relative to the associated key's
    /// (*not* the signature's) creation time, which is stored in the
    /// [Key].
    ///
    /// [Key]: https://tools.ietf.org/html/rfc4880#section-5.5.2
    ///
    /// This is a more convenient function than
    /// [`SignatureBuilder::set_key_validity_period`] that takes a
    /// relative expiration time.
    ///
    /// [`SignatureBuilder::set_key_validity_period`]: super::SignatureBuilder::set_key_validity_period()
    ///
    /// A Key Expiration Time subpacket specifies when the associated
    /// key expires.  This is different from the [Signature Expiration
    /// Time subpacket] (set using
    /// [`SignatureBuilder::set_signature_validity_period`]), which is
    /// used to specify when the signature expires.  That is, in the
    /// former case, the associated key expires, but in the latter
    /// case, the signature itself expires.  This difference is
    /// critical: if a binding signature expires, then an OpenPGP
    /// implementation will still consider the associated key to be
    /// valid if there is another valid binding signature, even if it
    /// is older than the expired signature; if the active binding
    /// signature indicates that the key has expired, then OpenPGP
    /// implementations will not fallback to an older binding
    /// signature.
    ///
    /// [Signature Expiration Time subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.6
    /// [`SignatureBuilder::set_signature_validity_period`]: super::SignatureBuilder::set_signature_validity_period()
    ///
    /// # Examples
    ///
    /// Change all subkeys to expire 10 minutes after their (not the
    /// new binding signature's) creation time.
    ///
    /// ```
    /// use std::time;
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///         .generate()?;
    /// let pk = cert.primary_key().key();
    /// let mut signer = pk.clone().parts_into_secret()?.into_keypair()?;
    ///
    /// // Create the binding signatures.
    /// let mut sigs = Vec::new();
    ///
    /// for key in cert.with_policy(p, None)?.keys().subkeys() {
    ///     // This reuses any existing backsignature.
    ///     let sig = SignatureBuilder::from(key.binding_signature().clone())
    ///         .set_key_expiration_time(&key,
    ///                                  time::SystemTime::now()
    ///                                  + time::Duration::new(10 * 60, 0))?
    ///         .sign_subkey_binding(&mut signer, None, &key)?;
    ///     sigs.push(sig);
    /// }
    ///
    /// let cert = cert.insert_packets(sigs)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// #
    /// # // "Before"
    /// # for key in cert.with_policy(p, None)?.keys().subkeys() {
    /// #     assert_eq!(key.bundle().self_signatures().len(), 2);
    /// #     assert!(key.alive().is_ok());
    /// # }
    /// #
    /// # // "After"
    /// # for key in cert.with_policy(p, time::SystemTime::now()
    /// #         + time::Duration::new(20 * 60, 0))?
    /// #     .keys().subkeys()
    /// # {
    /// #     assert!(key.alive().is_err());
    /// # }
    /// # Ok(()) }
    /// ```
    pub fn set_key_expiration_time<P, R, E>(
        self,
        key: &Key<P, R>,
        expiration: E)
        -> Result<Self>
        where P: key::KeyParts,
              R: key::KeyRole,
              E: Into<Option<time::SystemTime>>,
    {
        if let Some(e) = expiration.into()
            .map(crate::types::normalize_systemtime)
        {
            let ct = key.creation_time();
            let vp = match e.duration_since(ct) {
                Ok(v) => v,
                Err(_) => return Err(Error::InvalidArgument(
                    format!("Expiration time {:?} predates creation time \
                             {:?}", e, ct)).into()),
            };

            self.set_key_validity_period(Some(vp))
        } else {
            self.set_key_validity_period(None)
        }
    }

    /// Sets the Preferred Symmetric Algorithms subpacket.
    ///
    /// Replaces any [Preferred Symmetric Algorithms subpacket] in the
    /// hashed subpacket area with a new subpacket containing the
    /// specified value.  That is, this function first removes any
    /// Preferred Symmetric Algorithms subpacket from the hashed
    /// subpacket area, and then adds a new one.
    ///
    /// A Preferred Symmetric Algorithms subpacket lists what
    /// symmetric algorithms the user prefers.  When encrypting a
    /// message for a recipient, the OpenPGP implementation should not
    /// use an algorithm that is not on this list.
    ///
    /// [Preferred Symmetric Algorithms subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.7
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::SymmetricAlgorithm;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().add_userid("Alice").generate()?;
    /// let mut signer = cert.primary_key().key()
    ///     .clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let vc = cert.with_policy(p, None)?;
    ///
    /// let template = vc.direct_key_signature()
    ///     .expect("CertBuilder always includes a direct key signature");
    /// let sig = SignatureBuilder::from(template.clone())
    ///     .set_preferred_symmetric_algorithms(
    ///         vec![ SymmetricAlgorithm::AES256,
    ///               SymmetricAlgorithm::AES128,
    ///         ])?
    ///     .sign_direct_key(&mut signer, None)?;
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::PreferredSymmetricAlgorithms)
    /// #    .count(),
    /// #    1);
    ///
    /// // Merge in the new signature.
    /// let cert = cert.insert_packets(sig)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// # Ok(()) }
    /// ```
    pub fn set_preferred_symmetric_algorithms(mut self,
                                              preferences: Vec<SymmetricAlgorithm>)
                                              -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::PreferredSymmetricAlgorithms(preferences),
            false)?)?;

        Ok(self)
    }

    /// Sets the Revocation Key subpacket.
    ///
    /// Replaces any [Revocation Key subpacket] in the hashed
    /// subpacket area with a new subpacket containing the specified
    /// value.  That is, this function first removes any Revocation
    /// Key subpacket from the hashed subpacket area, and then adds a
    /// new one.
    ///
    /// [Revocation Key subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.15
    ///
    /// A Revocation Key subpacket indicates certificates (so-called
    /// designated revokers) that are allowed to revoke the signer's
    /// certificate.  For instance, if Alice trusts Bob, she can set
    /// him as a designated revoker.  This is useful if Alice loses
    /// access to her key, and therefore is unable to generate a
    /// revocation certificate on her own.  In this case, she can
    /// still Bob to generate one on her behalf.
    ///
    /// Due to the complexity of verifying such signatures, many
    /// OpenPGP implementations do not support this feature.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::RevocationKey;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (alice, _) = CertBuilder::new().add_userid("Alice").generate()?;
    /// let mut alices_signer = alice.primary_key().key()
    ///     .clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let (bob, _) = CertBuilder::new().add_userid("Bob").generate()?;
    ///
    /// let template = alice.with_policy(p, None)?.direct_key_signature()
    ///     .expect("CertBuilder always includes a direct key signature");
    /// let sig = SignatureBuilder::from(template.clone())
    ///     .set_revocation_key(vec![
    ///         RevocationKey::new(bob.primary_key().pk_algo(), bob.fingerprint(), false),
    ///     ])?
    ///     .sign_direct_key(&mut alices_signer, None)?;
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::RevocationKey)
    /// #    .count(),
    /// #    1);
    ///
    /// // Merge in the new signature.
    /// let alice = alice.insert_packets(sig)?;
    /// # assert_eq!(alice.bad_signatures().count(), 0);
    /// # assert_eq!(alice.primary_key().self_signatures().count(), 2);
    /// # Ok(()) }
    /// ```
    pub fn set_revocation_key(mut self, rk: Vec<RevocationKey>) -> Result<Self> {
        self.hashed_area.remove_all(SubpacketTag::RevocationKey);
        for rk in rk.into_iter() {
            self.hashed_area.add(Subpacket::new(
                SubpacketValue::RevocationKey(rk),
                true)?)?;
        }

        Ok(self)
    }

    /// Adds the Issuer subpacket.
    ///
    /// Adds an [Issuer subpacket] to the hashed subpacket area.
    /// Unlike [`add_issuer`], this function first removes any
    /// existing Issuer subpackets from the hashed and unhashed
    /// subpacket area.
    ///
    ///   [Issuer subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///   [`add_issuer`]: super::SignatureBuilder::add_issuer()
    ///
    /// The Issuer subpacket is used when processing a signature to
    /// identify which certificate created the signature.  Even though this
    /// information is self-authenticating (the act of validating the
    /// signature authenticates the subpacket), it is stored in the
    /// hashed subpacket area.  This has the advantage that the signer
    /// authenticates the set of issuers.  Furthermore, it makes
    /// handling of the resulting signatures more robust: If there are
    /// two two signatures that are equal modulo the contents of the
    /// unhashed area, there is the question of how to merge the
    /// information in the unhashed areas.  Storing issuer information
    /// in the hashed area avoids this problem.
    ///
    /// When creating a signature using a SignatureBuilder or the
    /// [streaming `Signer`], it is not necessary to explicitly set
    /// this subpacket: those functions automatically set both the
    /// [Issuer Fingerprint subpacket] (set using
    /// [`SignatureBuilder::set_issuer_fingerprint`]) and the Issuer
    /// subpacket, if they have not been set explicitly.
    ///
    /// [streaming `Signer`]: crate::serialize::stream::Signer
    /// [Issuer Fingerprint subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    /// [`SignatureBuilder::set_issuer_fingerprint`]: super::SignatureBuilder::set_issuer_fingerprint()
    ///
    /// # Examples
    ///
    /// It is possible to use the same key material with different
    /// OpenPGP keys.  This is useful when the OpenPGP format is
    /// upgraded, but not all deployed implementations support the new
    /// format.  Here, Alice signs a message, and adds the fingerprint
    /// of her v4 key and her v5 key indicating that the recipient can
    /// use either key to verify the message:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let (alicev4, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// # let mut alices_signer = alicev4.primary_key().key().clone().parts_into_secret()?.into_keypair()?;
    /// # let (alicev5, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// #
    /// let msg = b"Hi!";
    ///
    /// let sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .set_issuer(alicev4.keyid())?
    ///     .add_issuer(alicev5.keyid())?
    ///     .sign_message(&mut alices_signer, msg)?;
    /// # let mut sig = sig;
    /// # assert!(sig.verify_message(alices_signer.public(), msg).is_ok());
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::Issuer)
    /// #    .count(),
    /// #    2);
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::IssuerFingerprint)
    /// #    .count(),
    /// #    0);
    /// # Ok(()) }
    /// ```
    pub fn set_issuer(mut self, id: KeyID) -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::Issuer(id),
            false)?)?;
        self.unhashed_area.remove_all(SubpacketTag::Issuer);

        Ok(self)
    }

    /// Adds an Issuer subpacket.
    ///
    /// Adds an [Issuer subpacket] to the hashed subpacket area.
    /// Unlike [`set_issuer`], this function does not first remove any
    /// existing Issuer subpacket from neither the hashed nor the
    /// unhashed subpacket area.
    ///
    ///   [Issuer subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    ///   [`set_issuer`]: super::SignatureBuilder::set_issuer()
    ///
    /// The Issuer subpacket is used when processing a signature to
    /// identify which certificate created the signature.  Even though this
    /// information is self-authenticating (the act of validating the
    /// signature authenticates the subpacket), it is stored in the
    /// hashed subpacket area.  This has the advantage that the signer
    /// authenticates the set of issuers.  Furthermore, it makes
    /// handling of the resulting signatures more robust: If there are
    /// two two signatures that are equal modulo the contents of the
    /// unhashed area, there is the question of how to merge the
    /// information in the unhashed areas.  Storing issuer information
    /// in the hashed area avoids this problem.
    ///
    /// When creating a signature using a SignatureBuilder or the
    /// [streaming `Signer`], it is not necessary to explicitly set
    /// this subpacket: those functions automatically set both the
    /// [Issuer Fingerprint subpacket] (set using
    /// [`SignatureBuilder::set_issuer_fingerprint`]) and the Issuer
    /// subpacket, if they have not been set explicitly.
    ///
    /// [streaming `Signer`]: crate::serialize::stream::Signer
    /// [Issuer Fingerprint subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    /// [`SignatureBuilder::set_issuer_fingerprint`]: super::SignatureBuilder::set_issuer_fingerprint()
    ///
    /// # Examples
    ///
    /// It is possible to use the same key material with different
    /// OpenPGP keys.  This is useful when the OpenPGP format is
    /// upgraded, but not all deployed implementations support the new
    /// format.  Here, Alice signs a message, and adds the fingerprint
    /// of her v4 key and her v5 key indicating that the recipient can
    /// use either key to verify the message:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let (alicev4, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// # let mut alices_signer = alicev4.primary_key().key().clone().parts_into_secret()?.into_keypair()?;
    /// # let (alicev5, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// #
    /// let msg = b"Hi!";
    ///
    /// let sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .set_issuer(alicev4.keyid())?
    ///     .add_issuer(alicev5.keyid())?
    ///     .sign_message(&mut alices_signer, msg)?;
    /// # let mut sig = sig;
    /// # assert!(sig.verify_message(alices_signer.public(), msg).is_ok());
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::Issuer)
    /// #    .count(),
    /// #    2);
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::IssuerFingerprint)
    /// #    .count(),
    /// #    0);
    /// # Ok(()) }
    /// ```
    pub fn add_issuer(mut self, id: KeyID) -> Result<Self> {
        self.hashed_area.add(Subpacket::new(
            SubpacketValue::Issuer(id),
            false)?)?;

        Ok(self)
    }

    /// Sets a Notation Data subpacket.
    ///
    /// Adds a [Notation Data subpacket] to the hashed subpacket area.
    /// Unlike the [`SignatureBuilder::add_notation`] method, this
    /// function first removes any existing Notation Data subpacket
    /// with the specified name from the hashed subpacket area.
    ///
    /// [Notation Data subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    /// [`SignatureBuilder::add_notation`]: super::SignatureBuilder::add_notation()
    ///
    /// Notations are key-value pairs.  They can be used by
    /// applications to annotate signatures in a structured way.  For
    /// instance, they can define additional, application-specific
    /// security requirements.  Because they are functionally
    /// equivalent to subpackets, they can also be used for OpenPGP
    /// extensions.  This is how the [Intended Recipient subpacket]
    /// started life.
    ///
    /// [Intended Recipient subpacket]:https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#name-intended-recipient-fingerpr
    ///
    /// Notation names are structured, and are divided into two
    /// namespaces: the user namespace and the IETF namespace.  Names
    /// in the user namespace have the form `name@example.org` and
    /// their meaning is defined by the owner of the domain.  The
    /// meaning of the notation `name@example.org`, for instance, is
    /// defined by whoever controls `example.org`.  Names in the IETF
    /// namespace do not contain an `@` and are managed by IANA.  See
    /// [Section 5.2.3.16 of RFC 4880] for details.
    ///
    /// [Section 5.2.3.16 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    ///
    /// # Examples
    ///
    /// Adds two [social proofs] to a certificate's primary User ID.
    /// This first clears any social proofs.
    ///
    /// [social proofs]: https://metacode.biz/openpgp/proofs
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().add_userid("Wiktor").generate()?;
    /// let mut signer = cert.primary_key().key()
    ///     .clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let vc = cert.with_policy(p, None)?;
    /// let userid = vc.primary_userid().expect("Added a User ID");
    ///
    /// let template = userid.binding_signature();
    /// let sig = SignatureBuilder::from(template.clone())
    ///     .set_notation("proof@metacode.biz", "https://metacode.biz/@wiktor",
    ///                   NotationDataFlags::empty().set_human_readable(), false)?
    ///     .add_notation("proof@metacode.biz", "https://news.ycombinator.com/user?id=wiktor-k",
    ///                   NotationDataFlags::empty().set_human_readable(), false)?
    ///     .sign_userid_binding(&mut signer, None, &userid)?;
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::NotationData)
    /// #    .count(),
    /// #    3);
    ///
    /// // Merge in the new signature.
    /// let cert = cert.insert_packets(sig)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// # Ok(()) }
    /// ```
    pub fn set_notation<N, V, F>(mut self, name: N, value: V, flags: F,
                                 critical: bool)
                                 -> Result<Self>
        where N: AsRef<str>,
              V: AsRef<[u8]>,
              F: Into<Option<NotationDataFlags>>,
    {
        self.hashed_area.packets.retain(|s| {
            ! matches!(
                s.value,
                SubpacketValue::NotationData(ref v) if v.name == name.as_ref())
        });
        self.add_notation(name.as_ref(), value.as_ref(),
                          flags.into().unwrap_or_else(NotationDataFlags::empty),
                          critical)
    }

    /// Adds a Notation Data subpacket.
    ///
    /// Adds a [Notation Data subpacket] to the hashed subpacket area.
    /// Unlike the [`SignatureBuilder::set_notation`] method, this
    /// function does not first remove any existing Notation Data
    /// subpacket with the specified name from the hashed subpacket
    /// area.
    ///
    /// [Notation Data subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    /// [`SignatureBuilder::set_notation`]: super::SignatureBuilder::set_notation()
    ///
    /// Notations are key-value pairs.  They can be used by
    /// applications to annotate signatures in a structured way.  For
    /// instance, they can define additional, application-specific
    /// security requirements.  Because they are functionally
    /// equivalent to subpackets, they can also be used for OpenPGP
    /// extensions.  This is how the [Intended Recipient subpacket]
    /// started life.
    ///
    /// [Intended Recipient subpacket]:https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#name-intended-recipient-fingerpr
    ///
    /// Notation names are structured, and are divided into two
    /// namespaces: the user namespace and the IETF namespace.  Names
    /// in the user namespace have the form `name@example.org` and
    /// their meaning is defined by the owner of the domain.  The
    /// meaning of the notation `name@example.org`, for instance, is
    /// defined by whoever controls `example.org`.  Names in the IETF
    /// namespace do not contain an `@` and are managed by IANA.  See
    /// [Section 5.2.3.16 of RFC 4880] for details.
    ///
    /// [Section 5.2.3.16 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.16
    ///
    /// # Examples
    ///
    /// Adds two new [social proofs] to a certificate's primary User
    /// ID.  A more sophisticated program will check that the new
    /// notations aren't already present.
    ///
    /// [social proofs]: https://metacode.biz/openpgp/proofs
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().add_userid("Wiktor").generate()?;
    /// let mut signer = cert.primary_key().key()
    ///     .clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let vc = cert.with_policy(p, None)?;
    /// let userid = vc.primary_userid().expect("Added a User ID");
    ///
    /// let template = userid.binding_signature();
    /// let sig = SignatureBuilder::from(template.clone())
    ///     .add_notation("proof@metacode.biz", "https://metacode.biz/@wiktor",
    ///                   NotationDataFlags::empty().set_human_readable(), false)?
    ///     .add_notation("proof@metacode.biz", "https://news.ycombinator.com/user?id=wiktor-k",
    ///                   NotationDataFlags::empty().set_human_readable(), false)?
    ///     .sign_userid_binding(&mut signer, None, &userid)?;
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::NotationData)
    /// #    .count(),
    /// #    3);
    ///
    /// // Merge in the new signature.
    /// let cert = cert.insert_packets(sig)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// # Ok(()) }
    /// ```
    pub fn add_notation<N, V, F>(mut self, name: N, value: V, flags: F,
                           critical: bool)
                           -> Result<Self>
        where N: AsRef<str>,
              V: AsRef<[u8]>,
              F: Into<Option<NotationDataFlags>>,
    {
        self.hashed_area.add(Subpacket::new(SubpacketValue::NotationData(
            NotationData::new(name.as_ref(), value.as_ref(),
                              flags.into().unwrap_or_else(NotationDataFlags::empty))),
                                            critical)?)?;
        Ok(self)
    }

    /// Sets the Preferred Hash Algorithms subpacket.
    ///
    /// Replaces any [Preferred Hash Algorithms subpacket] in the
    /// hashed subpacket area with a new subpacket containing the
    /// specified value.  That is, this function first removes any
    /// Preferred Hash Algorithms subpacket from the hashed subpacket
    /// area, and then adds a new one.
    ///
    /// A Preferred Hash Algorithms subpacket lists what hash
    /// algorithms the user prefers.  When signing a message that
    /// should be verified by a particular recipient, the OpenPGP
    /// implementation should not use an algorithm that is not on this
    /// list.
    ///
    /// [Preferred Hash Algorithms subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.8
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::HashAlgorithm;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().add_userid("Alice").generate()?;
    /// let mut signer = cert.primary_key().key()
    ///     .clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let vc = cert.with_policy(p, None)?;
    ///
    /// let template = vc.direct_key_signature()
    ///     .expect("CertBuilder always includes a direct key signature");
    /// let sig = SignatureBuilder::from(template.clone())
    ///     .set_preferred_hash_algorithms(
    ///         vec![ HashAlgorithm::SHA512,
    ///               HashAlgorithm::SHA256,
    ///         ])?
    ///     .sign_direct_key(&mut signer, None)?;
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::PreferredHashAlgorithms)
    /// #    .count(),
    /// #    1);
    ///
    /// // Merge in the new signature.
    /// let cert = cert.insert_packets(sig)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// # Ok(()) }
    /// ```
    pub fn set_preferred_hash_algorithms(mut self,
                                         preferences: Vec<HashAlgorithm>)
                                         -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::PreferredHashAlgorithms(preferences),
            false)?)?;

        Ok(self)
    }

    /// Sets the Preferred Compression Algorithms subpacket.
    ///
    /// Replaces any [Preferred Compression Algorithms subpacket] in
    /// the hashed subpacket area with a new subpacket containing the
    /// specified value.  That is, this function first removes any
    /// Preferred Compression Algorithms subpacket from the hashed
    /// subpacket area, and then adds a new one.
    ///
    /// A Preferred Compression Algorithms subpacket lists what
    /// compression algorithms the user prefers.  When compressing a
    /// message for a recipient, the OpenPGP implementation should not
    /// use an algorithm that is not on the list.
    ///
    /// [Preferred Compression Algorithms subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.9
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::CompressionAlgorithm;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().add_userid("Alice").generate()?;
    /// let mut signer = cert.primary_key().key()
    ///     .clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let vc = cert.with_policy(p, None)?;
    ///
    /// let template = vc.direct_key_signature()
    ///     .expect("CertBuilder always includes a direct key signature");
    /// let sig = SignatureBuilder::from(template.clone())
    ///     .set_preferred_compression_algorithms(
    ///         vec![ CompressionAlgorithm::Zlib,
    ///               CompressionAlgorithm::Zip,
    ///               CompressionAlgorithm::BZip2,
    ///         ])?
    ///     .sign_direct_key(&mut signer, None)?;
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::PreferredCompressionAlgorithms)
    /// #    .count(),
    /// #    1);
    ///
    /// // Merge in the new signature.
    /// let cert = cert.insert_packets(sig)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// # Ok(()) }
    /// ```
    pub fn set_preferred_compression_algorithms(mut self,
                                                preferences: Vec<CompressionAlgorithm>)
                                                -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::PreferredCompressionAlgorithms(preferences),
            false)?)?;

        Ok(self)
    }

    /// Sets the Key Server Preferences subpacket.
    ///
    /// Replaces any [Key Server Preferences subpacket] in the hashed
    /// subpacket area with a new subpacket containing the specified
    /// value.  That is, this function first removes any Key Server
    /// Preferences subpacket from the hashed subpacket area, and then
    /// adds a new one.
    ///
    /// The Key Server Preferences subpacket indicates to key servers
    /// how they should handle the certificate.
    ///
    /// [Key Server Preferences subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.17
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::KeyServerPreferences;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().add_userid("Alice").generate()?;
    /// let mut signer = cert.primary_key().key()
    ///     .clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let vc = cert.with_policy(p, None)?;
    ///
    /// let sig = vc.direct_key_signature()
    ///     .expect("CertBuilder always includes a direct key signature");
    /// let sig =
    ///     SignatureBuilder::from(sig.clone())
    ///         .set_key_server_preferences(
    ///             KeyServerPreferences::empty().set_no_modify())?
    ///         .sign_direct_key(&mut signer, None)?;
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::KeyServerPreferences)
    /// #    .count(),
    /// #    1);
    ///
    /// // Merge in the new signature.
    /// let cert = cert.insert_packets(sig)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// # Ok(()) }
    /// ```
    pub fn set_key_server_preferences(mut self,
                                      preferences: KeyServerPreferences)
                                      -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::KeyServerPreferences(preferences),
            false)?)?;

        Ok(self)
    }

    /// Sets the Preferred Key Server subpacket.
    ///
    /// Adds a [Preferred Key Server subpacket] to the hashed
    /// subpacket area.  This function first removes any Preferred Key
    /// Server subpacket from the hashed subpacket area.
    ///
    /// [Preferred Key Server subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.18
    ///
    /// The Preferred Key Server subpacket contains a link to a key
    /// server where the certificate holder plans to publish updates
    /// to their certificate (e.g., extensions to the expiration time,
    /// new subkeys, revocation certificates).
    ///
    /// The Preferred Key Server subpacket should be handled
    /// cautiously, because it can be used by a certificate holder to
    /// track communication partners.
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().add_userid("Alice").generate()?;
    /// let mut signer = cert.primary_key().key()
    ///     .clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let vc = cert.with_policy(p, None)?;
    ///
    /// let sig = vc.direct_key_signature()
    ///     .expect("CertBuilder always includes a direct key signature");
    /// let sig =
    ///     SignatureBuilder::from(sig.clone())
    ///         .set_preferred_key_server(&"https://keys.openpgp.org")?
    ///         .sign_direct_key(&mut signer, None)?;
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::PreferredKeyServer)
    /// #    .count(),
    /// #    1);
    ///
    /// // Merge in the new signature.
    /// let cert = cert.insert_packets(sig)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// # Ok(()) }
    /// ```
    pub fn set_preferred_key_server<U>(mut self, uri: U)
                                       -> Result<Self>
        where U: AsRef<[u8]>,
    {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::PreferredKeyServer(uri.as_ref().to_vec()),
            false)?)?;

        Ok(self)
    }

    /// Sets the Primary User ID subpacket.
    ///
    /// Adds a [Primary User ID subpacket] to the hashed subpacket
    /// area.  This function first removes any Primary User ID
    /// subpacket from the hashed subpacket area.
    ///
    /// [Primary User ID subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.19
    ///
    /// The Primary User ID subpacket indicates whether the associated
    /// User ID or User Attribute should be considered the primary
    /// User ID.  It is possible that this is set on multiple User
    /// IDs.  See the documentation for [`ValidCert::primary_userid`] for
    /// an explanation of how Sequoia resolves this ambiguity.
    ///
    /// [`ValidCert::primary_userid`]: crate::cert::ValidCert::primary_userid()
    ///
    /// # Examples
    ///
    /// Change the primary User ID:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let club = "Alice <alice@club.org>";
    /// let home = "Alice <alice@home.org>";
    ///
    /// // CertBuilder makes the first User ID (club) the primary User ID.
    /// let (cert, _) = CertBuilder::new()
    /// #   // Create it in the past.
    /// #   .set_creation_time(std::time::SystemTime::now()
    /// #       - std::time::Duration::new(10, 0))
    ///     .add_userid(club)
    ///     .add_userid(home)
    ///     .generate()?;
    /// # assert_eq!(cert.userids().count(), 2);
    /// assert_eq!(cert.with_policy(p, None)?.primary_userid().unwrap().userid(),
    ///            &UserID::from(club));
    ///
    /// // Make the `home` User ID the primary User ID.
    ///
    /// // Derive a signer.
    /// let pk = cert.primary_key().key();
    /// let mut signer = pk.clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let mut sig = None;
    /// for ua in cert.with_policy(p, None)?.userids() {
    ///     if ua.userid() == &UserID::from(home) {
    ///         sig = Some(SignatureBuilder::from(ua.binding_signature().clone())
    ///             .set_primary_userid(true)?
    ///             .sign_userid_binding(&mut signer, pk, ua.userid())?);
    ///         # assert_eq!(sig.as_ref().unwrap()
    ///         #    .hashed_area()
    ///         #    .iter()
    ///         #    .filter(|sp| sp.tag() == SubpacketTag::PrimaryUserID)
    ///         #    .count(),
    ///         #    1);
    ///         break;
    ///     }
    /// }
    /// assert!(sig.is_some());
    ///
    /// let cert = cert.insert_packets(sig)?;
    ///
    /// assert_eq!(cert.with_policy(p, None)?.primary_userid().unwrap().userid(),
    ///            &UserID::from(home));
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_primary_userid(mut self, primary: bool) -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::PrimaryUserID(primary),
            true)?)?;

        Ok(self)
    }

    /// Sets the Policy URI subpacket.
    ///
    /// Adds a [Policy URI subpacket] to the hashed subpacket area.
    /// This function first removes any Policy URI subpacket from the
    /// hashed subpacket area.
    ///
    /// [Policy URI subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.20
    ///
    /// The Policy URI subpacket contains a link to a policy document,
    /// which contains information about the conditions under which
    /// the signature was made.
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// # Examples
    ///
    /// Alice updates her direct key signature to include a Policy URI
    /// subpacket:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (alice, _) = CertBuilder::new().add_userid("Alice").generate()?;
    /// let pk = alice.primary_key().key();
    /// let mut signer = pk.clone().parts_into_secret()?.into_keypair()?;
    ///
    /// let sig = SignatureBuilder::from(
    ///     alice
    ///         .with_policy(p, None)?
    ///         .direct_key_signature().expect("Direct key signature")
    ///         .clone()
    ///     )
    ///     .set_policy_uri("https://example.org/~alice/signing-policy.txt")?
    ///     .sign_direct_key(&mut signer, None)?;
    /// # let mut sig = sig;
    /// # sig.verify_direct_key(signer.public(), pk)?;
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::PolicyURI)
    /// #    .count(),
    /// #    1);
    ///
    /// // Merge it into the certificate.
    /// let alice = alice.insert_packets(sig)?;
    /// #
    /// # assert_eq!(alice.bad_signatures().count(), 0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_policy_uri<U>(mut self, uri: U) -> Result<Self>
        where U: AsRef<[u8]>,
    {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::PolicyURI(uri.as_ref().to_vec()),
            false)?)?;

        Ok(self)
    }

    /// Sets the Key Flags subpacket.
    ///
    /// Adds a [Key Flags subpacket] to the hashed subpacket area.
    /// This function first removes any Key Flags subpacket from the
    /// hashed subpacket area.
    ///
    /// [Key Flags subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.21
    ///
    /// The Key Flags subpacket describes a key's capabilities
    /// (certification capable, signing capable, etc.).  In the case
    /// of subkeys, the Key Flags are located on the subkey's binding
    /// signature.  For primary keys, locating the correct Key Flags
    /// subpacket is more complex: First, the primary User ID is
    /// consulted.  If the primary User ID contains a Key Flags
    /// subpacket, that is used.  Otherwise, any direct key signature
    /// is considered.  If that still doesn't contain a Key Flags
    /// packet, then the primary key should be assumed to be
    /// certification capable.
    ///
    /// # Examples
    ///
    /// Adds a new subkey, which is intended for encrypting data at
    /// rest, to a certificate:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::{
    ///     Curve,
    ///     KeyFlags,
    ///     SignatureType
    /// };
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// // Generate a Cert, and create a keypair from the primary key.
    /// let (cert, _) = CertBuilder::new().generate()?;
    /// # assert_eq!(cert.keys().with_policy(p, None).alive().revoked(false)
    /// #                .key_flags(&KeyFlags::empty().set_storage_encryption()).count(),
    /// #            0);
    /// let mut signer = cert.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    ///
    /// // Generate a subkey and a binding signature.
    /// let subkey: Key<_, key::SubordinateRole>
    ///     = Key4::generate_ecc(false, Curve::Cv25519)?
    ///         .into();
    /// let builder = signature::SignatureBuilder::new(SignatureType::SubkeyBinding)
    ///     .set_key_flags(KeyFlags::empty().set_storage_encryption())?;
    /// let binding = subkey.bind(&mut signer, &cert, builder)?;
    ///
    /// // Now merge the key and binding signature into the Cert.
    /// let cert = cert.insert_packets(vec![Packet::from(subkey),
    ///                                    binding.into()])?;
    ///
    /// # assert_eq!(cert.keys().with_policy(p, None).alive().revoked(false)
    /// #                .key_flags(&KeyFlags::empty().set_storage_encryption()).count(),
    /// #            1);
    /// # Ok(()) }
    /// ```
    pub fn set_key_flags(mut self, flags: KeyFlags) -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::KeyFlags(flags),
            true)?)?;

        Ok(self)
    }

    /// Sets the Signer's User ID subpacket.
    ///
    /// Adds a [Signer's User ID subpacket] to the hashed subpacket
    /// area.  This function first removes any Signer's User ID
    /// subpacket from the hashed subpacket area.
    ///
    /// [Signer's User ID subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.22
    ///
    /// The Signer's User ID subpacket indicates, which User ID made
    /// the signature.  This is useful when a key has multiple User
    /// IDs, which correspond to different roles.  For instance, it is
    /// not uncommon to use the same certificate in private as well as
    /// for a club.
    ///
    /// # Examples
    ///
    /// Sign a message being careful to set the Signer's User ID
    /// subpacket to the user's private identity and not their club
    /// identity:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let (cert, _) = CertBuilder::new()
    ///     .add_userid("Alice <alice@home.org>")
    ///     .add_userid("Alice (President) <alice@club.org>")
    ///     .generate()?;
    /// let mut signer = cert.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    ///
    /// let msg = "Speaking for myself, I agree.";
    ///
    /// let sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .set_signers_user_id(&b"Alice <alice@home.org>"[..])?
    ///     .sign_message(&mut signer, msg)?;
    /// # let mut sig = sig;
    /// # assert!(sig.verify_message(signer.public(), msg).is_ok());
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::SignersUserID)
    /// #    .count(),
    /// #    1);
    /// # Ok(()) }
    /// ```
    pub fn set_signers_user_id<U>(mut self, uid: U) -> Result<Self>
        where U: AsRef<[u8]>,
    {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::SignersUserID(uid.as_ref().to_vec()),
            false)?)?;

        Ok(self)
    }

    /// Sets the value of the Reason for Revocation subpacket.
    ///
    /// Adds a [Reason For Revocation subpacket] to the hashed
    /// subpacket area.  This function first removes any Reason For
    /// Revocation subpacket from the hashed subpacket
    /// area.
    ///
    /// [Reason For Revocation subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.23
    ///
    /// The Reason For Revocation subpacket indicates why a key, User
    /// ID, or User Attribute is being revoked.  It includes both a
    /// machine readable code, and a human-readable string.  The code
    /// is essential as it indicates to the OpenPGP implementation
    /// that reads the certificate whether the key was compromised (a
    /// hard revocation), or is no longer used (a soft revocation).
    /// In the former case, the OpenPGP implementation must
    /// conservatively consider all past signatures as suspect whereas
    /// in the latter case, past signatures can still be considered
    /// valid.
    ///
    /// # Examples
    ///
    /// Revoke a certificate whose private key material has been
    /// compromised:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::ReasonForRevocation;
    /// use openpgp::types::RevocationStatus;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, _) = CertBuilder::new().generate()?;
    /// assert_eq!(RevocationStatus::NotAsFarAsWeKnow,
    ///            cert.revocation_status(p, None));
    ///
    /// // Create and sign a revocation certificate.
    /// let mut signer = cert.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    /// let sig = CertRevocationBuilder::new()
    ///     .set_reason_for_revocation(ReasonForRevocation::KeyCompromised,
    ///                                b"It was the maid :/")?
    ///     .build(&mut signer, &cert, None)?;
    ///
    /// // Merge it into the certificate.
    /// let cert = cert.insert_packets(sig.clone())?;
    ///
    /// // Now it's revoked.
    /// assert_eq!(RevocationStatus::Revoked(vec![ &sig ]),
    ///            cert.revocation_status(p, None));
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::ReasonForRevocation)
    /// #    .count(),
    /// #    1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_reason_for_revocation<R>(mut self, code: ReasonForRevocation,
                                        reason: R)
                                        -> Result<Self>
        where R: AsRef<[u8]>,
    {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::ReasonForRevocation {
                code,
                reason: reason.as_ref().to_vec(),
            },
            false)?)?;

        Ok(self)
    }

    /// Sets the Features subpacket.
    ///
    /// Adds a [Feature subpacket] to the hashed subpacket area.  This
    /// function first removes any Feature subpacket from the hashed
    /// subpacket area.
    ///
    /// A Feature subpacket lists what OpenPGP features the user wants
    /// to use.  When creating a message, features that the intended
    /// recipients do not support should not be used.  However,
    /// because this information is rarely held up to date in
    /// practice, this information is only advisory, and
    /// implementations are allowed to infer what features the
    /// recipients support from contextual clues, e.g., their past
    /// behavior.
    ///
    /// [Feature subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.24
    /// [features]: crate::types::Features
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// # Examples
    ///
    /// Update a certificate's binding signatures to indicate support for AEAD:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::{AEADAlgorithm, Features};
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
    ///     sigs.push(
    ///         SignatureBuilder::from(sig.clone())
    ///             .set_preferred_aead_algorithms(vec![ AEADAlgorithm::EAX ])?
    ///             .set_features(
    ///                 sig.features().unwrap_or_else(Features::sequoia)
    ///                     .set_aead())?
    ///             .sign_direct_key(&mut signer, None)?);
    /// }
    ///
    /// for ua in vc.userids() {
    ///     let sig = ua.binding_signature();
    ///     sigs.push(
    ///         SignatureBuilder::from(sig.clone())
    ///             .set_preferred_aead_algorithms(vec![ AEADAlgorithm::EAX ])?
    ///             .set_features(
    ///                 sig.features().unwrap_or_else(Features::sequoia)
    ///                     .set_aead())?
    ///             .sign_userid_binding(&mut signer, pk, ua.userid())?);
    /// }
    ///
    /// // Merge in the new signatures.
    /// let cert = cert.insert_packets(sigs)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_features(mut self, features: Features) -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::Features(features),
            false)?)?;

        Ok(self)
    }

    /// Sets the Signature Target subpacket.
    ///
    /// Adds a [Signature Target subpacket] to the hashed subpacket
    /// area.  This function first removes any Signature Target
    /// subpacket from the hashed subpacket area.
    ///
    /// The Signature Target subpacket is used to identify the target
    /// of a signature.  This is used when revoking a signature, and
    /// by timestamp signatures.  It contains a hash of the target
    /// signature.
    ///
    ///   [Signature Target subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.25
    pub fn set_signature_target<D>(mut self,
                                   pk_algo: PublicKeyAlgorithm,
                                   hash_algo: HashAlgorithm,
                                   digest: D)
                                   -> Result<Self>
        where D: AsRef<[u8]>,
    {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::SignatureTarget {
                pk_algo,
                hash_algo,
                digest: digest.as_ref().to_vec(),
            },
            true)?)?;

        Ok(self)
    }

    /// Sets the value of the Embedded Signature subpacket.
    ///
    /// Adds an [Embedded Signature subpacket] to the hashed
    /// subpacket area.  This function first removes any Embedded
    /// Signature subpacket from both the hashed and the unhashed
    /// subpacket area.
    ///
    /// [Embedded Signature subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.26
    ///
    /// The Embedded Signature subpacket is normally used to hold a
    /// [Primary Key Binding signature], which binds a
    /// signing-capable, authentication-capable, or
    /// certification-capable subkey to the primary key.  Since this
    /// information is self-authenticating, it is usually stored in the
    /// unhashed subpacket area.
    ///
    /// [Primary Key Binding signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///
    /// # Examples
    ///
    /// Add a new signing-capable subkey to a certificate:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::KeyFlags;
    /// use openpgp::types::SignatureType;
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
    /// // Generate a new signing subkey.
    /// let mut subkey: Key<_, _> = Key4::generate_rsa(3072)?.into();
    /// // Derive a signer.
    /// let mut sk_signer = subkey.clone().into_keypair()?;
    ///
    /// // Create the binding signature.
    /// let sig = SignatureBuilder::new(SignatureType::SubkeyBinding)
    ///     .set_key_flags(KeyFlags::empty().set_signing())?
    ///     // And, the backsig.  This is essential for subkeys that create signatures!
    ///     .set_embedded_signature(
    ///         SignatureBuilder::new(SignatureType::PrimaryKeyBinding)
    ///             .sign_primary_key_binding(&mut sk_signer, &pk, &subkey)?)?
    ///     .sign_subkey_binding(&mut pk_signer, None, &subkey)?;
    ///
    /// let cert = cert.insert_packets(vec![Packet::SecretSubkey(subkey),
    ///                                    sig.into()])?;
    ///
    /// assert_eq!(cert.keys().count(), 2);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_embedded_signature(mut self, signature: Signature)
                                  -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::EmbeddedSignature(signature),
            true)?)?;
        self.unhashed_area.remove_all(SubpacketTag::EmbeddedSignature);

        Ok(self)
    }

    /// Sets the Issuer Fingerprint subpacket.
    ///
    /// Adds an [Issuer Fingerprint subpacket] to the hashed
    /// subpacket area.  Unlike [`add_issuer_fingerprint`], this
    /// function first removes any existing Issuer Fingerprint
    /// subpackets from the hashed and unhashed subpacket area.
    ///
    ///   [Issuer Fingerprint subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///   [`add_issuer_fingerprint`]: super::SignatureBuilder::add_issuer_fingerprint()
    ///
    /// The Issuer Fingerprint subpacket is used when processing a
    /// signature to identify which certificate created the signature.
    /// Even though this information is self-authenticating (the act of
    /// validating the signature authenticates the subpacket), it is
    /// stored in the hashed subpacket area.  This has the advantage
    /// that the signer authenticates the set of issuers.
    /// Furthermore, it makes handling of the resulting signatures
    /// more robust: If there are two two signatures that are equal
    /// modulo the contents of the unhashed area, there is the
    /// question of how to merge the information in the unhashed
    /// areas.  Storing issuer information in the hashed area avoids
    /// this problem.
    ///
    /// When creating a signature using a SignatureBuilder or the
    /// [streaming `Signer`], it is not necessary to explicitly set
    /// this subpacket: those functions automatically set both the
    /// Issuer Fingerprint subpacket, and the [Issuer subpacket] (set
    /// using [`SignatureBuilder::set_issuer`]), if they have not been
    /// set explicitly.
    ///
    /// [streaming `Signer`]: crate::serialize::stream::Signer
    /// [Issuer subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    /// [`SignatureBuilder::set_issuer`]: super::SignatureBuilder::set_issuer()
    ///
    /// # Examples
    ///
    /// It is possible to use the same key material with different
    /// OpenPGP keys.  This is useful when the OpenPGP format is
    /// upgraded, but not all deployed implementations support the new
    /// format.  Here, Alice signs a message, and adds the fingerprint
    /// of her v4 key and her v5 key indicating that the recipient can
    /// use either key to verify the message:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let (alicev4, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// # let mut alices_signer = alicev4.primary_key().key().clone().parts_into_secret()?.into_keypair()?;
    /// # let (alicev5, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// #
    /// let msg = b"Hi!";
    ///
    /// let sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .set_issuer_fingerprint(alicev4.fingerprint())?
    ///     .add_issuer_fingerprint(alicev5.fingerprint())?
    ///     .sign_message(&mut alices_signer, msg)?;
    /// # let mut sig = sig;
    /// # assert!(sig.verify_message(alices_signer.public(), msg).is_ok());
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::Issuer)
    /// #    .count(),
    /// #    0);
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::IssuerFingerprint)
    /// #    .count(),
    /// #    2);
    /// # Ok(()) }
    /// ```
    pub fn set_issuer_fingerprint(mut self, fp: Fingerprint) -> Result<Self> {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::IssuerFingerprint(fp),
            false)?)?;
        self.unhashed_area.remove_all(SubpacketTag::IssuerFingerprint);

        Ok(self)
    }

    /// Adds an Issuer Fingerprint subpacket.
    ///
    /// Adds an [Issuer Fingerprint subpacket] to the hashed
    /// subpacket area.  Unlike [`set_issuer_fingerprint`], this
    /// function does not first remove any existing Issuer Fingerprint
    /// subpacket from neither the hashed nor the unhashed subpacket
    /// area.
    ///
    ///   [Issuer Fingerprint subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.28
    ///   [`set_issuer_fingerprint`]: super::SignatureBuilder::set_issuer_fingerprint()
    ///
    /// The Issuer Fingerprint subpacket is used when processing a
    /// signature to identify which certificate created the signature.
    /// Even though this information is self-authenticating (the act of
    /// validating the signature authenticates the subpacket), it is
    /// stored in the hashed subpacket area.  This has the advantage
    /// that the signer authenticates the set of issuers.
    /// Furthermore, it makes handling of the resulting signatures
    /// more robust: If there are two two signatures that are equal
    /// modulo the contents of the unhashed area, there is the
    /// question of how to merge the information in the unhashed
    /// areas.  Storing issuer information in the hashed area avoids
    /// this problem.
    ///
    /// When creating a signature using a SignatureBuilder or the
    /// [streaming `Signer`], it is not necessary to explicitly set
    /// this subpacket: those functions automatically set both the
    /// Issuer Fingerprint subpacket, and the [Issuer subpacket] (set
    /// using [`SignatureBuilder::set_issuer`]), if they have not been
    /// set explicitly.
    ///
    /// [streaming `Signer`]: crate::serialize::stream::Signer
    /// [Issuer subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.5
    /// [`SignatureBuilder::set_issuer`]: super::SignatureBuilder::set_issuer()
    ///
    /// # Examples
    ///
    /// It is possible to use the same key material with different
    /// OpenPGP keys.  This is useful when the OpenPGP format is
    /// upgraded, but not all deployed implementations support the new
    /// format.  Here, Alice signs a message, and adds the fingerprint
    /// of her v4 key and her v5 key indicating that the recipient can
    /// use either key to verify the message:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// # use openpgp::packet::signature::subpacket::SubpacketTag;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let (alicev4, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// # let mut alices_signer = alicev4.primary_key().key().clone().parts_into_secret()?.into_keypair()?;
    /// # let (alicev5, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// #
    /// let msg = b"Hi!";
    ///
    /// let sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .set_issuer_fingerprint(alicev4.fingerprint())?
    ///     .add_issuer_fingerprint(alicev5.fingerprint())?
    ///     .sign_message(&mut alices_signer, msg)?;
    /// # let mut sig = sig;
    /// # assert!(sig.verify_message(alices_signer.public(), msg).is_ok());
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::Issuer)
    /// #    .count(),
    /// #    0);
    /// # assert_eq!(sig
    /// #    .hashed_area()
    /// #    .iter()
    /// #    .filter(|sp| sp.tag() == SubpacketTag::IssuerFingerprint)
    /// #    .count(),
    /// #    2);
    /// # Ok(()) }
    /// ```
    pub fn add_issuer_fingerprint(mut self, fp: Fingerprint) -> Result<Self> {
        self.hashed_area.add(Subpacket::new(
            SubpacketValue::IssuerFingerprint(fp),
            false)?)?;

        Ok(self)
    }

    /// Sets the Preferred AEAD Algorithms subpacket.
    ///
    /// Replaces any [Preferred AEAD Algorithms subpacket] in the
    /// hashed subpacket area with a new subpacket containing the
    /// specified value.  That is, this function first removes any
    /// Preferred AEAD Algorithms subpacket from the hashed subpacket
    /// area, and then adds a Preferred AEAD Algorithms subpacket.
    ///
    /// [Preferred AEAD Algorithms subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.8
    ///
    /// The Preferred AEAD Algorithms subpacket indicates what AEAD
    /// algorithms the key holder prefers ordered by preference.  If
    /// this is set, then the AEAD feature flag should in the
    /// [Features subpacket] should also be set.
    ///
    /// Note: because support for AEAD has not yet been standardized,
    /// we recommend not yet advertising support for it.
    ///
    /// [Features subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.25
    ///
    /// This subpacket is a type of preference.  When looking up a
    /// preference, an OpenPGP implementation should first look for
    /// the subpacket on the binding signature of the User ID or the
    /// User Attribute used to locate the certificate (or the primary
    /// User ID, if it was addressed by Key ID or fingerprint).  If
    /// the binding signature doesn't contain the subpacket, then the
    /// direct key signature should be checked.  See the
    /// [`Preferences`] trait for details.
    ///
    /// Unless addressing different User IDs really should result in
    /// different behavior, it is best to only set this preference on
    /// the direct key signature.  This guarantees that even if some
    /// or all User IDs are stripped, the behavior remains consistent.
    ///
    /// [`Preferences`]: crate::cert::Preferences
    ///
    /// # Examples
    ///
    /// Update a certificate's binding signatures to indicate support for AEAD:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::{AEADAlgorithm, Features};
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
    ///     sigs.push(
    ///         SignatureBuilder::from(sig.clone())
    ///             .set_preferred_aead_algorithms(vec![ AEADAlgorithm::EAX ])?
    ///             .set_features(
    ///                 sig.features().unwrap_or_else(Features::sequoia)
    ///                     .set_aead())?
    ///             .sign_direct_key(&mut signer, None)?);
    /// }
    ///
    /// for ua in vc.userids() {
    ///     let sig = ua.binding_signature();
    ///     sigs.push(
    ///         SignatureBuilder::from(sig.clone())
    ///             .set_preferred_aead_algorithms(vec![ AEADAlgorithm::EAX ])?
    ///             .set_features(
    ///                 sig.features().unwrap_or_else(Features::sequoia)
    ///                     .set_aead())?
    ///             .sign_userid_binding(&mut signer, pk, ua.userid())?);
    /// }
    ///
    /// // Merge in the new signatures.
    /// let cert = cert.insert_packets(sigs)?;
    /// # assert_eq!(cert.bad_signatures().count(), 0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_preferred_aead_algorithms(mut self,
                                         preferences: Vec<AEADAlgorithm>)
        -> Result<Self>
    {
        self.hashed_area.replace(Subpacket::new(
            SubpacketValue::PreferredAEADAlgorithms(preferences),
            false)?)?;

        Ok(self)
    }

    /// Sets the Intended Recipient subpacket.
    ///
    /// Replaces any [Intended Recipient subpacket] in the hashed
    /// subpacket area with one new subpacket for each of the
    /// specified values.  That is, unlike
    /// [`SignatureBuilder::add_intended_recipient`], this function
    /// first removes any Intended Recipient subpackets from the
    /// hashed subpacket area, and then adds new ones.
    ///
    ///   [Intended Recipient subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.29
    ///   [`SignatureBuilder::add_intended_recipient`]: super::SignatureBuilder::add_intended_recipient()
    ///
    /// The Intended Recipient subpacket holds the fingerprint of a
    /// certificate.
    ///
    /// When signing a message, the message should include one such
    /// subpacket for each intended recipient.  Note: not all messages
    /// have intended recipients.  For instance, when signing an open
    /// letter, or a software release, the message is intended for
    /// anyone.
    ///
    /// When processing a signature, the application should ensure
    /// that if there are any such subpackets, then one of the
    /// subpackets identifies the recipient's certificate (or user
    /// signed the message).  If this is not the case, then an
    /// attacker may have taken the message out of its original
    /// context.  For instance, if Alice sends a signed email to Bob,
    /// with the content: "I agree to the contract", and Bob forwards
    /// that message to Carol, then Carol may think that Alice agreed
    /// to a contract with her if the signature appears to be valid!
    /// By adding an intended recipient, it is possible for Carol's
    /// mail client to warn her that although Alice signed the
    /// message, the content was intended for Bob and not for her.
    ///
    /// # Examples
    ///
    /// To create a signed message intended for both Bob and Carol,
    /// Alice adds an intended recipient subpacket for each of their
    /// certificates.  Because this function first removes any
    /// existing Intended Recipient subpackets both recipients must be
    /// added at once (cf. [`SignatureBuilder::add_intended_recipient`]):
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::SignatureBuilder;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let (alice, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// # let mut alices_signer = alice.primary_key().key().clone().parts_into_secret()?.into_keypair()?;
    /// # let (bob, _) =
    /// #     CertBuilder::general_purpose(None, Some("bob@example.org"))
    /// #     .generate()?;
    /// # let (carol, _) =
    /// #     CertBuilder::general_purpose(None, Some("carol@example.org"))
    /// #     .generate()?;
    /// #
    /// let msg = b"Let's do it!";
    ///
    /// let sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .set_intended_recipients(&[ bob.fingerprint(), carol.fingerprint() ])?
    ///     .sign_message(&mut alices_signer, msg)?;
    /// # let mut sig = sig;
    /// # assert!(sig.verify_message(alices_signer.public(), msg).is_ok());
    /// # assert_eq!(sig.intended_recipients().count(), 2);
    /// # Ok(()) }
    /// ```
    pub fn set_intended_recipients<T>(mut self, recipients: T)
        -> Result<Self>
        where T: AsRef<[Fingerprint]>
    {
        self.hashed_area.remove_all(SubpacketTag::IntendedRecipient);
        for fp in recipients.as_ref().iter() {
            self.hashed_area.add(
                Subpacket::new(SubpacketValue::IntendedRecipient(fp.clone()), false)?)?;
        }

        Ok(self)
    }

    /// Adds an Intended Recipient subpacket.
    ///
    /// Adds an [Intended Recipient subpacket] to the hashed subpacket
    /// area.  Unlike [`SignatureBuilder::set_intended_recipients`], this function does
    /// not first remove any Intended Recipient subpackets from the
    /// hashed subpacket area.
    ///
    ///   [Intended Recipient subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.2.3.29
    ///   [`SignatureBuilder::set_intended_recipients`]: super::SignatureBuilder::set_intended_recipients()
    ///
    /// The Intended Recipient subpacket holds the fingerprint of a
    /// certificate.
    ///
    /// When signing a message, the message should include one such
    /// subpacket for each intended recipient.  Note: not all messages
    /// have intended recipients.  For instance, when signing an open
    /// letter, or a software release, the message is intended for
    /// anyone.
    ///
    /// When processing a signature, the application should ensure
    /// that if there are any such subpackets, then one of the
    /// subpackets identifies the recipient's certificate (or user
    /// signed the message).  If this is not the case, then an
    /// attacker may have taken the message out of its original
    /// context.  For instance, if Alice sends a signed email to Bob,
    /// with the content: "I agree to the contract", and Bob forwards
    /// that message to Carol, then Carol may think that Alice agreed
    /// to a contract with her if the signature appears to be valid!
    /// By adding an intended recipient, it is possible for Carol's
    /// mail client to warn her that although Alice signed the
    /// message, the content was intended for Bob and not for her.
    ///
    /// # Examples
    ///
    /// To create a signed message intended for both Bob and Carol,
    /// Alice adds an Intended Recipient subpacket for each of their
    /// certificates.  Unlike
    /// [`SignatureBuilder::set_intended_recipients`], which first
    /// removes any existing Intended Recipient subpackets, with this
    /// function we can add one recipient after the other:
    ///
    /// [`SignatureBuilder::set_intended_recipients`]: NotationDataFlags::set_intended_recipients()
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::SignatureBuilder;
    /// use openpgp::types::SignatureType;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// #
    /// # let (alice, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// # let mut alices_signer = alice.primary_key().key().clone().parts_into_secret()?.into_keypair()?;
    /// # let (bob, _) =
    /// #     CertBuilder::general_purpose(None, Some("bob@example.org"))
    /// #     .generate()?;
    /// # let (carol, _) =
    /// #     CertBuilder::general_purpose(None, Some("carol@example.org"))
    /// #     .generate()?;
    /// #
    /// let msg = b"Let's do it!";
    ///
    /// let sig = SignatureBuilder::new(SignatureType::Binary)
    ///     .add_intended_recipient(bob.fingerprint())?
    ///     .add_intended_recipient(carol.fingerprint())?
    ///     .sign_message(&mut alices_signer, msg)?;
    /// # let mut sig = sig;
    /// # assert!(sig.verify_message(alices_signer.public(), msg).is_ok());
    /// # assert_eq!(sig.intended_recipients().count(), 2);
    /// # Ok(()) }
    /// ```
    pub fn add_intended_recipient(mut self, recipient: Fingerprint)
        -> Result<Self>
    {
        self.hashed_area.add(
            Subpacket::new(SubpacketValue::IntendedRecipient(recipient),
                           false)?)?;

        Ok(self)
    }

    /// Adds an attested certifications subpacket.
    ///
    /// This feature is [experimental](crate#experimental-features).
    ///
    /// Allows the certificate holder to attest to third party
    /// certifications, allowing them to be distributed with the
    /// certificate.  This can be used to address certificate flooding
    /// concerns.
    ///
    /// Sorts the digests and adds an [Attested Certification
    /// subpacket] to the hashed subpacket area.  The digests must be
    /// calculated using the same hash algorithm that is used in the
    /// resulting signature.  To attest a signature, hash it with
    /// [`super::Signature::hash_for_confirmation`].
    ///
    /// Note: The maximum size of the hashed signature subpacket area
    /// constrains the number of attestations that can be stored in a
    /// signature.  If you need to attest to more certifications,
    /// split the digests into chunks and create multiple attested key
    /// signatures with the same creation time.
    ///
    /// See [Section 5.2.3.30 of RFC 4880bis] for details.
    ///
    ///   [Section 5.2.3.30 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-10.html#section-5.2.3.30
    ///   [Attested Certification subpacket]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-10.html#section-5.2.3.30
    pub fn set_attested_certifications<A, C>(mut self, certifications: C)
                                             -> Result<Self>
    where C: IntoIterator<Item = A>,
          A: AsRef<[u8]>,
    {
        let mut digests: Vec<_> = certifications.into_iter()
            .map(|d| d.as_ref().to_vec().into_boxed_slice())
            .collect();

        if let Some(first) = digests.get(0) {
            if digests.iter().any(|d| d.len() != first.len()) {
                return Err(Error::InvalidOperation(
                    "Inconsistent digest algorithm used".into()).into());
            }
        }

        // Hashes SHOULD be sorted.  This optimizes lookups for the
        // consumer and provides a canonical form.
        digests.sort_unstable();

        self.hashed_area_mut().replace(
            Subpacket::new(
                SubpacketValue::AttestedCertifications(digests),
                true)?)?;

        Ok(self)
    }
}

#[test]
fn accessors() {
    use crate::types::Curve;

    let pk_algo = PublicKeyAlgorithm::EdDSA;
    let hash_algo = HashAlgorithm::SHA512;
    let hash = hash_algo.context().unwrap();
    let mut sig = signature::SignatureBuilder::new(crate::types::SignatureType::Binary);
    let mut key: crate::packet::key::SecretKey =
        crate::packet::key::Key4::generate_ecc(true, Curve::Ed25519).unwrap().into();
    let mut keypair = key.clone().into_keypair().unwrap();

    // Cook up a timestamp without ns resolution.
    use std::convert::TryFrom;
    let now: time::SystemTime =
        Timestamp::try_from(crate::now()).unwrap().into();

    sig = sig.set_signature_creation_time(now).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.signature_creation_time(), Some(now));

    let zero_s = time::Duration::new(0, 0);
    let minute = time::Duration::new(60, 0);
    let five_minutes = 5 * minute;
    let ten_minutes = 10 * minute;
    sig = sig.set_signature_validity_period(five_minutes).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.signature_validity_period(), Some(five_minutes));

    assert!(sig_.signature_alive(None, zero_s).is_ok());
    assert!(sig_.signature_alive(now, zero_s).is_ok());
    assert!(!sig_.signature_alive(now - five_minutes, zero_s).is_ok());
    assert!(!sig_.signature_alive(now + ten_minutes, zero_s).is_ok());

    sig = sig.modify_hashed_area(|mut a| {
        a.remove_all(SubpacketTag::SignatureExpirationTime);
        Ok(a)
    }).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.signature_validity_period(), None);

    assert!(sig_.signature_alive(None, zero_s).is_ok());
    assert!(sig_.signature_alive(now, zero_s).is_ok());
    assert!(!sig_.signature_alive(now - five_minutes, zero_s).is_ok());
    assert!(sig_.signature_alive(now + ten_minutes, zero_s).is_ok());

    sig = sig.set_exportable_certification(true).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.exportable_certification(), Some(true));
    sig = sig.set_exportable_certification(false).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.exportable_certification(), Some(false));

    sig = sig.set_trust_signature(2, 3).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.trust_signature(), Some((2, 3)));

    sig = sig.set_regular_expression(b"foobar").unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.regular_expressions().collect::<Vec<&[u8]>>(),
               vec![ &b"foobar"[..] ]);

    sig = sig.set_revocable(true).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.revocable(), Some(true));
    sig = sig.set_revocable(false).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.revocable(), Some(false));

    key.set_creation_time(now).unwrap();
    sig = sig.set_key_validity_period(Some(five_minutes)).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.key_validity_period(), Some(five_minutes));

    assert!(sig_.key_alive(&key, None).is_ok());
    assert!(sig_.key_alive(&key, now).is_ok());
    assert!(!sig_.key_alive(&key, now - five_minutes).is_ok());
    assert!(!sig_.key_alive(&key, now + ten_minutes).is_ok());

    sig = sig.set_key_validity_period(None).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.key_validity_period(), None);

    assert!(sig_.key_alive(&key, None).is_ok());
    assert!(sig_.key_alive(&key, now).is_ok());
    assert!(!sig_.key_alive(&key, now - five_minutes).is_ok());
    assert!(sig_.key_alive(&key, now + ten_minutes).is_ok());

    let pref = vec![SymmetricAlgorithm::AES256,
                    SymmetricAlgorithm::AES192,
                    SymmetricAlgorithm::AES128];
    sig = sig.set_preferred_symmetric_algorithms(pref.clone()).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.preferred_symmetric_algorithms(), Some(&pref[..]));

    let fp = Fingerprint::from_bytes(b"bbbbbbbbbbbbbbbbbbbb");
    let rk = RevocationKey::new(pk_algo, fp.clone(), true);
    sig = sig.set_revocation_key(vec![ rk.clone() ]).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.revocation_keys().next().unwrap(), &rk);

    sig = sig.set_issuer(fp.clone().into()).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.issuers().collect::<Vec<_>>(),
               vec![ &fp.clone().into() ]);

    let pref = vec![HashAlgorithm::SHA512,
                    HashAlgorithm::SHA384,
                    HashAlgorithm::SHA256];
    sig = sig.set_preferred_hash_algorithms(pref.clone()).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.preferred_hash_algorithms(), Some(&pref[..]));

    let pref = vec![CompressionAlgorithm::BZip2,
                    CompressionAlgorithm::Zlib,
                    CompressionAlgorithm::Zip];
    sig = sig.set_preferred_compression_algorithms(pref.clone()).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.preferred_compression_algorithms(), Some(&pref[..]));

    let pref = KeyServerPreferences::empty()
        .set_no_modify();
    sig = sig.set_key_server_preferences(pref.clone()).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.key_server_preferences().unwrap(), pref);

    sig = sig.set_primary_userid(true).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.primary_userid(), Some(true));
    sig = sig.set_primary_userid(false).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.primary_userid(), Some(false));

    sig = sig.set_policy_uri(b"foobar").unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.policy_uri(), Some(&b"foobar"[..]));

    let key_flags = KeyFlags::empty()
        .set_certification()
        .set_signing();
    sig = sig.set_key_flags(key_flags.clone()).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.key_flags().unwrap(), key_flags);

    sig = sig.set_signers_user_id(b"foobar").unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.signers_user_id(), Some(&b"foobar"[..]));

    sig = sig.set_reason_for_revocation(ReasonForRevocation::KeyRetired,
                                  b"foobar").unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.reason_for_revocation(),
               Some((ReasonForRevocation::KeyRetired, &b"foobar"[..])));

    let feats = Features::empty().set_mdc();
    sig = sig.set_features(feats.clone()).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.features().unwrap(), feats);

    let feats = Features::empty().set_aead();
    sig = sig.set_features(feats.clone()).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.features().unwrap(), feats);

    let digest = vec![0; hash_algo.context().unwrap().digest_size()];
    sig = sig.set_signature_target(pk_algo, hash_algo, &digest).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.signature_target(), Some((pk_algo,
                                             hash_algo,
                                             &digest[..])));

    let embedded_sig = sig_.clone();
    sig = sig.set_embedded_signature(embedded_sig.clone()).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.embedded_signatures().next(), Some(&embedded_sig));

    sig = sig.set_issuer_fingerprint(fp.clone()).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.issuer_fingerprints().collect::<Vec<_>>(),
               vec![ &fp ]);

    let pref = vec![AEADAlgorithm::EAX,
                    AEADAlgorithm::OCB];
    sig = sig.set_preferred_aead_algorithms(pref.clone()).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.preferred_aead_algorithms(), Some(&pref[..]));

    let fps = vec![
        Fingerprint::from_bytes(b"aaaaaaaaaaaaaaaaaaaa"),
        Fingerprint::from_bytes(b"bbbbbbbbbbbbbbbbbbbb"),
    ];
    sig = sig.set_intended_recipients(fps.clone()).unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.intended_recipients().collect::<Vec<&Fingerprint>>(),
               fps.iter().collect::<Vec<&Fingerprint>>());

    sig = sig.set_notation("test@example.org", &[0, 1, 2], None, false)
        .unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.notation("test@example.org").collect::<Vec<&[u8]>>(),
               vec![&[0, 1, 2]]);

    sig = sig.add_notation("test@example.org", &[3, 4, 5], None, false)
        .unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.notation("test@example.org").collect::<Vec<&[u8]>>(),
               vec![&[0, 1, 2], &[3, 4, 5]]);

    sig = sig.set_notation("test@example.org", &[6, 7, 8], None, false)
        .unwrap();
    let sig_ =
        sig.clone().sign_hash(&mut keypair, hash.clone()).unwrap();
    assert_eq!(sig_.notation("test@example.org").collect::<Vec<&[u8]>>(),
               vec![&[6, 7, 8]]);
}

#[cfg(feature = "compression-deflate")]
#[test]
fn subpacket_test_1 () {
    use crate::Packet;
    use crate::PacketPile;
    use crate::parse::Parse;

    let pile = PacketPile::from_bytes(crate::tests::message("signed.gpg")).unwrap();
    eprintln!("PacketPile has {} top-level packets.", pile.children().len());
    eprintln!("PacketPile: {:?}", pile);

    let mut count = 0;
    for p in pile.descendants() {
        if let &Packet::Signature(ref sig) = p {
            count += 1;

            let mut got2 = false;
            let mut got16 = false;
            let mut got33 = false;

            for i in 0..255 {
                if let Some(sb) = sig.subpacket(i.into()) {
                    if i == 2 {
                        got2 = true;
                        assert!(!sb.critical);
                    } else if i == 16 {
                        got16 = true;
                        assert!(!sb.critical);
                    } else if i == 33 {
                        got33 = true;
                        assert!(!sb.critical);
                    } else {
                        panic!("Unexpectedly found subpacket {}", i);
                    }
                }
            }

            assert!(got2 && got16 && got33);

            let hex = format!("{:X}", sig.issuer_fingerprints().next().unwrap());
            assert!(
                hex == "7FAF6ED7238143557BDF7ED26863C9AD5B4D22D3"
                || hex == "C03FA6411B03AE12576461187223B56678E02528");
        }
    }
    // 2 packets have subpackets.
    assert_eq!(count, 2);
}

#[test]
fn subpacket_test_2() {
    use crate::Packet;
    use crate::parse::Parse;
    use crate::PacketPile;

    //   Test #    Subpacket
    // 1 2 3 4 5 6   SignatureCreationTime
    //               * SignatureExpirationTime
    //   2           ExportableCertification
    //           6   TrustSignature
    //           6   RegularExpression
    //     3         Revocable
    // 1           7 KeyExpirationTime
    // 1             PreferredSymmetricAlgorithms
    //     3         RevocationKey
    // 1   3       7 Issuer
    // 1   3   5     NotationData
    // 1             PreferredHashAlgorithms
    // 1             PreferredCompressionAlgorithms
    // 1             KeyServerPreferences
    //               * PreferredKeyServer
    //               * PrimaryUserID
    //               * PolicyURI
    // 1             KeyFlags
    //               * SignersUserID
    //       4       ReasonForRevocation
    // 1             Features
    //               * SignatureTarget
    //             7 EmbeddedSignature
    // 1   3       7 IssuerFingerprint
    //
    // XXX: The subpackets marked with * are not tested.

    let pile = PacketPile::from_bytes(
        crate::tests::key("subpackets/shaw.gpg")).unwrap();

    // Test #1
    if let (Some(&Packet::PublicKey(ref key)),
            Some(&Packet::Signature(ref sig)))
        = (pile.children().next(), pile.children().nth(2))
    {
        //  tag: 2, SignatureCreationTime(1515791508) }
        //  tag: 9, KeyExpirationTime(63072000) }
        //  tag: 11, PreferredSymmetricAlgorithms([9, 8, 7, 2]) }
        //  tag: 16, Issuer(KeyID("F004 B9A4 5C58 6126")) }
        //  tag: 20, NotationData(NotationData { flags: 2147483648, name: [114, 97, 110, 107, 64, 110, 97, 118, 121, 46, 109, 105, 108], value: [109, 105, 100, 115, 104, 105, 112, 109, 97, 110] }) }
        //  tag: 21, PreferredHashAlgorithms([8, 9, 10, 11, 2]) }
        //  tag: 22, PreferredCompressionAlgorithms([2, 3, 1]) }
        //  tag: 23, KeyServerPreferences([128]) }
        //  tag: 27, KeyFlags([3]) }
        //  tag: 30, Features([1]) }
        //  tag: 33, IssuerFingerprint(Fingerprint("361A 96BD E1A6 5B6D 6C25  AE9F F004 B9A4 5C58 6126")) }
        // for i in 0..256 {
        //     if let Some(sb) = sig.subpacket(i as u8) {
        //         eprintln!("  {:?}", sb);
        //     }
        // }

        assert_eq!(sig.signature_creation_time(),
                   Some(Timestamp::from(1515791508).into()));
        assert_eq!(sig.subpacket(SubpacketTag::SignatureCreationTime),
                   Some(&Subpacket {
                       length: 5.into(),
                       critical: false,
                       value: SubpacketValue::SignatureCreationTime(
                           1515791508.into()),
                       authenticated: false,
                   }));

        // The signature does not expire.
        assert!(sig.signature_alive(None, None).is_ok());

        assert_eq!(sig.key_validity_period(),
                   Some(Duration::from(63072000).into()));
        assert_eq!(sig.subpacket(SubpacketTag::KeyExpirationTime),
                   Some(&Subpacket {
                       length: 5.into(),
                       critical: false,
                       value: SubpacketValue::KeyExpirationTime(
                           63072000.into()),
                       authenticated: false,
                   }));

        // Check key expiration.
        assert!(sig.key_alive(
            key,
            key.creation_time() + time::Duration::new(63072000 - 1, 0))
                .is_ok());
        assert!(! sig.key_alive(
            key,
            key.creation_time() + time::Duration::new(63072000, 0))
                .is_ok());

        assert_eq!(sig.preferred_symmetric_algorithms(),
                   Some(&[SymmetricAlgorithm::AES256,
                          SymmetricAlgorithm::AES192,
                          SymmetricAlgorithm::AES128,
                          SymmetricAlgorithm::TripleDES][..]));
        assert_eq!(sig.subpacket(SubpacketTag::PreferredSymmetricAlgorithms),
                   Some(&Subpacket {
                       length: 5.into(),
                       critical: false,
                       value: SubpacketValue::PreferredSymmetricAlgorithms(
                           vec![SymmetricAlgorithm::AES256,
                                SymmetricAlgorithm::AES192,
                                SymmetricAlgorithm::AES128,
                                SymmetricAlgorithm::TripleDES]
                       ),
                       authenticated: false,
                   }));

        assert_eq!(sig.preferred_hash_algorithms(),
                   Some(&[HashAlgorithm::SHA256,
                          HashAlgorithm::SHA384,
                          HashAlgorithm::SHA512,
                          HashAlgorithm::SHA224,
                          HashAlgorithm::SHA1][..]));
        assert_eq!(sig.subpacket(SubpacketTag::PreferredHashAlgorithms),
                   Some(&Subpacket {
                       length: 6.into(),
                       critical: false,
                       value: SubpacketValue::PreferredHashAlgorithms(
                           vec![HashAlgorithm::SHA256,
                                HashAlgorithm::SHA384,
                                HashAlgorithm::SHA512,
                                HashAlgorithm::SHA224,
                                HashAlgorithm::SHA1]
                       ),
                       authenticated: false,
                   }));

        assert_eq!(sig.preferred_compression_algorithms(),
                   Some(&[CompressionAlgorithm::Zlib,
                          CompressionAlgorithm::BZip2,
                          CompressionAlgorithm::Zip][..]));
        assert_eq!(sig.subpacket(SubpacketTag::PreferredCompressionAlgorithms),
                   Some(&Subpacket {
                       length: 4.into(),
                       critical: false,
                       value: SubpacketValue::PreferredCompressionAlgorithms(
                           vec![CompressionAlgorithm::Zlib,
                                CompressionAlgorithm::BZip2,
                                CompressionAlgorithm::Zip]
                       ),
                       authenticated: false,
                   }));

        assert_eq!(sig.key_server_preferences().unwrap(),
                   KeyServerPreferences::empty().set_no_modify());
        assert_eq!(sig.subpacket(SubpacketTag::KeyServerPreferences),
                   Some(&Subpacket {
                       length: 2.into(),
                       critical: false,
                       value: SubpacketValue::KeyServerPreferences(
                           KeyServerPreferences::empty().set_no_modify()),
                       authenticated: false,
                   }));

        assert!(sig.key_flags().unwrap().for_certification());
        assert!(sig.key_flags().unwrap().for_signing());
        assert_eq!(sig.subpacket(SubpacketTag::KeyFlags),
                   Some(&Subpacket {
                       length: 2.into(),
                       critical: false,
                       value: SubpacketValue::KeyFlags(
                           KeyFlags::empty().set_certification().set_signing()),
                       authenticated: false,
                   }));

        assert_eq!(sig.features().unwrap(), Features::empty().set_mdc());
        assert_eq!(sig.subpacket(SubpacketTag::Features),
                   Some(&Subpacket {
                       length: 2.into(),
                       critical: false,
                       value: SubpacketValue::Features(
                           Features::empty().set_mdc()),
                       authenticated: false,
                   }));

        let keyid = "F004 B9A4 5C58 6126".parse().unwrap();
        assert_eq!(sig.issuers().collect::<Vec<_>>(), vec![ &keyid ]);
        assert_eq!(sig.subpacket(SubpacketTag::Issuer),
                   Some(&Subpacket {
                       length: 9.into(),
                       critical: false,
                       value: SubpacketValue::Issuer(keyid),
                       authenticated: false,
                   }));

        let fp = "361A96BDE1A65B6D6C25AE9FF004B9A45C586126".parse().unwrap();
        assert_eq!(sig.issuer_fingerprints().collect::<Vec<_>>(), vec![ &fp ]);
        assert_eq!(sig.subpacket(SubpacketTag::IssuerFingerprint),
                   Some(&Subpacket {
                       length: 22.into(),
                       critical: false,
                       value: SubpacketValue::IssuerFingerprint(fp),
                       authenticated: false,
                   }));

        let n = NotationData {
            flags: NotationDataFlags::empty().set_human_readable(),
            name: "rank@navy.mil".into(),
            value: b"midshipman".to_vec()
        };
        assert_eq!(sig.notation_data().collect::<Vec<&NotationData>>(),
                   vec![&n]);
        assert_eq!(sig.subpacket(SubpacketTag::NotationData),
                   Some(&Subpacket {
                       length: 32.into(),
                       critical: false,
                       value: SubpacketValue::NotationData(n.clone()),
                       authenticated: false,
                   }));
        assert_eq!(sig.hashed_area().subpackets(SubpacketTag::NotationData)
                   .collect::<Vec<_>>(),
                   vec![&Subpacket {
                       length: 32.into(),
                       critical: false,
                       value: SubpacketValue::NotationData(n.clone()),
                       authenticated: false,
                   }]);
    } else {
        panic!("Expected signature!");
    }

    // Test #2
    if let Some(&Packet::Signature(ref sig)) = pile.children().nth(3) {
        // tag: 2, SignatureCreationTime(1515791490)
        // tag: 4, ExportableCertification(false)
        // tag: 16, Issuer(KeyID("CEAD 0621 0934 7957"))
        // tag: 33, IssuerFingerprint(Fingerprint("B59B 8817 F519 DCE1 0AFD  85E4 CEAD 0621 0934 7957"))

        // for i in 0..256 {
        //     if let Some(sb) = sig.subpacket(i as u8) {
        //         eprintln!("  {:?}", sb);
        //     }
        // }

        assert_eq!(sig.signature_creation_time(),
                   Some(Timestamp::from(1515791490).into()));
        assert_eq!(sig.subpacket(SubpacketTag::SignatureCreationTime),
                   Some(&Subpacket {
                       length: 5.into(),
                       critical: false,
                       value: SubpacketValue::SignatureCreationTime(
                           1515791490.into()),
                       authenticated: false,
                   }));

        assert_eq!(sig.exportable_certification(), Some(false));
        assert_eq!(sig.subpacket(SubpacketTag::ExportableCertification),
                   Some(&Subpacket {
                       length: 2.into(),
                       critical: false,
                       value: SubpacketValue::ExportableCertification(false),
                       authenticated: false,
                   }));
    }

    let pile = PacketPile::from_bytes(
        crate::tests::key("subpackets/marven.gpg")).unwrap();

    // Test #3
    if let Some(&Packet::Signature(ref sig)) = pile.children().nth(1) {
        // tag: 2, SignatureCreationTime(1515791376)
        // tag: 7, Revocable(false)
        // tag: 12, RevocationKey((128, 1, Fingerprint("361A 96BD E1A6 5B6D 6C25  AE9F F004 B9A4 5C58 6126")))
        // tag: 16, Issuer(KeyID("CEAD 0621 0934 7957"))
        // tag: 33, IssuerFingerprint(Fingerprint("B59B 8817 F519 DCE1 0AFD  85E4 CEAD 0621 0934 7957"))

        // for i in 0..256 {
        //     if let Some(sb) = sig.subpacket(i as u8) {
        //         eprintln!("  {:?}", sb);
        //     }
        // }

        assert_eq!(sig.signature_creation_time(),
                   Some(Timestamp::from(1515791376).into()));
        assert_eq!(sig.subpacket(SubpacketTag::SignatureCreationTime),
                   Some(&Subpacket {
                       length: 5.into(),
                       critical: false,
                       value: SubpacketValue::SignatureCreationTime(
                           1515791376.into()),
                       authenticated: false,
                   }));

        assert_eq!(sig.revocable(), Some(false));
        assert_eq!(sig.subpacket(SubpacketTag::Revocable),
                   Some(&Subpacket {
                       length: 2.into(),
                       critical: false,
                       value: SubpacketValue::Revocable(false),
                       authenticated: false,
                   }));

        let fp = "361A96BDE1A65B6D6C25AE9FF004B9A45C586126".parse().unwrap();
        let rk = RevocationKey::new(PublicKeyAlgorithm::RSAEncryptSign,
                                    fp, false);
        assert_eq!(sig.revocation_keys().next().unwrap(), &rk);
        assert_eq!(sig.subpacket(SubpacketTag::RevocationKey),
                   Some(&Subpacket {
                       length: 23.into(),
                       critical: false,
                       value: SubpacketValue::RevocationKey(rk),
                       authenticated: false,
                   }));


        let keyid = "CEAD 0621 0934 7957".parse().unwrap();
        assert_eq!(sig.issuers().collect::<Vec<_>>(),
                   vec![ &keyid ]);
        assert_eq!(sig.subpacket(SubpacketTag::Issuer),
                   Some(&Subpacket {
                       length: 9.into(),
                       critical: false,
                       value: SubpacketValue::Issuer(keyid),
                       authenticated: false,
                   }));

        let fp = "B59B8817F519DCE10AFD85E4CEAD062109347957".parse().unwrap();
        assert_eq!(sig.issuer_fingerprints().collect::<Vec<_>>(),
                   vec![ &fp ]);
        assert_eq!(sig.subpacket(SubpacketTag::IssuerFingerprint),
                   Some(&Subpacket {
                       length: 22.into(),
                       critical: false,
                       value: SubpacketValue::IssuerFingerprint(fp),
                       authenticated: false,
                   }));

        // This signature does not contain any notation data.
        assert_eq!(sig.notation_data().count(), 0);
        assert_eq!(sig.subpacket(SubpacketTag::NotationData),
                   None);
        assert_eq!(sig.subpackets(SubpacketTag::NotationData).count(), 0);
    } else {
        panic!("Expected signature!");
    }

    // Test #4
    if let Some(&Packet::Signature(ref sig)) = pile.children().nth(6) {
        // for i in 0..256 {
        //     if let Some(sb) = sig.subpacket(i as u8) {
        //         eprintln!("  {:?}", sb);
        //     }
        // }

        assert_eq!(sig.signature_creation_time(),
                   Some(Timestamp::from(1515886658).into()));
        assert_eq!(sig.subpacket(SubpacketTag::SignatureCreationTime),
                   Some(&Subpacket {
                       length: 5.into(),
                       critical: false,
                       value: SubpacketValue::SignatureCreationTime(
                           1515886658.into()),
                       authenticated: false,
                   }));

        assert_eq!(sig.reason_for_revocation(),
                   Some((ReasonForRevocation::Unspecified,
                         &b"Forgot to set a sig expiration."[..])));
        assert_eq!(sig.subpacket(SubpacketTag::ReasonForRevocation),
                   Some(&Subpacket {
                       length: 33.into(),
                       critical: false,
                       value: SubpacketValue::ReasonForRevocation {
                           code: ReasonForRevocation::Unspecified,
                           reason: b"Forgot to set a sig expiration.".to_vec(),
                       },
                       authenticated: false,
                   }));
    }


    // Test #5
    if let Some(&Packet::Signature(ref sig)) = pile.children().nth(7) {
        // The only thing interesting about this signature is that it
        // has multiple notations.

        assert_eq!(sig.signature_creation_time(),
                   Some(Timestamp::from(1515791467).into()));
        assert_eq!(sig.subpacket(SubpacketTag::SignatureCreationTime),
                   Some(&Subpacket {
                       length: 5.into(),
                       critical: false,
                       value: SubpacketValue::SignatureCreationTime(
                           1515791467.into()),
                       authenticated: false,
                   }));

        let n1 = NotationData {
            flags: NotationDataFlags::empty().set_human_readable(),
            name: "rank@navy.mil".into(),
            value: b"third lieutenant".to_vec()
        };
        let n2 = NotationData {
            flags: NotationDataFlags::empty().set_human_readable(),
            name: "foo@navy.mil".into(),
            value: b"bar".to_vec()
        };
        let n3 = NotationData {
            flags: NotationDataFlags::empty().set_human_readable(),
            name: "whistleblower@navy.mil".into(),
            value: b"true".to_vec()
        };

        // We expect all three notations, in order.
        assert_eq!(sig.notation_data().collect::<Vec<&NotationData>>(),
                   vec![&n1, &n2, &n3]);

        // We expect only the last notation.
        assert_eq!(sig.subpacket(SubpacketTag::NotationData),
                   Some(&Subpacket {
                       length: 35.into(),
                       critical: false,
                       value: SubpacketValue::NotationData(n3.clone()),
                       authenticated: false,
                   }));

        // We expect all three notations, in order.
        assert_eq!(sig.subpackets(SubpacketTag::NotationData)
                   .collect::<Vec<_>>(),
                   vec![
                       &Subpacket {
                           length: 38.into(),
                           critical: false,
                           value: SubpacketValue::NotationData(n1),
                           authenticated: false,
                       },
                       &Subpacket {
                           length: 24.into(),
                           critical: false,
                           value: SubpacketValue::NotationData(n2),
                           authenticated: false,
                       },
                       &Subpacket {
                           length: 35.into(),
                           critical: false,
                           value: SubpacketValue::NotationData(n3),
                           authenticated: false,
                       },
                   ]);
    }

    // # Test 6
    if let Some(&Packet::Signature(ref sig)) = pile.children().nth(8) {
        // A trusted signature.

        // tag: 2, SignatureCreationTime(1515791223)
        // tag: 5, TrustSignature((2, 120))
        // tag: 6, RegularExpression([60, 91, 94, 62, 93, 43, 91, 64, 46, 93, 110, 97, 118, 121, 92, 46, 109, 105, 108, 62, 36])
        // tag: 16, Issuer(KeyID("F004 B9A4 5C58 6126"))
        // tag: 33, IssuerFingerprint(Fingerprint("361A 96BD E1A6 5B6D 6C25  AE9F F004 B9A4 5C58 6126"))

        // for i in 0..256 {
        //     if let Some(sb) = sig.subpacket(i as u8) {
        //         eprintln!("  {:?}", sb);
        //     }
        // }

        assert_eq!(sig.signature_creation_time(),
                   Some(Timestamp::from(1515791223).into()));
        assert_eq!(sig.subpacket(SubpacketTag::SignatureCreationTime),
                   Some(&Subpacket {
                       length: 5.into(),
                       critical: false,
                       value: SubpacketValue::SignatureCreationTime(
                           1515791223.into()),
                       authenticated: false,
                   }));

        assert_eq!(sig.trust_signature(), Some((2, 120)));
        assert_eq!(sig.subpacket(SubpacketTag::TrustSignature),
                   Some(&Subpacket {
                       length: 3.into(),
                       critical: false,
                       value: SubpacketValue::TrustSignature {
                           level: 2,
                           trust: 120,
                       },
                       authenticated: false,
                   }));

        // Note: our parser strips the trailing NUL.
        let regex = &b"<[^>]+[@.]navy\\.mil>$"[..];
        assert_eq!(sig.regular_expressions().collect::<Vec<&[u8]>>(),
                   vec![ regex ]);
        assert_eq!(sig.subpacket(SubpacketTag::RegularExpression),
                   Some(&Subpacket {
                       length: 23.into(),
                       critical: true,
                       value: SubpacketValue::RegularExpression(regex.to_vec()),
                       authenticated: false,
                   }));
    }

    // Test #7
    if let Some(&Packet::Signature(ref sig)) = pile.children().nth(11) {
        // A subkey self-sig, which contains an embedded signature.
        //  tag: 2, SignatureCreationTime(1515798986)
        //  tag: 9, KeyExpirationTime(63072000)
        //  tag: 16, Issuer(KeyID("CEAD 0621 0934 7957"))
        //  tag: 27, KeyFlags([2])
        //  tag: 32, EmbeddedSignature(Signature(Signature {
        //    version: 4, sigtype: 25, timestamp: Some(1515798986),
        //    issuer: "F682 42EA 9847 7034 5DEC  5F08 4688 10D3 D67F 6CA9",
        //    pk_algo: 1, hash_algo: 8, hashed_area: "29 bytes",
        //    unhashed_area: "10 bytes", hash_prefix: [162, 209],
        //    mpis: "258 bytes"))
        //  tag: 33, IssuerFingerprint(Fingerprint("B59B 8817 F519 DCE1 0AFD  85E4 CEAD 0621 0934 7957"))

        // for i in 0..256 {
        //     if let Some(sb) = sig.subpacket(i as u8) {
        //         eprintln!("  {:?}", sb);
        //     }
        // }

        assert_eq!(sig.key_validity_period(),
                   Some(Duration::from(63072000).into()));
        assert_eq!(sig.subpacket(SubpacketTag::KeyExpirationTime),
                   Some(&Subpacket {
                       length: 5.into(),
                       critical: false,
                       value: SubpacketValue::KeyExpirationTime(
                           63072000.into()),
                       authenticated: false,
                   }));

        let keyid = "CEAD 0621 0934 7957".parse().unwrap();
        assert_eq!(sig.issuers().collect::<Vec<_>>(), vec! [&keyid ]);
        assert_eq!(sig.subpacket(SubpacketTag::Issuer),
                   Some(&Subpacket {
                       length: 9.into(),
                       critical: false,
                       value: SubpacketValue::Issuer(keyid),
                       authenticated: false,
                   }));

        let fp = "B59B8817F519DCE10AFD85E4CEAD062109347957".parse().unwrap();
        assert_eq!(sig.issuer_fingerprints().collect::<Vec<_>>(),
                   vec![ &fp ]);
        assert_eq!(sig.subpacket(SubpacketTag::IssuerFingerprint),
                   Some(&Subpacket {
                       length: 22.into(),
                       critical: false,
                       value: SubpacketValue::IssuerFingerprint(fp),
                       authenticated: false,
                   }));

        assert_eq!(sig.embedded_signatures().count(), 1);
        assert!(sig.subpacket(SubpacketTag::EmbeddedSignature)
                .is_some());
    }

//     for (i, p) in pile.children().enumerate() {
//         if let &Packet::Signature(ref sig) = p {
//             eprintln!("{:?}: {:?}", i, sig);
//             for j in 0..256 {
//                 if let Some(sb) = sig.subpacket(j as u8) {
//                     eprintln!("  {:?}", sb);
//                 }
//             }
//         }
    //     }
    ()
}

#[test]
fn issuer_default() -> Result<()> {
    use crate::types::Curve;

    let hash_algo = HashAlgorithm::SHA512;
    let hash = hash_algo.context()?;
    let sig = signature::SignatureBuilder::new(crate::types::SignatureType::Binary);
    let key: crate::packet::key::SecretKey =
        crate::packet::key::Key4::generate_ecc(true, Curve::Ed25519)?.into();
    let mut keypair = key.into_keypair()?;

    // no issuer or issuer_fingerprint present, use default
    let sig_ = sig.sign_hash(&mut keypair, hash.clone())?;

    assert_eq!(sig_.issuers().collect::<Vec<_>>(),
               vec![ &keypair.public().keyid() ]);
    assert_eq!(sig_.issuer_fingerprints().collect::<Vec<_>>(),
               vec![ &keypair.public().fingerprint() ]);

    let fp = Fingerprint::from_bytes(b"bbbbbbbbbbbbbbbbbbbb");

    // issuer subpacket present, do not override
    let mut sig = signature::SignatureBuilder::new(crate::types::SignatureType::Binary);

    sig = sig.set_issuer(fp.clone().into())?;
    let sig_ = sig.clone().sign_hash(&mut keypair, hash.clone())?;

    assert_eq!(sig_.issuers().collect::<Vec<_>>(),
               vec![ &fp.clone().into() ]);
    assert_eq!(sig_.issuer_fingerprints().count(), 0);

    // issuer_fingerprint subpacket present, do not override
    let mut sig = signature::SignatureBuilder::new(crate::types::SignatureType::Binary);

    sig = sig.set_issuer_fingerprint(fp.clone())?;
    let sig_ = sig.clone().sign_hash(&mut keypair, hash.clone())?;

    assert_eq!(sig_.issuer_fingerprints().collect::<Vec<_>>(),
               vec![ &fp ]);
    assert_eq!(sig_.issuers().count(), 0);
    Ok(())
}
