//! Packet-related data types.
//!
//! OpenPGP data structures are [packet based].  This module defines
//! the corresponding data structures.
//!
//! Most users of this library will not need to generate these packets
//! themselves.  Instead, the packets are instantiated as a side
//! effect of [parsing a message], or [creating a message].  The main
//! current exception are `Signature` packets.  Working with
//! `Signature` packets is, however, simplified by using the
//! [`SignatureBuilder`].
//!
//! # Data Types
//!
//! Many OpenPGP packets include a version field.  Versioning is used
//! to make it easier to change the standard.  For instance, using
//! versioning, it is possible to remove a field from a packet without
//! introducing a new packet type, which would also require changing
//! [the grammar].  Versioning also enables a degree of forward
//! compatibility when a new version of a packet can be safely
//! ignored.  For instance, there are currently two versions of the
//! [`Signature`] packet with completely different layouts: [v3] and
//! [v4].  An implementation that does not understand the latest
//! version of the packet can still parse and display a message using
//! them; it will just be unable to verify that signature.
//!
//! In Sequoia, packets that have a version field are represented by
//! `enum`s, and each supported version of the packet has a variant,
//! and a corresponding `struct`.  This is the case even when only one
//! version of the packet is currently defined, as is the case with
//! the [`OnePassSig`] packet.  The `enum`s implement forwarders for
//! common operations.  As such, users of this library can often
//! ignore that there are multiple versions of a given packet.
//!
//! # Unknown Packets
//!
//! Sequoia gracefully handles unsupported packets by storing them as
//! [`Unknown`] packets.  There are several types of unknown packets:
//!
//!   - Packets that are known, but explicitly not supported.
//!
//!     The two major examples are the [`SED`] packet type and v3
//!     `Signature` packets, which have both been considered insecure
//!     for well over a decade.
//!
//!     Note: future versions of Sequoia may add limited support for
//!     these packets to enable parsing archived messages.
//!
//!   - Packets that are known about, but that use unsupported
//!     options, e.g., a [`Compressed Data`] packet using an unknown or
//!     unsupported algorithm.
//!
//!   - Packets that are unknown, e.g., future or [private
//!     extensions].
//!
//! When Sequoia [parses] a message containing these packets, it
//! doesn't fail.  Instead, Sequoia stores them in the [`Unknown`]
//! data structure.  This allows applications to not only continue to
//! process such messages (albeit with degraded performance), but to
//! losslessly reserialize the messages, should that be required.
//!
//! # Containers
//!
//! Packets can be divided into two categories: containers and
//! non-containers.  A container is a packet that contains other
//! OpenPGP packets.  For instance, by definition, a [`Compressed
//! Data`] packet contains an [OpenPGP Message].  It is possible to
//! iterate over a container's descendants using the
//! [`Container::descendants`] method.  (Note: `Container`s [`Deref`]
//! to [`Container`].)
//!
//! # Packet Headers and Bodies
//!
//! Conceptually, packets have zero or more headers and an optional
//! body.  The headers are small, and have a known upper bound.  The
//! version field is, for instance, 4 bytes, and although
//! [`Signature`][] [`SubpacketArea`][] areas are variable in size,
//! they are limited to 64 KB.  In contrast the body, can be unbounded
//! in size.
//!
//! To limit memory use, and enable streaming processing (i.e.,
//! ensuring that processing a message can be done using a fixed size
//! buffer), Sequoia does not require that a packet's body be present
//! in memory.  For instance, the body of a literal data packet may be
//! streamed.  And, at the end, a [`Literal`] packet is still
//! returned.  This allows the caller to examine the message
//! structure, and the message headers in *in toto* even when
//! streaming.  It is even possible to compare two streamed version of
//! a packet: Sequoia stores a hash of the body.  See the [`Body`]
//! data structure for more details.
//!
//! # Equality
//!
//! There are several reasonable ways to define equality for
//! `Packet`s.  Unfortunately, none of them are appropriate in all
//! situations.  This makes choosing a general-purpose equality
//! function for [`Eq`] difficult.
//!
//! Consider defining `Eq` as the equivalence of two `Packet`s'
//! serialized forms.  If an application naively deduplicates
//! signatures, then an attacker can potentially perform a denial of
//! service attack by causing the application to process many
//! cryptographically-valid `Signature`s by varying the content of one
//! cryptographically-valid `Signature`'s unhashed area.  This attack
//! can be prevented by only comparing data that is protected by the
//! signature.  But this means that naively deduplicating `Signature`
//! packets will return in "a random" variant being used.  So, again,
//! an attacker could create variants of a cryptographically-valid
//! `Signature` to get the implementation to incorrectly drop a useful
//! one.
//!
//! These issues are also relevant when comparing [`Key`s]: should the
//! secret key material be compared?  Usually we want to merge the
//! secret key material.  But, again, if done naively, the incorrect
//! secret key material may be retained or dropped completely.
//!
//! Instead of trying to come up with a definition of equality that is
//! reasonable for all situations, we use a conservative definition:
//! two packets are considered equal if the serialized forms of their
//! packet bodies as defined by RFC 4880 are equal.  That is, two
//! packets are considered equal if and only if their serialized forms
//! are equal modulo the OpenPGP framing ([`CTB`] and [length style],
//! potential [partial body encoding]).  This definition will avoid
//! unintentionally dropping information when naively deduplicating
//! packets, but it will result in potential redundancies.
//!
//! For some packets, we provide additional variants of equality.  For
//! instance, [`Key::public_cmp`] compares just the public parts of
//! two keys.
//!
//! [packet based]: https://tools.ietf.org/html/rfc4880#section-5
//! [the grammar]: https://tools.ietf.org/html/rfc4880#section-11
//! [v3]: https://tools.ietf.org/html/rfc4880#section-5.2.2
//! [v4]: https://tools.ietf.org/html/rfc4880#section-5.2.3
//! [parsing a message]: crate::parse
//! [creating a message]: crate::serialize::stream
//! [`SignatureBuilder`]: signature::SignatureBuilder
//! [`SED`]: https://tools.ietf.org/html/rfc4880#section-5.7
//! [private extensions]: https://tools.ietf.org/html/rfc4880#section-4.3
//! [`Compressed Data`]: CompressedData
//! [parses]: crate::parse
//! [OpenPGP Message]: https://tools.ietf.org/html/rfc4880#section-11.3
//! [`Container::descendants`]: Container::descendants()
//! [`Deref`]: std::ops::Deref
//! [`SubpacketArea`]: signature::subpacket::SubpacketArea
//! [`Eq`]: std::cmp::Eq
//! [`Key`s]: Key
//! [`CTB`]: header::CTB
//! [length style]: https://tools.ietf.org/html/rfc4880#section-4.2
//! [partial body encoding]: https://tools.ietf.org/html/rfc4880#section-4.2.2.4
//! [`Key::public_cmp`]: Key::public_cmp()
use std::fmt;
use std::hash::Hasher;
use std::ops::{Deref, DerefMut};
use std::slice;
use std::iter::IntoIterator;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::Error;
use crate::Result;

#[macro_use]
mod container;
pub use container::Container;
pub use container::Body;

pub mod prelude;

use crate::crypto::{
    KeyPair,
    Password,
};

mod any;
pub use self::any::Any;

mod tag;
pub use self::tag::Tag;
pub mod header;
pub use self::header::Header;

mod unknown;
pub use self::unknown::Unknown;
pub mod signature;
pub mod one_pass_sig;
pub mod key;
use key::{
    Key4,
    SecretKeyMaterial
};
mod marker;
pub use self::marker::Marker;
mod trust;
pub use self::trust::Trust;
mod userid;
pub use self::userid::UserID;
pub mod user_attribute;
pub use self::user_attribute::UserAttribute;
mod literal;
pub use self::literal::Literal;
mod compressed_data;
pub use self::compressed_data::CompressedData;
pub mod seip;
pub mod skesk;
pub mod pkesk;
mod mdc;
pub use self::mdc::MDC;
pub mod aed;

/// Enumeration of packet types.
///
/// The different OpenPGP packets are detailed in [Section 5 of RFC 4880].
///
///   [Section 5 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5
///
/// The [`Unknown`] packet allows Sequoia to deal with packets that it
/// doesn't understand.  It is basically a binary blob that includes
/// the packet's [tag].  See the [module-level documentation] for
/// details.
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// # A note on equality
///
/// We define equality on `Packet` as the equality of the serialized
/// form of their packet bodies as defined by RFC 4880.  That is, two
/// packets are considered equal if and only if their serialized forms
/// are equal, modulo the OpenPGP framing ([`CTB`] and [length style],
/// potential [partial body encoding]).
///
/// [`Unknown`]: crate::packet::Unknown
/// [tag]: https://tools.ietf.org/html/rfc4880#section-4.3
/// [module-level documentation]: crate::packet#unknown-packets
/// [`CTB`]: crate::packet::header::CTB
/// [length style]: https://tools.ietf.org/html/rfc4880#section-4.2
/// [partial body encoding]: https://tools.ietf.org/html/rfc4880#section-4.2.2.4
#[non_exhaustive]
#[derive(PartialEq, Eq, Hash, Clone)]
pub enum Packet {
    /// Unknown packet.
    Unknown(Unknown),
    /// Signature packet.
    Signature(Signature),
    /// One pass signature packet.
    OnePassSig(OnePassSig),
    /// Public key packet.
    PublicKey(key::PublicKey),
    /// Public subkey packet.
    PublicSubkey(key::PublicSubkey),
    /// Public/Secret key pair.
    SecretKey(key::SecretKey),
    /// Public/Secret subkey pair.
    SecretSubkey(key::SecretSubkey),
    /// Marker packet.
    Marker(Marker),
    /// Trust packet.
    Trust(Trust),
    /// User ID packet.
    UserID(UserID),
    /// User attribute packet.
    UserAttribute(UserAttribute),
    /// Literal data packet.
    Literal(Literal),
    /// Compressed literal data packet.
    CompressedData(CompressedData),
    /// Public key encrypted data packet.
    PKESK(PKESK),
    /// Symmetric key encrypted data packet.
    SKESK(SKESK),
    /// Symmetric key encrypted, integrity protected data packet.
    SEIP(SEIP),
    /// Modification detection code packet.
    MDC(MDC),
    /// AEAD Encrypted Data Packet.
    AED(AED),
}
assert_send_and_sync!(Packet);

macro_rules! impl_into_iterator {
    ($t:ty) => {
        impl_into_iterator!($t where);
    };
    ($t:ty where $( $w:ident: $c:path ),*) => {
        /// Implement `IntoIterator` so that
        /// `cert::insert_packets(sig)` just works.
        impl<$($w),*> IntoIterator for $t
            where $($w: $c ),*
        {
            type Item = $t;
            type IntoIter = std::iter::Once<$t>;

            fn into_iter(self) -> Self::IntoIter {
                std::iter::once(self)
            }
        }
    }
}

impl_into_iterator!(Packet);
impl_into_iterator!(Unknown);
impl_into_iterator!(Signature);
impl_into_iterator!(OnePassSig);
impl_into_iterator!(Marker);
impl_into_iterator!(Trust);
impl_into_iterator!(UserID);
impl_into_iterator!(UserAttribute);
impl_into_iterator!(Literal);
impl_into_iterator!(CompressedData);
impl_into_iterator!(PKESK);
impl_into_iterator!(SKESK);
impl_into_iterator!(SEIP);
impl_into_iterator!(MDC);
impl_into_iterator!(AED);
impl_into_iterator!(Key<P, R> where P: key::KeyParts, R: key::KeyRole);

// Make it easy to pass an iterator of Packets to something expecting
// an iterator of Into<Result<Packet>> (specifically,
// CertParser::into_iter).
impl From<Packet> for Result<Packet> {
    fn from(p: Packet) -> Self {
        Ok(p)
    }
}

impl Packet {
    /// Returns the `Packet's` corresponding OpenPGP tag.
    ///
    /// Tags are explained in [Section 4.3 of RFC 4880].
    ///
    ///   [Section 4.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.3
    pub fn tag(&self) -> Tag {
        match self {
            Packet::Unknown(ref packet) => packet.tag(),
            Packet::Signature(_) => Tag::Signature,
            Packet::OnePassSig(_) => Tag::OnePassSig,
            Packet::PublicKey(_) => Tag::PublicKey,
            Packet::PublicSubkey(_) => Tag::PublicSubkey,
            Packet::SecretKey(_) => Tag::SecretKey,
            Packet::SecretSubkey(_) => Tag::SecretSubkey,
            Packet::Marker(_) => Tag::Marker,
            Packet::Trust(_) => Tag::Trust,
            Packet::UserID(_) => Tag::UserID,
            Packet::UserAttribute(_) => Tag::UserAttribute,
            Packet::Literal(_) => Tag::Literal,
            Packet::CompressedData(_) => Tag::CompressedData,
            Packet::PKESK(_) => Tag::PKESK,
            Packet::SKESK(_) => Tag::SKESK,
            Packet::SEIP(_) => Tag::SEIP,
            Packet::MDC(_) => Tag::MDC,
            Packet::AED(_) => Tag::AED,
        }
    }

    /// Returns the parsed `Packet's` corresponding OpenPGP tag.
    ///
    /// Returns the packets tag, but only if it was successfully
    /// parsed into the corresponding packet type.  If e.g. a
    /// Signature Packet uses some unsupported methods, it is parsed
    /// into an `Packet::Unknown`.  `tag()` returns `Tag::Signature`,
    /// whereas `kind()` returns `None`.
    pub fn kind(&self) -> Option<Tag> {
        match self {
            Packet::Unknown(_) => None,
            _ => Some(self.tag()),
        }
    }

    /// Returns the `Packet's` version, if the packet is versioned and
    /// recognized.
    ///
    /// If the packet is not versioned, or we couldn't parse the
    /// packet, this function returns `None`.
    pub fn version(&self) -> Option<u8> {
        match self {
            Packet::Unknown(_) => None,
            Packet::Signature(p) => Some(p.version()),
            Packet::OnePassSig(p) => Some(p.version()),
            Packet::PublicKey(p) => Some(p.version()),
            Packet::PublicSubkey(p) => Some(p.version()),
            Packet::SecretKey(p) => Some(p.version()),
            Packet::SecretSubkey(p) => Some(p.version()),
            Packet::Marker(_) => None,
            Packet::Trust(_) => None,
            Packet::UserID(_) => None,
            Packet::UserAttribute(_) => None,
            Packet::Literal(_) => None,
            Packet::CompressedData(_) => None,
            Packet::PKESK(p) => Some(p.version()),
            Packet::SKESK(p) => Some(p.version()),
            Packet::SEIP(p) => Some(p.version()),
            Packet::MDC(_) => None,
            Packet::AED(p) => Some(p.version()),
        }
    }

    /// Hashes most everything into state.
    ///
    /// This is an alternate implementation of [`Hash`], which does
    /// not hash:
    ///
    ///   - The unhashed subpacket area of Signature packets.
    ///   - Secret key material.
    ///
    ///   [`Hash`]: std::hash::Hash
    ///
    /// Unlike [`Signature::normalize`], this method ignores
    /// authenticated packets in the unhashed subpacket area.
    ///
    ///   [`Signature::normalize`]: Signature::normalize()
    pub fn normalized_hash<H>(&self, state: &mut H)
        where H: Hasher
    {
        use std::hash::Hash;

        match self {
            Packet::Signature(sig) => sig.normalized_hash(state),
            Packet::OnePassSig(x) => Hash::hash(&x, state),
            Packet::PublicKey(k) => k.public_hash(state),
            Packet::PublicSubkey(k) => k.public_hash(state),
            Packet::SecretKey(k) => k.public_hash(state),
            Packet::SecretSubkey(k) => k.public_hash(state),
            Packet::Marker(x) => Hash::hash(&x, state),
            Packet::Trust(x) => Hash::hash(&x, state),
            Packet::UserID(x) => Hash::hash(&x, state),
            Packet::UserAttribute(x) => Hash::hash(&x, state),
            Packet::Literal(x) => Hash::hash(&x, state),
            Packet::CompressedData(x) => Hash::hash(&x, state),
            Packet::PKESK(x) => Hash::hash(&x, state),
            Packet::SKESK(x) => Hash::hash(&x, state),
            Packet::SEIP(x) => Hash::hash(&x, state),
            Packet::MDC(x) => Hash::hash(&x, state),
            Packet::AED(x) => Hash::hash(&x, state),
            Packet::Unknown(x) => Hash::hash(&x, state),
        }
    }
}

// Allow transparent access of common fields.
impl Deref for Packet {
    type Target = Common;

    fn deref(&self) -> &Self::Target {
        match self {
            Packet::Unknown(ref packet) => &packet.common,
            Packet::Signature(ref packet) => &packet.common,
            Packet::OnePassSig(ref packet) => &packet.common,
            Packet::PublicKey(ref packet) => &packet.common,
            Packet::PublicSubkey(ref packet) => &packet.common,
            Packet::SecretKey(ref packet) => &packet.common,
            Packet::SecretSubkey(ref packet) => &packet.common,
            Packet::Marker(ref packet) => &packet.common,
            Packet::Trust(ref packet) => &packet.common,
            Packet::UserID(ref packet) => &packet.common,
            Packet::UserAttribute(ref packet) => &packet.common,
            Packet::Literal(ref packet) => &packet.common,
            Packet::CompressedData(ref packet) => &packet.common,
            Packet::PKESK(ref packet) => &packet.common,
            Packet::SKESK(SKESK::V4(ref packet)) => &packet.common,
            Packet::SKESK(SKESK::V5(ref packet)) => &packet.skesk4.common,
            Packet::SEIP(ref packet) => &packet.common,
            Packet::MDC(ref packet) => &packet.common,
            Packet::AED(ref packet) => &packet.common,
        }
    }
}

impl DerefMut for Packet {
    fn deref_mut(&mut self) -> &mut Common {
        match self {
            Packet::Unknown(ref mut packet) => &mut packet.common,
            Packet::Signature(ref mut packet) => &mut packet.common,
            Packet::OnePassSig(ref mut packet) => &mut packet.common,
            Packet::PublicKey(ref mut packet) => &mut packet.common,
            Packet::PublicSubkey(ref mut packet) => &mut packet.common,
            Packet::SecretKey(ref mut packet) => &mut packet.common,
            Packet::SecretSubkey(ref mut packet) => &mut packet.common,
            Packet::Marker(ref mut packet) => &mut packet.common,
            Packet::Trust(ref mut packet) => &mut packet.common,
            Packet::UserID(ref mut packet) => &mut packet.common,
            Packet::UserAttribute(ref mut packet) => &mut packet.common,
            Packet::Literal(ref mut packet) => &mut packet.common,
            Packet::CompressedData(ref mut packet) => &mut packet.common,
            Packet::PKESK(ref mut packet) => &mut packet.common,
            Packet::SKESK(SKESK::V4(ref mut packet)) => &mut packet.common,
            Packet::SKESK(SKESK::V5(ref mut packet)) => &mut packet.skesk4.common,
            Packet::SEIP(ref mut packet) => &mut packet.common,
            Packet::MDC(ref mut packet) => &mut packet.common,
            Packet::AED(ref mut packet) => &mut packet.common,
        }
    }
}

impl fmt::Debug for Packet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fn debug_fmt(p: &Packet, f: &mut fmt::Formatter) -> fmt::Result {
            use Packet::*;
            match p {
                Unknown(v) => write!(f, "Unknown({:?})", v),
                Signature(v) => write!(f, "Signature({:?})", v),
                OnePassSig(v) => write!(f, "OnePassSig({:?})", v),
                PublicKey(v) => write!(f, "PublicKey({:?})", v),
                PublicSubkey(v) => write!(f, "PublicSubkey({:?})", v),
                SecretKey(v) => write!(f, "SecretKey({:?})", v),
                SecretSubkey(v) => write!(f, "SecretSubkey({:?})", v),
                Marker(v) => write!(f, "Marker({:?})", v),
                Trust(v) => write!(f, "Trust({:?})", v),
                UserID(v) => write!(f, "UserID({:?})", v),
                UserAttribute(v) => write!(f, "UserAttribute({:?})", v),
                Literal(v) => write!(f, "Literal({:?})", v),
                CompressedData(v) => write!(f, "CompressedData({:?})", v),
                PKESK(v) => write!(f, "PKESK({:?})", v),
                SKESK(v) => write!(f, "SKESK({:?})", v),
                SEIP(v) => write!(f, "SEIP({:?})", v),
                MDC(v) => write!(f, "MDC({:?})", v),
                AED(v) => write!(f, "AED({:?})", v),
            }
        }

        fn try_armor_fmt(p: &Packet, f: &mut fmt::Formatter)
                         -> Result<fmt::Result> {
            use crate::armor::{Writer, Kind};
            use crate::serialize::Serialize;
            let mut w = Writer::new(Vec::new(), Kind::File)?;
            p.serialize(&mut w)?;
            let buf = w.finalize()?;
            Ok(f.write_str(std::str::from_utf8(&buf).expect("clean")))
        }

        if ! cfg!(test) {
            debug_fmt(self, f)
        } else {
            try_armor_fmt(self, f).unwrap_or_else(|_| debug_fmt(self, f))
        }
    }
}

#[cfg(test)]
impl Arbitrary for Packet {
    fn arbitrary(g: &mut Gen) -> Self {
        use crate::arbitrary_helper::gen_arbitrary_from_range;

        match gen_arbitrary_from_range(0..15, g) {
            0 => Signature::arbitrary(g).into(),
            1 => OnePassSig::arbitrary(g).into(),
            2 => Key::<key::PublicParts, key::UnspecifiedRole>::arbitrary(g)
                .role_into_primary().into(),
            3 => Key::<key::PublicParts, key::UnspecifiedRole>::arbitrary(g)
                .role_into_subordinate().into(),
            4 => Key::<key::SecretParts, key::UnspecifiedRole>::arbitrary(g)
                .role_into_primary().into(),
            5 => Key::<key::SecretParts, key::UnspecifiedRole>::arbitrary(g)
                .role_into_subordinate().into(),
            6 => Marker::arbitrary(g).into(),
            7 => Trust::arbitrary(g).into(),
            8 => UserID::arbitrary(g).into(),
            9 => UserAttribute::arbitrary(g).into(),
            10 => Literal::arbitrary(g).into(),
            11 => CompressedData::arbitrary(g).into(),
            12 => PKESK::arbitrary(g).into(),
            13 => SKESK::arbitrary(g).into(),
            14 => loop {
                let mut u = Unknown::new(
                    Tag::arbitrary(g), anyhow::anyhow!("Arbitrary::arbitrary"));
                u.set_body(Arbitrary::arbitrary(g));
                let u = Packet::Unknown(u);

                // Check that we didn't accidentally make a valid
                // packet.
                use crate::parse::Parse;
                use crate::serialize::SerializeInto;
                if let Ok(Packet::Unknown(_)) = Packet::from_bytes(
                    &u.to_vec().unwrap())
                {
                    break u;
                }

                // Try again!
            },
            _ => unreachable!(),
        }
    }
}

/// Fields used by multiple packet types.
#[derive(Default, Debug, Clone)]
pub struct Common {
    // In the future, this structure will hold the parsed CTB, packet
    // length, and lengths of chunks of partial body encoded packets.
    // This will allow for bit-perfect roundtripping of parsed
    // packets.  Since we consider Packets to be equal if their
    // serialized form is equal modulo CTB, packet length encoding,
    // and chunk lengths, this structure has trivial implementations
    // for PartialEq, Eq, PartialOrd, Ord, and Hash, so that we can
    // derive PartialEq, Eq, PartialOrd, Ord, and Hash for most
    // packets.

    /// XXX: Prevents trivial matching on this structure.  Remove once
    /// this structure actually gains some fields.
    dummy: std::marker::PhantomData<()>,
}
assert_send_and_sync!(Common);

#[cfg(test)]
impl Arbitrary for Common {
    fn arbitrary(_: &mut Gen) -> Self {
        // XXX: Change if this gets interesting fields.
        Common::default()
    }
}

impl PartialEq for Common {
    fn eq(&self, _: &Common) -> bool {
        // Don't compare anything.
        true
    }
}

impl Eq for Common {}

impl PartialOrd for Common {
    fn partial_cmp(&self, _: &Self) -> Option<std::cmp::Ordering> {
        Some(std::cmp::Ordering::Equal)
    }
}

impl Ord for Common {
    fn cmp(&self, _: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Equal
    }
}

impl std::hash::Hash for Common {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {
        // Don't hash anything.
    }
}


/// An iterator over the *contents* of a packet in depth-first order.
///
/// Given a [`Packet`], an `Iter` iterates over the `Packet` and any
/// `Packet`s that it contains.  For non-container `Packet`s, this
/// just returns a reference to the `Packet` itself.  For [container
/// `Packet`s] like [`CompressedData`], [`SEIP`], and [`AED`], this
/// walks the `Packet` hierarchy in depth-first order, and returns the
/// `Packet`s the first time they are visited.  (Thus, the packet
/// itself is always returned first.)
///
/// This is returned by [`PacketPile::descendants`] and
/// [`Container::descendants`].
///
/// [container `Packet`s]: self#containers
/// [`PacketPile::descendants`]: super::PacketPile::descendants()
/// [`Container::descendants`]: Container::descendants()
pub struct Iter<'a> {
    // An iterator over the current message's children.
    children: slice::Iter<'a, Packet>,
    // The current child (i.e., the last value returned by
    // children.next()).
    child: Option<&'a Packet>,
    // The an iterator over the current child's children.
    grandchildren: Option<Box<Iter<'a>>>,

    // The depth of the last returned packet.  This is used by the
    // `paths` iter.
    depth: usize,
}
assert_send_and_sync!(Iter<'_>);

impl<'a> Default for Iter<'a> {
    fn default() -> Self {
        Iter {
            children: [].iter(),
            child: None,
            grandchildren: None,
            depth: 0,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Packet;

    fn next(&mut self) -> Option<Self::Item> {
        // If we don't have a grandchild iterator (self.grandchildren
        // is None), then we are just starting, and we need to get the
        // next child.
        if let Some(ref mut grandchildren) = self.grandchildren {
            let grandchild = grandchildren.next();
            // If the grandchild iterator is exhausted (grandchild is
            // None), then we need the next child.
            if grandchild.is_some() {
                self.depth = grandchildren.depth + 1;
                return grandchild;
            }
        }

        // Get the next child and the iterator for its children.
        self.child = self.children.next();
        if let Some(child) = self.child {
            self.grandchildren = child.descendants().map(Box::new);
        }

        // First return the child itself.  Subsequent calls will
        // return its grandchildren.
        self.depth = 0;
        self.child
    }
}

impl<'a> Iter<'a> {
    /// Extends an `Iter` to also return each packet's `pathspec`.
    ///
    /// This is similar to `enumerate`, but instead of counting, this
    /// returns each packet's `pathspec` in addition to a reference to
    /// the packet.
    ///
    /// See [`PacketPile::path_ref`] for an explanation of
    /// `pathspec`s.
    ///
    /// [`PacketPile::path_ref`]: super::PacketPile::path_ref
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::PacketPile;
    ///
    /// # fn main() -> Result<()> {
    /// # let message = {
    /// #     use openpgp::types::CompressionAlgorithm;
    /// #     use openpgp::packet;
    /// #     use openpgp::PacketPile;
    /// #     use openpgp::serialize::Serialize;
    /// #     use openpgp::parse::Parse;
    /// #     use openpgp::types::DataFormat;
    /// #
    /// #     let mut lit = Literal::new(DataFormat::Text);
    /// #     lit.set_body(b"test".to_vec());
    /// #     let lit = Packet::from(lit);
    /// #
    /// #     let mut cd = CompressedData::new(
    /// #         CompressionAlgorithm::Uncompressed);
    /// #     cd.set_body(packet::Body::Structured(vec![lit.clone()]));
    /// #     let cd = Packet::from(cd);
    /// #
    /// #     // Make sure we created the message correctly: serialize,
    /// #     // parse it, and then check its form.
    /// #     let mut bytes = Vec::new();
    /// #     cd.serialize(&mut bytes)?;
    /// #
    /// #     let pp = PacketPile::from_bytes(&bytes[..])?;
    /// #
    /// #     assert_eq!(pp.descendants().count(), 2);
    /// #     assert_eq!(pp.path_ref(&[0]).unwrap().tag(),
    /// #                packet::Tag::CompressedData);
    /// #     assert_eq!(pp.path_ref(&[0, 0]), Some(&lit));
    /// #
    /// #     cd
    /// # };
    /// #
    /// let pp = PacketPile::from(message);
    /// let tags: Vec<(Vec<usize>, Tag)> = pp.descendants().paths()
    ///     .map(|(path, packet)| (path, packet.into()))
    ///     .collect::<Vec<_>>();
    /// assert_eq!(&tags,
    ///            &[
    ///               // Root.
    ///               ([0].to_vec(), Tag::CompressedData),
    ///               // Root's first child.
    ///               ([0, 0].to_vec(), Tag::Literal),
    ///             ]);
    /// # Ok(()) }
    /// ```
    pub fn paths(self)
                 -> impl Iterator<Item = (Vec<usize>, &'a Packet)> + Send + Sync
    {
        PacketPathIter {
            iter: self,
            path: None,
        }
    }
}


/// Augments the packet returned by `Iter` with its `pathspec`.
///
/// Like [`Iter::enumerate`].
///
/// [`Iter::enumerate`]: std::iter::Iterator::enumerate()
struct PacketPathIter<'a> {
    iter: Iter<'a>,

    // The path to the most recently returned node relative to the
    // start of the iterator.
    path: Option<Vec<usize>>,
}

impl<'a> Iterator for PacketPathIter<'a> {
    type Item = (Vec<usize>, &'a Packet);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(packet) = self.iter.next() {
            if self.path.is_none() {
                // Init.
                let mut path = Vec::with_capacity(4);
                path.push(0);
                self.path = Some(path);
            } else {
                let mut path = self.path.take().unwrap();
                let old_depth = path.len() - 1;

                let depth = self.iter.depth;
                if old_depth > depth {
                    // We popped.
                    path.truncate(depth + 1);
                    path[depth] += 1;
                } else if old_depth == depth {
                    // Sibling.
                    path[old_depth] += 1;
                } else if old_depth + 1 == depth {
                    // Recursion.
                    path.push(0);
                }
                self.path = Some(path);
            }
            Some((self.path.as_ref().unwrap().clone(), packet))
        } else {
            None
        }
    }
}

// Tests the `paths`() iter and `path_ref`().
#[test]
fn packet_path_iter() {
    use crate::parse::Parse;
    use crate::PacketPile;

    fn paths<'a>(iter: impl Iterator<Item=&'a Packet>) -> Vec<Vec<usize>> {
        let mut lpaths : Vec<Vec<usize>> = Vec::new();
        for (i, packet) in iter.enumerate() {
            let mut v = Vec::new();
            v.push(i);
            lpaths.push(v);

            if let Some(container) = packet.container_ref() {
                if let Some(c) = container.children() {
                    for mut path in paths(c).into_iter()
                    {
                        path.insert(0, i);
                        lpaths.push(path);
                    }
                }
            }
        }
        lpaths
    }

    for i in 1..5 {
        let pile = PacketPile::from_bytes(
            crate::tests::message(&format!("recursive-{}.gpg", i)[..])).unwrap();

        let mut paths1 : Vec<Vec<usize>> = Vec::new();
        for path in paths(pile.children()).iter() {
            paths1.push(path.clone());
        }

        let mut paths2 : Vec<Vec<usize>> = Vec::new();
        for (path, packet) in pile.descendants().paths() {
            assert_eq!(Some(packet), pile.path_ref(&path[..]));
            paths2.push(path);
        }

        if paths1 != paths2 {
            eprintln!("PacketPile:");
            pile.pretty_print();

            eprintln!("Expected paths:");
            for p in paths1 {
                eprintln!("  {:?}", p);
            }

            eprintln!("Got paths:");
            for p in paths2 {
                eprintln!("  {:?}", p);
            }

            panic!("Something is broken.  Don't panic.");
        }
    }
}

/// Holds a signature packet.
///
/// Signature packets are used to hold all kinds of signatures
/// including certifications, and signatures over documents.  See
/// [Section 5.2 of RFC 4880] for details.
///
///   [Section 5.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2
///
/// When signing a document, a `Signature` packet is typically created
/// indirectly by the [streaming `Signer`].  Similarly, a `Signature`
/// packet is created as a side effect of parsing a signed message
/// using the [`PacketParser`].
///
/// `Signature` packets are also used for [self signatures on Keys],
/// [self signatures on User IDs], [self signatures on User
/// Attributes], [certifications of User IDs], and [certifications of
/// User Attributes].  In these cases, you'll typically want to use
/// the [`SignatureBuilder`] to create the `Signature` packet.  See
/// the linked documentation for details, and examples.
///
/// [streaming `Signer`]: crate::serialize::stream::Signer
/// [`PacketParser`]: crate::parse::PacketParser
/// [self signatures on Keys]: Key::bind()
/// [self signatures on User IDs]: UserID::bind()
/// [self signatures on User Attributes]: user_attribute::UserAttribute::bind()
/// [certifications of User IDs]: UserID::certify()
/// [certifications of User Attributes]: user_attribute::UserAttribute::certify()
/// [`SignatureBuilder`]: signature::SignatureBuilder
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// # A note on equality
///
/// Two `Signature` packets are considered equal if their serialized
/// form is equal.  Notably this includes the unhashed subpacket area
/// and the order of subpackets and notations.  This excludes the
/// computed digest and signature level, which are not serialized.
///
/// A consequence of considering packets in the unhashed subpacket
/// area is that an adversary can take a valid signature and create
/// many distinct but valid signatures by changing the unhashed
/// subpacket area.  This has the potential of creating a denial of
/// service vector, if `Signature`s are naively deduplicated.  To
/// protect against this, consider using [`Signature::normalized_eq`].
///
///   [`Signature::normalized_eq`]: Signature::normalized_eq()
///
/// # Examples
///
/// Add a User ID to an existing certificate:
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
///     .add_userid("Alice <alice@example.org>")
///     .generate()?;
///
/// // Add a new User ID.
/// let mut signer = cert
///     .primary_key().key().clone().parts_into_secret()?.into_keypair()?;
///
/// // Use the existing User ID's signature as a template.  This ensures that
/// // we use the same
/// let userid = UserID::from("Alice <alice@other.com>");
/// let template: signature::SignatureBuilder
///     = cert.with_policy(p, t1)?.primary_userid().unwrap()
///         .binding_signature().clone().into();
/// let sig = template.clone()
///     .set_signature_creation_time(t2)?;
/// let sig = userid.bind(&mut signer, &cert, sig)?;
///
/// let cert = cert.insert_packets(vec![Packet::from(userid), sig.into()])?;
/// # assert_eq!(cert.with_policy(p, t2)?.userids().count(), 2);
/// # Ok(()) }
/// ```
#[non_exhaustive]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
pub enum Signature {
    /// Signature packet version 3.
    V3(self::signature::Signature3),

    /// Signature packet version 4.
    V4(self::signature::Signature4),
}
assert_send_and_sync!(Signature);

impl Signature {
    /// Gets the version.
    pub fn version(&self) -> u8 {
        match self {
            Signature::V3(_) => 3,
            Signature::V4(_) => 4,
        }
    }
}

impl From<Signature> for Packet {
    fn from(s: Signature) -> Self {
        Packet::Signature(s)
    }
}

// Trivial forwarder for singleton enum.
impl Deref for Signature {
    type Target = signature::Signature4;

    fn deref(&self) -> &Self::Target {
        match self {
            Signature::V3(sig) => &sig.intern,
            Signature::V4(sig) => sig,
        }
    }
}

// Trivial forwarder for singleton enum.
impl DerefMut for Signature {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Signature::V3(ref mut sig) => &mut sig.intern,
            Signature::V4(ref mut sig) => sig,
        }
    }
}

/// Holds a one-pass signature packet.
///
/// See [Section 5.4 of RFC 4880] for details.
///
/// A `OnePassSig` packet is not normally instantiated directly.  In
/// most cases, you'll create one as a side-effect of signing a
/// message using the [streaming serializer], or parsing a signed
/// message using the [`PacketParser`].
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// [Section 5.4 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.4
/// [`PacketParser`]: crate::parse::PacketParser
/// [streaming serializer]: crate::serialize::stream
#[non_exhaustive]
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum OnePassSig {
    /// OnePassSig packet version 3.
    V3(self::one_pass_sig::OnePassSig3),
}
assert_send_and_sync!(OnePassSig);

impl OnePassSig {
    /// Gets the version.
    pub fn version(&self) -> u8 {
        match self {
            OnePassSig::V3(_) => 3,
        }
    }
}

impl From<OnePassSig> for Packet {
    fn from(s: OnePassSig) -> Self {
        Packet::OnePassSig(s)
    }
}

// Trivial forwarder for singleton enum.
impl Deref for OnePassSig {
    type Target = one_pass_sig::OnePassSig3;

    fn deref(&self) -> &Self::Target {
        match self {
            OnePassSig::V3(ops) => ops,
        }
    }
}

// Trivial forwarder for singleton enum.
impl DerefMut for OnePassSig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            OnePassSig::V3(ref mut ops) => ops,
        }
    }
}

/// Holds an asymmetrically encrypted session key.
///
/// The session key is used to decrypt the actual ciphertext, which is
/// typically stored in a [SEIP] or [AED] packet.  See [Section 5.1 of
/// RFC 4880] for details.
///
/// A PKESK packet is not normally instantiated directly.  In most
/// cases, you'll create one as a side-effect of encrypting a message
/// using the [streaming serializer], or parsing an encrypted message
/// using the [`PacketParser`].
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// [Section 5.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.1
/// [streaming serializer]: crate::serialize::stream
/// [`PacketParser`]: crate::parse::PacketParser
#[non_exhaustive]
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum PKESK {
    /// PKESK packet version 3.
    V3(self::pkesk::PKESK3),
}
assert_send_and_sync!(PKESK);

impl PKESK {
    /// Gets the version.
    pub fn version(&self) -> u8 {
        match self {
            PKESK::V3(_) => 3,
        }
    }
}

impl From<PKESK> for Packet {
    fn from(p: PKESK) -> Self {
        Packet::PKESK(p)
    }
}

// Trivial forwarder for singleton enum.
impl Deref for PKESK {
    type Target = self::pkesk::PKESK3;

    fn deref(&self) -> &Self::Target {
        match self {
            PKESK::V3(ref p) => p,
        }
    }
}

// Trivial forwarder for singleton enum.
impl DerefMut for PKESK {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            PKESK::V3(ref mut p) => p,
        }
    }
}

/// Holds a symmetrically encrypted session key.
///
/// The session key is used to decrypt the actual ciphertext, which is
/// typically stored in a [SEIP] or [AED] packet.  See [Section 5.3 of
/// RFC 4880] for details.
///
/// An SKESK packet is not normally instantiated directly.  In most
/// cases, you'll create one as a side-effect of encrypting a message
/// using the [streaming serializer], or parsing an encrypted message
/// using the [`PacketParser`].
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// [Section 5.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.3
/// [streaming serializer]: crate::serialize::stream
/// [`PacketParser`]: crate::parse::PacketParser
#[non_exhaustive]
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum SKESK {
    /// SKESK packet version 4.
    V4(self::skesk::SKESK4),
    /// SKESK packet version 5.
    ///
    /// This feature is [experimental](super#experimental-features).
    V5(self::skesk::SKESK5),
}
assert_send_and_sync!(SKESK);

impl SKESK {
    /// Gets the version.
    pub fn version(&self) -> u8 {
        match self {
            SKESK::V4(_) => 4,
            SKESK::V5(_) => 5,
        }
    }
}

impl From<SKESK> for Packet {
    fn from(p: SKESK) -> Self {
        Packet::SKESK(p)
    }
}

/// Holds a public key, public subkey, private key or private subkey packet.
///
/// The different `Key` packets are described in [Section 5.5 of RFC 4880].
///
///   [Section 5.5 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.5
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// # Key Variants
///
/// There are four different types of keys in OpenPGP: [public keys],
/// [secret keys], [public subkeys], and [secret subkeys].  Although
/// the semantics of each type of key are slightly different, the
/// underlying representation is identical (even a public key and a
/// secret key are the same: the public key variant just contains 0
/// bits of secret key material).
///
/// In Sequoia, we use a single type, `Key`, for all four variants.
/// To improve type safety, we use marker traits rather than an `enum`
/// to distinguish them.  Specifically, we `Key` is generic over two
/// type variables, `P` and `R`.
///
/// `P` and `R` take marker traits, which describe how any secret key
/// material should be treated, and the key's role (primary or
/// subordinate).  The markers also determine the `Key`'s behavior and
/// the exposed functionality.  `P` can be [`key::PublicParts`],
/// [`key::SecretParts`], or [`key::UnspecifiedParts`].  And, `R` can
/// be [`key::PrimaryRole`], [`key::SubordinateRole`], or
/// [`key::UnspecifiedRole`].
///
/// If `P` is `key::PublicParts`, any secret key material that is
/// present is ignored.  For instance, when serializing a key with
/// this marker, any secret key material will be skipped.  This is
/// illutrated in the following example.  If `P` is
/// `key::SecretParts`, then the key definitely contains secret key
/// material (although it is not guaranteed that the secret key
/// material is valid), and methods that require secret key material
/// are available.
///
/// Unlike `P`, `R` does not say anything about the `Key`'s content.
/// But, a key's role does influence's the key's semantics.  For
/// instance, some of a primary key's meta-data is located on the
/// primary User ID whereas a subordinate key's meta-data is located
/// on its binding signature.
///
/// The unspecified variants [`key::UnspecifiedParts`] and
/// [`key::UnspecifiedRole`] exist to simplify type erasure, which is
/// needed to mix different types of keys in a single collection.  For
/// instance, [`Cert::keys`] returns an iterator over the keys in a
/// certificate.  Since the keys have different roles (a primary key
/// and zero or more subkeys), but the `Iterator` has to be over a
/// single, fixed type, the returned keys use the
/// `key::UnspecifiedRole` marker.
///
/// [public keys]: https://tools.ietf.org/html/rfc4880#section-5.5.1.1
/// [secret keys]: https://tools.ietf.org/html/rfc4880#section-5.5.1.3
/// [public subkeys]: https://tools.ietf.org/html/rfc4880#section-5.5.1.2
/// [secret subkeys]: https://tools.ietf.org/html/rfc4880#section-5.5.1.4
/// [`Cert::keys`]: super::Cert::keys()
///
/// ## Examples
///
/// Serializing a public key with secret key material drops the secret
/// key material:
///
/// ```
/// use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
/// use openpgp::packet::prelude::*;
/// use sequoia_openpgp::parse::Parse;
/// use openpgp::serialize::Serialize;
///
/// # fn main() -> openpgp::Result<()> {
/// // Generate a new certificate.  It has secret key material.
/// let (cert, _) = CertBuilder::new()
///     .generate()?;
///
/// let pk = cert.primary_key().key();
/// assert!(pk.has_secret());
///
/// // Serializing a `Key<key::PublicParts, _>` drops the secret key
/// // material.
/// let mut bytes = Vec::new();
/// Packet::from(pk.clone()).serialize(&mut bytes);
/// let p : Packet = Packet::from_bytes(&bytes)?;
///
/// if let Packet::PublicKey(key) = p {
///     assert!(! key.has_secret());
/// } else {
///     unreachable!();
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Conversions
///
/// Sometimes it is necessary to change a marker.  For instance, to
/// help prevent a user from inadvertently leaking secret key
/// material, the [`Cert`] data structure never returns keys with the
/// [`key::SecretParts`] marker.  This means, to use any secret key
/// material, e.g., when creating a [`Signer`], the user needs to
/// explicitly opt-in by changing the marker using
/// [`Key::parts_into_secret`] or [`Key::parts_as_secret`].
///
/// For `P`, the conversion functions are: [`Key::parts_into_public`],
/// [`Key::parts_as_public`], [`Key::parts_into_secret`],
/// [`Key::parts_as_secret`], [`Key::parts_into_unspecified`], and
/// [`Key::parts_as_unspecified`].  With the exception of converting
/// `P` to `key::SecretParts`, these functions are infallible.
/// Converting `P` to `key::SecretParts` may fail if the key doesn't
/// have any secret key material.  (Note: although the secret key
/// material is required, it not checked for validity.)
///
/// For `R`, the conversion functions are [`Key::role_into_primary`],
/// [`Key::role_as_primary`], [`Key::role_into_subordinate`],
/// [`Key::role_as_subordinate`], [`Key::role_into_unspecified`], and
/// [`Key::role_as_unspecified`].
///
/// It is also possible to use `From`.
///
/// [`Signer`]: super::crypto::Signer
/// [`Key::parts_as_secret`]: Key::parts_as_secret()
/// [`Key::parts_into_public`]: Key::parts_into_public()
/// [`Key::parts_as_public`]: Key::parts_as_public()
/// [`Key::parts_into_secret`]: Key::parts_into_secret()
/// [`Key::parts_as_secret`]: Key::parts_as_secret()
/// [`Key::parts_into_unspecified`]: Key::parts_into_unspecified()
/// [`Key::parts_as_unspecified`]: Key::parts_as_unspecified()
/// [`Key::role_into_primary`]: Key::role_into_primary()
/// [`Key::role_as_primary`]: Key::role_as_primary()
/// [`Key::role_into_subordinate`]: Key::role_into_subordinate()
/// [`Key::role_as_subordinate`]: Key::role_as_subordinate()
/// [`Key::role_into_unspecified`]: Key::role_into_unspecified()
/// [`Key::role_as_unspecified`]: Key::role_as_unspecified()
///
/// ## Examples
///
/// Changing a marker:
///
/// ```
/// use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
/// use openpgp::packet::prelude::*;
///
/// # fn main() -> openpgp::Result<()> {
/// // Generate a new certificate.  It has secret key material.
/// let (cert, _) = CertBuilder::new()
///     .generate()?;
///
/// let pk: &Key<key::PublicParts, key::PrimaryRole>
///     = cert.primary_key().key();
/// // `has_secret`s is one of the few methods that ignores the
/// // parts type.
/// assert!(pk.has_secret());
///
/// // Treat it like a secret key.  This only works if `pk` really
/// // has secret key material (which it does in this case, see above).
/// let sk = pk.parts_as_secret()?;
/// assert!(sk.has_secret());
///
/// // And back.
/// let pk = sk.parts_as_public();
/// // Yes, the secret key material is still there.
/// assert!(pk.has_secret());
/// # Ok(())
/// # }
/// ```
///
/// The [`Cert`] data structure only returns public keys.  To work
/// with any secret key material, the `Key` first needs to be
/// converted to a secret key.  This is necessary, for instance, when
/// creating a [`Signer`]:
///
/// [`Cert`]: super::Cert
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
/// let the_past = time::SystemTime::now() - time::Duration::from_secs(1);
/// let (cert, _) = CertBuilder::new()
///     .set_creation_time(the_past)
///     .generate()?;
///
/// // Set the certificate to expire now.  To do this, we need
/// // to create a new self-signature, and sign it using a
/// // certification-capable key.  The primary key is always
/// // certification capable.
/// let mut keypair = cert.primary_key()
///     .key().clone().parts_into_secret()?.into_keypair()?;
/// let sigs = cert.set_expiration_time(p, None, &mut keypair,
///                                     Some(time::SystemTime::now()))?;
///
/// let cert = cert.insert_packets(sigs)?;
/// // It's expired now.
/// assert!(cert.with_policy(p, None)?.alive().is_err());
/// # Ok(())
/// # }
/// ```
///
/// # Key Generation
///
/// `Key` is a wrapper around [the different key formats].
/// (Currently, Sequoia only supports version 4 keys, however, future
/// versions may add limited support for version 3 keys to facilitate
/// working with achieved messages, and RFC 4880bis includes [a
/// proposal for a new key format].)  As such, it doesn't provide a
/// mechanism to generate keys or import existing key material.
/// Instead, use the format-specific functions (e.g.,
/// [`Key4::generate_ecc`]) and then convert the result into a `Key`
/// packet, as the following example demonstrates.
///
/// [the different key formats]: https://tools.ietf.org/html/rfc4880#section-5.5.2
/// [a proposal for a new key format]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#section-5.5.2
/// [`Key4::generate_ecc`]: key::Key4::generate_ecc()
///
///
/// ## Examples
///
/// ```
/// use sequoia_openpgp as openpgp;
/// use openpgp::packet::prelude::*;
/// use openpgp::types::Curve;
///
/// # fn main() -> openpgp::Result<()> {
/// let key: Key<key::SecretParts, key::PrimaryRole>
///     = Key::from(Key4::generate_ecc(true, Curve::Ed25519)?);
/// # Ok(())
/// # }
/// ```
///
/// # Password Protection
///
/// OpenPGP provides a mechanism to [password protect keys].  If a key
/// is password protected, you need to decrypt the password using
/// [`Key::decrypt_secret`] before using its secret key material
/// (e.g., to decrypt a message, or to generate a signature).
///
/// [password protect keys]: https://tools.ietf.org/html/rfc4880#section-3.7
/// [`Key::decrypt_secret`]: Key::decrypt_secret()
///
/// # A note on equality
///
/// The implementation of `Eq` for `Key` compares the serialized form
/// of `Key`s.  Comparing or serializing values of `Key<PublicParts,
/// _>` ignore secret key material, whereas the secret key material is
/// considered and serialized for `Key<SecretParts, _>`, and for
/// `Key<UnspecifiedParts, _>` if present.  To explicitly exclude the
/// secret key material from the comparison, use [`Key::public_cmp`]
/// or [`Key::public_eq`].
///
/// When merging in secret key material from untrusted sources, you
/// need to be very careful: secret key material is not
/// cryptographically protected by the key's self signature.  Thus, an
/// attacker can provide a valid key with a valid self signature, but
/// invalid secret key material.  If naively merged, this could
/// overwrite valid secret key material, and thereby render the key
/// useless.  Unfortunately, the only way to find out that the secret
/// key material is bad is to actually try using it.  But, because the
/// secret key material is usually encrypted, this can't always be
/// done automatically.
///
/// [`Key::public_cmp`]: Key::public_cmp()
/// [`Key::public_eq`]: Key::public_eq()
///
/// Compare:
///
/// ```
/// use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
/// use openpgp::packet::prelude::*;
/// use openpgp::packet::key::*;
///
/// # fn main() -> openpgp::Result<()> {
/// // Generate a new certificate.  It has secret key material.
/// let (cert, _) = CertBuilder::new()
///     .generate()?;
///
/// let sk: &Key<PublicParts, _> = cert.primary_key().key();
/// assert!(sk.has_secret());
///
/// // Strip the secret key material.
/// let cert = cert.clone().strip_secret_key_material();
/// let pk: &Key<PublicParts, _> = cert.primary_key().key();
/// assert!(! pk.has_secret());
///
/// // Eq on Key<PublicParts, _> compares only the public bits, so it
/// // considers pk and sk to be equal.
/// assert_eq!(pk, sk);
///
/// // Convert to Key<UnspecifiedParts, _>.
/// let sk: &Key<UnspecifiedParts, _> = sk.parts_as_unspecified();
/// let pk: &Key<UnspecifiedParts, _> = pk.parts_as_unspecified();
///
/// // Eq on Key<UnspecifiedParts, _> compares both the public and the
/// // secret bits, so it considers pk and sk to be different.
/// assert_ne!(pk, sk);
///
/// // In any case, Key::public_eq only compares the public bits,
/// // so it considers them to be equal.
/// assert!(Key::public_eq(pk, sk));
/// # Ok(())
/// # }
/// ```
#[non_exhaustive]
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum Key<P: key::KeyParts, R: key::KeyRole> {
    /// A version 4 `Key` packet.
    V4(Key4<P, R>),
}
assert_send_and_sync!(Key<P, R> where P: key::KeyParts, R: key::KeyRole);

impl<P: key::KeyParts, R: key::KeyRole> fmt::Display for Key<P, R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Key::V4(k) => k.fmt(f),
        }
    }
}

impl<P: key::KeyParts, R: key::KeyRole> Key<P, R> {
    /// Gets the version.
    pub fn version(&self) -> u8 {
        match self {
            Key::V4(_) => 4,
        }
    }

    /// Compares the public bits of two keys.
    ///
    /// This returns `Ordering::Equal` if the public MPIs, version,
    /// creation time and algorithm of the two `Key`s match.  This
    /// does not consider the packet's encoding, packet's tag or the
    /// secret key material.
    pub fn public_cmp<PB, RB>(&self, b: &Key<PB, RB>) -> std::cmp::Ordering
        where PB: key::KeyParts,
              RB: key::KeyRole,
    {
        match (self, b) {
            (Key::V4(a), Key::V4(b)) => a.public_cmp(b),
        }
    }

    /// This method tests for self and other values to be equal modulo
    /// the secret key material.
    ///
    /// This returns true if the public MPIs, creation time and
    /// algorithm of the two `Key`s match.  This does not consider
    /// the packet's encoding, packet's tag or the secret key
    /// material.
    pub fn public_eq<PB, RB>(&self, b: &Key<PB, RB>) -> bool
        where PB: key::KeyParts,
              RB: key::KeyRole,
    {
        self.public_cmp(b) == std::cmp::Ordering::Equal
    }
}

impl From<Key<key::PublicParts, key::PrimaryRole>> for Packet {
    /// Convert the `Key` struct to a `Packet`.
    fn from(k: Key<key::PublicParts, key::PrimaryRole>) -> Self {
        Packet::PublicKey(k)
    }
}

impl From<Key<key::PublicParts, key::SubordinateRole>> for Packet {
    /// Convert the `Key` struct to a `Packet`.
    fn from(k: Key<key::PublicParts, key::SubordinateRole>) -> Self {
        Packet::PublicSubkey(k)
    }
}

impl From<Key<key::SecretParts, key::PrimaryRole>> for Packet {
    /// Convert the `Key` struct to a `Packet`.
    fn from(k: Key<key::SecretParts, key::PrimaryRole>) -> Self {
        Packet::SecretKey(k)
    }
}

impl From<Key<key::SecretParts, key::SubordinateRole>> for Packet {
    /// Convert the `Key` struct to a `Packet`.
    fn from(k: Key<key::SecretParts, key::SubordinateRole>) -> Self {
        Packet::SecretSubkey(k)
    }
}

impl<R: key::KeyRole> Key<key::SecretParts, R> {
    /// Creates a new key pair from a `Key` with an unencrypted
    /// secret key.
    ///
    /// If the `Key` is password protected, you first need to decrypt
    /// it using [`Key::decrypt_secret`].
    ///
    /// [`Key::decrypt_secret`]: Key::decrypt_secret()
    ///
    /// # Errors
    ///
    /// Fails if the secret key is encrypted.
    ///
    /// # Examples
    ///
    /// Revoke a certificate by signing a new revocation certificate:
    ///
    /// ```rust
    /// use std::time;
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::crypto::KeyPair;
    /// use openpgp::types::ReasonForRevocation;
    ///
    /// # fn main() -> Result<()> {
    /// // Generate a certificate.
    /// let (cert, _) =
    ///     CertBuilder::general_purpose(None,
    ///                                  Some("Alice Lovelace <alice@example.org>"))
    ///         .generate()?;
    ///
    /// // Use the secret key material to sign a revocation certificate.
    /// let mut keypair = cert.primary_key()
    ///     .key().clone().parts_into_secret()?
    ///     .into_keypair()?;
    /// let rev = cert.revoke(&mut keypair,
    ///                       ReasonForRevocation::KeyCompromised,
    ///                       b"It was the maid :/")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn into_keypair(self) -> Result<KeyPair> {
        match self {
            Key::V4(k) => k.into_keypair(),
        }
    }

    /// Decrypts the secret key material.
    ///
    /// In OpenPGP, secret key material can be [protected with a
    /// password].  The password is usually hardened using a [KDF].
    ///
    /// [protected with a password]: https://tools.ietf.org/html/rfc4880#section-5.5.3
    /// [KDF]: https://tools.ietf.org/html/rfc4880#section-3.7
    ///
    /// This function takes ownership of the `Key`, decrypts the
    /// secret key material using the password, and returns a new key
    /// whose secret key material is not password protected.
    ///
    /// If the secret key material is not password protected or if the
    /// password is wrong, this function returns an error.
    ///
    /// # Examples
    ///
    /// Sign a new revocation certificate using a password-protected
    /// key:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::types::ReasonForRevocation;
    ///
    /// # fn main() -> Result<()> {
    /// // Generate a certificate whose secret key material is
    /// // password protected.
    /// let (cert, _) =
    ///     CertBuilder::general_purpose(None,
    ///                                  Some("Alice Lovelace <alice@example.org>"))
    ///         .set_password(Some("1234".into()))
    ///         .generate()?;
    ///
    /// // Use the secret key material to sign a revocation certificate.
    /// let key = cert.primary_key().key().clone().parts_into_secret()?;
    ///
    /// // We can't turn it into a keypair without decrypting it.
    /// assert!(key.clone().into_keypair().is_err());
    ///
    /// // And, we need to use the right password.
    /// assert!(key.clone()
    ///     .decrypt_secret(&"correct horse battery staple".into())
    ///     .is_err());
    ///
    /// // Let's do it right:
    /// let mut keypair = key.decrypt_secret(&"1234".into())?.into_keypair()?;
    /// let rev = cert.revoke(&mut keypair,
    ///                       ReasonForRevocation::KeyCompromised,
    ///                       b"It was the maid :/")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn decrypt_secret(self, password: &Password) -> Result<Self>
    {
        match self {
            Key::V4(k) => Ok(Key::V4(k.decrypt_secret(password)?)),
        }
    }

    /// Encrypts the secret key material.
    ///
    /// In OpenPGP, secret key material can be [protected with a
    /// password].  The password is usually hardened using a [KDF].
    ///
    /// [protected with a password]: https://tools.ietf.org/html/rfc4880#section-5.5.3
    /// [KDF]: https://tools.ietf.org/html/rfc4880#section-3.7
    ///
    /// This function takes ownership of the `Key`, encrypts the
    /// secret key material using the password, and returns a new key
    /// whose secret key material is protected with the password.
    ///
    /// If the secret key material is already password protected, this
    /// function returns an error.
    ///
    /// # Examples
    ///
    /// This example demonstrates how to encrypt the secret key
    /// material of every key in a certificate.  Decryption can be
    /// done the same way with [`Key::decrypt_secret`].
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::Packet;
    ///
    /// # fn main() -> Result<()> {
    /// // Generate a certificate whose secret key material is
    /// // not password protected.
    /// let (cert, _) =
    ///     CertBuilder::general_purpose(None,
    ///                                  Some("Alice Lovelace <alice@example.org>"))
    ///         .generate()?;
    ///
    /// // Encrypt every key.
    /// let mut encrypted_keys: Vec<Packet> = Vec::new();
    /// for ka in cert.keys().secret() {
    ///     assert!(ka.has_unencrypted_secret());
    ///
    ///     // Encrypt the key's secret key material.
    ///     let key = ka.key().clone().encrypt_secret(&"1234".into())?;
    ///     assert!(! key.has_unencrypted_secret());
    ///
    ///     // We cannot merge it right now, because `cert` is borrowed.
    ///     encrypted_keys.push(if ka.primary() {
    ///         key.role_into_primary().into()
    ///     } else {
    ///         key.role_into_subordinate().into()
    ///     });
    /// }
    ///
    /// // Merge the keys into the certificate.  Note: `Cert::insert_packets`
    /// // prefers added versions of keys.  So, the encrypted version
    /// // will override the decrypted version.
    /// let cert = cert.insert_packets(encrypted_keys)?;
    ///
    /// // Now the every key's secret key material is encrypted.  We'll
    /// // demonstrate this using the primary key:
    /// let key = cert.primary_key().key().parts_as_secret()?;
    /// assert!(! key.has_unencrypted_secret());
    ///
    /// // We can't turn it into a keypair without decrypting it.
    /// assert!(key.clone().into_keypair().is_err());
    ///
    /// // And, we need to use the right password.
    /// assert!(key.clone()
    ///     .decrypt_secret(&"correct horse battery staple".into())
    ///     .is_err());
    ///
    /// // Let's do it right:
    /// let mut keypair = key.clone()
    ///     .decrypt_secret(&"1234".into())?.into_keypair()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn encrypt_secret(self, password: &Password) -> Result<Self>
    {
        match self {
            Key::V4(k) => Ok(Key::V4(k.encrypt_secret(password)?)),
        }
    }
}

impl<R: key::KeyRole> Key4<key::SecretParts, R> {
    /// Creates a new key pair from a secret `Key` with an unencrypted
    /// secret key.
    ///
    /// # Errors
    ///
    /// Fails if the secret key is encrypted.  You can use
    /// [`Key::decrypt_secret`] to decrypt a key.
    pub fn into_keypair(self) -> Result<KeyPair> {
        let (key, secret) = self.take_secret();
        let secret = match secret {
            SecretKeyMaterial::Unencrypted(secret) => secret,
            SecretKeyMaterial::Encrypted(_) =>
                return Err(Error::InvalidArgument(
                    "secret key material is encrypted".into()).into()),
        };

        KeyPair::new(key.role_into_unspecified().into(), secret)
    }
}

macro_rules! impl_common_secret_functions {
    ($t: path) => {
        /// Secret key handling.
        impl<R: key::KeyRole> Key<$t, R> {
            /// Takes the key packet's `SecretKeyMaterial`, if any.
            pub fn take_secret(self)
                               -> (Key<key::PublicParts, R>,
                                   Option<key::SecretKeyMaterial>)
            {
                match self {
                    Key::V4(k) => {
                        let (k, s) = k.take_secret();
                        (k.into(), s)
                    },
                }
            }

            /// Adds `SecretKeyMaterial` to the packet, returning the old if
            /// any.
            pub fn add_secret(self, secret: key::SecretKeyMaterial)
                              -> (Key<key::SecretParts, R>,
                                  Option<key::SecretKeyMaterial>)
            {
                match self {
                    Key::V4(k) => {
                        let (k, s) = k.add_secret(secret);
                        (k.into(), s)
                    },
                }
            }
        }
    }
}
impl_common_secret_functions!(key::PublicParts);
impl_common_secret_functions!(key::UnspecifiedParts);

/// Secret key handling.
impl<R: key::KeyRole> Key<key::SecretParts, R> {
    /// Takes the key packet's `SecretKeyMaterial`.
    pub fn take_secret(self)
                       -> (Key<key::PublicParts, R>, key::SecretKeyMaterial)
    {
        match self {
            Key::V4(k) => {
                let (k, s) = k.take_secret();
                (k.into(), s)
            },
        }
    }

    /// Adds `SecretKeyMaterial` to the packet, returning the old.
    pub fn add_secret(self, secret: key::SecretKeyMaterial)
                      -> (Key<key::SecretParts, R>, key::SecretKeyMaterial)
    {
        match self {
            Key::V4(k) => {
                let (k, s) = k.add_secret(secret);
                (k.into(), s)
            },
        }
    }
}


// Trivial forwarder for singleton enum.
impl<P: key::KeyParts, R: key::KeyRole> Deref for Key<P, R> {
    type Target = Key4<P, R>;

    fn deref(&self) -> &Self::Target {
        match self {
            Key::V4(ref p) => p,
        }
    }
}

// Trivial forwarder for singleton enum.
impl<P: key::KeyParts, R: key::KeyRole> DerefMut for Key<P, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Key::V4(ref mut p) => p,
        }
    }
}

/// Holds a SEIP packet.
///
/// A SEIP packet holds encrypted data.  The data contains additional
/// OpenPGP packets.  See [Section 5.13 of RFC 4880] for details.
///
/// A SEIP packet is not normally instantiated directly.  In most
/// cases, you'll create one as a side-effect of encrypting a message
/// using the [streaming serializer], or parsing an encrypted message
/// using the [`PacketParser`].
///
/// [Section 5.13 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.13
/// [streaming serializer]: crate::serialize::stream
/// [`PacketParser`]: crate::parse::PacketParser
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum SEIP {
    /// SEIP packet version 1.
    V1(self::seip::SEIP1),
}
assert_send_and_sync!(SEIP);

impl SEIP {
    /// Gets the version.
    pub fn version(&self) -> u8 {
        match self {
            SEIP::V1(_) => 1,
        }
    }
}

impl From<SEIP> for Packet {
    fn from(p: SEIP) -> Self {
        Packet::SEIP(p)
    }
}

// Trivial forwarder for singleton enum.
impl Deref for SEIP {
    type Target = self::seip::SEIP1;

    fn deref(&self) -> &Self::Target {
        match self {
            SEIP::V1(ref p) => p,
        }
    }
}

// Trivial forwarder for singleton enum.
impl DerefMut for SEIP {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            SEIP::V1(ref mut p) => p,
        }
    }
}

/// Holds an AEAD encrypted data packet.
///
/// An AEAD packet holds encrypted data.  It is contains additional
/// OpenPGP packets.  See [Section 5.16 of RFC 4880bis] for details.
///
/// [Section 5.16 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-05#section-5.16
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// An AEAD packet is not normally instantiated directly.  In most
/// cases, you'll create one as a side-effect of encrypting a message
/// using the [streaming serializer], or parsing an encrypted message
/// using the [`PacketParser`].
///
/// [streaming serializer]: crate::serialize::stream
/// [`PacketParser`]: crate::parse::PacketParser
///
/// This feature is [experimental](super#experimental-features).
#[non_exhaustive]
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum AED {
    /// AED packet version 1.
    V1(self::aed::AED1),
}
assert_send_and_sync!(AED);

impl AED {
    /// Gets the version.
    pub fn version(&self) -> u8 {
        match self {
            AED::V1(_) => 1,
        }
    }
}

impl From<AED> for Packet {
    fn from(p: AED) -> Self {
        Packet::AED(p)
    }
}

// Trivial forwarder for singleton enum.
impl Deref for AED {
    type Target = self::aed::AED1;

    fn deref(&self) -> &Self::Target {
        match self {
            AED::V1(ref p) => p,
        }
    }
}

// Trivial forwarder for singleton enum.
impl DerefMut for AED {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            AED::V1(ref mut p) => p,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::serialize::SerializeInto;
    use crate::parse::Parse;

    quickcheck! {
        fn roundtrip(p: Packet) -> bool {
            let buf = p.to_vec().expect("Failed to serialize packet");
            let q = Packet::from_bytes(&buf).unwrap();
            assert_eq!(p, q);
            true
        }
    }

    quickcheck! {
        /// Given a packet and a position, induces a bit flip in the
        /// serialized form, then checks that PartialEq detects that.
        /// Recall that for packets, PartialEq is defined using the
        /// serialized form.
        fn mutate_eq_discriminates(p: Packet, i: usize) -> bool {
            if p.tag() == Tag::CompressedData {
                // Mutating compressed data streams is not that
                // trivial, because there are bits we can flip without
                // changing the decompressed data.
                return true;
            }

            let mut buf = p.to_vec().unwrap();
            let bit =
                // Avoid first two bytes so that we don't change the
                // type and reduce the chance of changing the length.
                i.saturating_add(16)
                % (buf.len() * 8);
            buf[bit / 8] ^= 1 << (bit % 8);
            match Packet::from_bytes(&buf) {
                Ok(q) => p != q,
                Err(_) => true, // Packet failed to parse.
            }
        }
    }

    /// Problem on systems with 32-bit time_t.
    #[test]
    fn issue_802() -> Result<()> {
        let pp = crate::PacketPile::from_bytes(b"-----BEGIN PGP ARMORED FILE-----

xiEE/////xIJKyQDAwIIAQENAFYp8M2JngCfc04tIwMBCuU=
-----END PGP ARMORED FILE-----
")?;
        let p = pp.path_ref(&[0]).unwrap();
        let buf = p.to_vec().expect("Failed to serialize packet");
        let q = Packet::from_bytes(&buf).unwrap();
        assert_eq!(p, &q);
        Ok(())
    }
}
