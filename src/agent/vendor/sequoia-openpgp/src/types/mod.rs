//! Primitive types.
//!
//! This module provides types used in OpenPGP, like enumerations
//! describing algorithms.
//!
//! # Common Operations
//!
//!  - *Rounding the creation time of signatures*: See the [`Timestamp::round_down`] method.
//!  - *Checking key usage flags*: See the [`KeyFlags`] data structure.
//!  - *Setting key validity ranges*: See the [`Timestamp`] and [`Duration`] data structures.
//!
//! # Data structures
//!
//! ## `CompressionLevel`
//!
//! Allows adjusting the amount of effort spent on compressing encoded data.
//! This structure additionally has several helper methods for commonly used
//! compression strategies.
//!
//! ## `Features`
//!
//! Describes particular features supported by the given OpenPGP implementation.
//!
//! ## `KeyFlags`
//!
//! Holds imformation about a key in particular how the given key can be used.
//!
//! ## `RevocationKey`
//!
//! Describes a key that has been designated to issue revocation signatures.
//!
//! # `KeyServerPreferences`
//!
//! Describes preferences regarding to key servers.
//!
//! ## `Timestamp` and `Duration`
//!
//! In OpenPGP time is represented as the number of seconds since the UNIX epoch stored
//! as an `u32`. These two data structures allow manipulating OpenPGP time ensuring
//! that adding or subtracting durations will never overflow or underflow without
//! notice.
//!
//! [`Timestamp::round_down`]: Timestamp::round_down()

use std::fmt;
use std::str::FromStr;
use std::result;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::Error;
use crate::Result;

mod bitfield;
pub(crate) use bitfield::Bitfield;
mod compression_level;
pub use compression_level::CompressionLevel;
mod features;
pub use self::features::Features;
mod key_flags;
pub use self::key_flags::KeyFlags;
mod revocation_key;
pub use revocation_key::RevocationKey;
mod server_preferences;
pub use self::server_preferences::KeyServerPreferences;
mod timestamp;
pub use timestamp::{Timestamp, Duration};
pub(crate) use timestamp::normalize_systemtime;

pub(crate) trait Sendable : Send {}
pub(crate) trait Syncable : Sync {}

/// The OpenPGP public key algorithms as defined in [Section 9.1 of
/// RFC 4880], and [Section 5 of RFC 6637].
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// # Examples
///
/// ```rust
/// # fn main() -> sequoia_openpgp::Result<()> {
/// use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
/// use openpgp::types::PublicKeyAlgorithm;
///
/// let (cert, _) = CertBuilder::new()
///     .set_cipher_suite(CipherSuite::Cv25519)
///     .generate()?;
///
/// assert_eq!(cert.primary_key().pk_algo(), PublicKeyAlgorithm::EdDSA);
/// # Ok(()) }
/// ```
///
///   [Section 9.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-9.1
///   [Section 5 of RFC 6637]: https://tools.ietf.org/html/rfc6637
#[non_exhaustive]
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum PublicKeyAlgorithm {
    /// RSA (Encrypt or Sign)
    RSAEncryptSign,
    /// RSA Encrypt-Only, deprecated in RFC 4880.
    #[deprecated(note = "Use `PublicKeyAlgorithm::RSAEncryptSign`.")]
    RSAEncrypt,
    /// RSA Sign-Only, deprecated in RFC 4880.
    #[deprecated(note = "Use `PublicKeyAlgorithm::RSAEncryptSign`.")]
    RSASign,
    /// ElGamal (Encrypt-Only)
    ElGamalEncrypt,
    /// DSA (Digital Signature Algorithm)
    DSA,
    /// Elliptic curve DH
    ECDH,
    /// Elliptic curve DSA
    ECDSA,
    /// ElGamal (Encrypt or Sign), deprecated in RFC 4880.
    #[deprecated(note = "If you really must, use \
                         `PublicKeyAlgorithm::ElGamalEncrypt`.")]
    ElGamalEncryptSign,
    /// "Twisted" Edwards curve DSA
    EdDSA,
    /// Private algorithm identifier.
    Private(u8),
    /// Unknown algorithm identifier.
    Unknown(u8),
}
assert_send_and_sync!(PublicKeyAlgorithm);

#[allow(deprecated)]
const PUBLIC_KEY_ALGORITHM_VARIANTS: [PublicKeyAlgorithm; 9] = [
    PublicKeyAlgorithm::RSAEncryptSign,
    PublicKeyAlgorithm::RSAEncrypt,
    PublicKeyAlgorithm::RSASign,
    PublicKeyAlgorithm::ElGamalEncrypt,
    PublicKeyAlgorithm::DSA,
    PublicKeyAlgorithm::ECDH,
    PublicKeyAlgorithm::ECDSA,
    PublicKeyAlgorithm::ElGamalEncryptSign,
    PublicKeyAlgorithm::EdDSA,
];

impl PublicKeyAlgorithm {
    /// Returns true if the algorithm can sign data.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::PublicKeyAlgorithm;
    ///
    /// assert!(PublicKeyAlgorithm::EdDSA.for_signing());
    /// assert!(PublicKeyAlgorithm::RSAEncryptSign.for_signing());
    /// assert!(!PublicKeyAlgorithm::ElGamalEncrypt.for_signing());
    /// ```
    pub fn for_signing(&self) -> bool {
        use self::PublicKeyAlgorithm::*;
        #[allow(deprecated)] {
            matches!(self, RSAEncryptSign
                     | RSASign
                     | DSA
                     | ECDSA
                     | ElGamalEncryptSign
                     | EdDSA
                     | Private(_)
                     | Unknown(_)
            )
        }
    }

    /// Returns true if the algorithm can encrypt data.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::PublicKeyAlgorithm;
    ///
    /// assert!(!PublicKeyAlgorithm::EdDSA.for_encryption());
    /// assert!(PublicKeyAlgorithm::RSAEncryptSign.for_encryption());
    /// assert!(PublicKeyAlgorithm::ElGamalEncrypt.for_encryption());
    /// ```
    pub fn for_encryption(&self) -> bool {
        use self::PublicKeyAlgorithm::*;
        #[allow(deprecated)] {
            matches!(self, RSAEncryptSign
                     | RSAEncrypt
                     | ElGamalEncrypt
                     | ECDH
                     | ElGamalEncryptSign
                     | Private(_)
                     | Unknown(_)
            )
        }
    }

    /// Returns whether this algorithm is supported.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::PublicKeyAlgorithm;
    ///
    /// assert!(PublicKeyAlgorithm::EdDSA.is_supported());
    /// assert!(PublicKeyAlgorithm::RSAEncryptSign.is_supported());
    /// assert!(!PublicKeyAlgorithm::ElGamalEncrypt.is_supported());
    /// assert!(!PublicKeyAlgorithm::Private(101).is_supported());
    /// ```
    pub fn is_supported(&self) -> bool {
        self.is_supported_by_backend()
    }

    /// Returns an iterator over all valid variants.
    ///
    /// Returns an iterator over all known variants.  This does not
    /// include the [`PublicKeyAlgorithm::Private`], or
    /// [`PublicKeyAlgorithm::Unknown`] variants.
    pub fn variants() -> impl Iterator<Item=Self> {
        PUBLIC_KEY_ALGORITHM_VARIANTS.iter().cloned()
    }
}

impl From<u8> for PublicKeyAlgorithm {
    fn from(u: u8) -> Self {
        use crate::PublicKeyAlgorithm::*;
        #[allow(deprecated)]
        match u {
            1 => RSAEncryptSign,
            2 => RSAEncrypt,
            3 => RSASign,
            16 => ElGamalEncrypt,
            17 => DSA,
            18 => ECDH,
            19 => ECDSA,
            20 => ElGamalEncryptSign,
            22 => EdDSA,
            100..=110 => Private(u),
            u => Unknown(u),
        }
    }
}

impl From<PublicKeyAlgorithm> for u8 {
    fn from(p: PublicKeyAlgorithm) -> u8 {
        use crate::PublicKeyAlgorithm::*;
        #[allow(deprecated)]
        match p {
            RSAEncryptSign => 1,
            RSAEncrypt => 2,
            RSASign => 3,
            ElGamalEncrypt => 16,
            DSA => 17,
            ECDH => 18,
            ECDSA => 19,
            ElGamalEncryptSign => 20,
            EdDSA => 22,
            Private(u) => u,
            Unknown(u) => u,
        }
    }
}

/// Formats the public key algorithm name.
///
/// There are two ways the public key algorithm name can be formatted.
/// By default the short name is used.  The alternate format uses the
/// full public key algorithm name.
///
/// # Examples
///
/// ```
/// use sequoia_openpgp as openpgp;
/// use openpgp::types::PublicKeyAlgorithm;
///
/// // default, short format
/// assert_eq!("ECDH", format!("{}", PublicKeyAlgorithm::ECDH));
///
/// // alternate, long format
/// assert_eq!("ECDH public key algorithm", format!("{:#}", PublicKeyAlgorithm::ECDH));
/// ```
impl fmt::Display for PublicKeyAlgorithm {
    #[allow(deprecated)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use crate::PublicKeyAlgorithm::*;
        if f.alternate() {
            match *self {
                RSAEncryptSign => f.write_str("RSA (Encrypt or Sign)"),
                RSAEncrypt => f.write_str("RSA Encrypt-Only"),
                RSASign => f.write_str("RSA Sign-Only"),
                ElGamalEncrypt => f.write_str("ElGamal (Encrypt-Only)"),
                DSA => f.write_str("DSA (Digital Signature Algorithm)"),
                ECDSA => f.write_str("ECDSA public key algorithm"),
                ElGamalEncryptSign => f.write_str("ElGamal (Encrypt or Sign)"),
                ECDH => f.write_str("ECDH public key algorithm"),
                EdDSA => f.write_str("EdDSA Edwards-curve Digital Signature Algorithm"),
                Private(u) =>
                    f.write_fmt(format_args!("Private/Experimental public key algorithm {}", u)),
                Unknown(u) =>
                    f.write_fmt(format_args!("Unknown public key algorithm {}", u)),
            }
        } else {
            match *self {
                RSAEncryptSign => f.write_str("RSA"),
                RSAEncrypt => f.write_str("RSA"),
                RSASign => f.write_str("RSA"),
                ElGamalEncrypt => f.write_str("ElGamal"),
                DSA => f.write_str("DSA"),
                ECDSA => f.write_str("ECDSA"),
                ElGamalEncryptSign => f.write_str("ElGamal"),
                ECDH => f.write_str("ECDH"),
                EdDSA => f.write_str("EdDSA"),
                Private(u) =>
                    f.write_fmt(format_args!("Private algo {}", u)),
                Unknown(u) =>
                    f.write_fmt(format_args!("Unknown algo {}", u)),
            }
        }
    }
}

#[cfg(test)]
impl Arbitrary for PublicKeyAlgorithm {
    fn arbitrary(g: &mut Gen) -> Self {
        u8::arbitrary(g).into()
    }
}

#[cfg(test)]
impl PublicKeyAlgorithm {
    pub(crate) fn arbitrary_for_signing(g: &mut Gen) -> Self {
        use self::PublicKeyAlgorithm::*;

        #[allow(deprecated)]
        let a = g.choose(&[RSAEncryptSign, RSASign, DSA, ECDSA, EdDSA]).unwrap();
        assert!(a.for_signing());
        *a
    }
}

/// Elliptic curves used in OpenPGP.
///
/// `PublicKeyAlgorithm` does not differentiate between elliptic
/// curves.  Instead, the curve is specified using an OID prepended to
/// the key material.  We provide this type to be able to match on the
/// curves.
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Curve {
    /// NIST curve P-256.
    NistP256,
    /// NIST curve P-384.
    NistP384,
    /// NIST curve P-521.
    NistP521,
    /// brainpoolP256r1.
    BrainpoolP256,
    /// brainpoolP512r1.
    BrainpoolP512,
    /// D.J. Bernstein's "Twisted" Edwards curve Ed25519.
    Ed25519,
    /// Elliptic curve Diffie-Hellman using D.J. Bernstein's Curve25519.
    Cv25519,
    /// Unknown curve.
    Unknown(Box<[u8]>),
}
assert_send_and_sync!(Curve);

impl Curve {
    /// Returns the length of public keys over this curve in bits.
    ///
    /// For the Kobliz curves this is the size of the underlying
    /// finite field.  For X25519 it is 256.
    ///
    /// Note: This information is useless and should not be used to
    /// gauge the security of a particular curve. This function exists
    /// only because some legacy PGP application like HKP need it.
    ///
    /// Returns `None` for unknown curves.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::Curve;
    ///
    /// assert_eq!(Curve::NistP256.bits(), Some(256));
    /// assert_eq!(Curve::NistP384.bits(), Some(384));
    /// assert_eq!(Curve::Ed25519.bits(), Some(256));
    /// assert_eq!(Curve::Unknown(Box::new([0x2B, 0x11])).bits(), None);
    /// ```
    pub fn bits(&self) -> Option<usize> {
        use self::Curve::*;

        match self {
            NistP256 => Some(256),
            NistP384 => Some(384),
            NistP521 => Some(521),
            BrainpoolP256 => Some(256),
            BrainpoolP512 => Some(512),
            Ed25519 => Some(256),
            Cv25519 => Some(256),
            Unknown(_) => None,
        }
    }

    /// Returns the curve's field size in bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::Curve;
    ///
    /// assert_eq!(Curve::NistP256.field_size()?, 32);
    /// assert_eq!(Curve::NistP384.field_size()?, 48);
    /// assert_eq!(Curve::NistP521.field_size()?, 66);
    /// assert_eq!(Curve::Ed25519.field_size()?, 32);
    /// assert!(Curve::Unknown(Box::new([0x2B, 0x11])).field_size().is_err());
    /// # Ok(()) }
    /// ```
    pub fn field_size(&self) -> Result<usize> {
        self.bits()
            .map(|bits| (bits + 7) / 8)
            .ok_or_else(|| Error::UnsupportedEllipticCurve(self.clone()).into())
    }
}

/// Formats the elliptic curve name.
///
/// There are two ways the elliptic curve name can be formatted.  By
/// default the short name is used.  The alternate format uses the
/// full curve name.
///
/// # Examples
///
/// ```
/// use sequoia_openpgp as openpgp;
/// use openpgp::types::Curve;
///
/// // default, short format
/// assert_eq!("NIST P-256", format!("{}", Curve::NistP256));
///
/// // alternate, long format
/// assert_eq!("NIST curve P-256", format!("{:#}", Curve::NistP256));
/// ```
impl fmt::Display for Curve {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Curve::*;
        if f.alternate() {
            match *self {
                NistP256 => f.write_str("NIST curve P-256"),
                NistP384 => f.write_str("NIST curve P-384"),
                NistP521 => f.write_str("NIST curve P-521"),
                BrainpoolP256 => f.write_str("brainpoolP256r1"),
                BrainpoolP512 => f.write_str("brainpoolP512r1"),
                Ed25519
                    => f.write_str("D.J. Bernstein's \"Twisted\" Edwards curve Ed25519"),
                Cv25519
                    => f.write_str("Elliptic curve Diffie-Hellman using D.J. Bernstein's Curve25519"),
                Unknown(ref oid)
                    => write!(f, "Unknown curve (OID: {:?})", oid),
            }
        } else {
            match *self {
                NistP256 => f.write_str("NIST P-256"),
                NistP384 => f.write_str("NIST P-384"),
                NistP521 => f.write_str("NIST P-521"),
                BrainpoolP256 => f.write_str("brainpoolP256r1"),
                BrainpoolP512 => f.write_str("brainpoolP512r1"),
                Ed25519
                    => f.write_str("Ed25519"),
                Cv25519
                    => f.write_str("Curve25519"),
                Unknown(ref oid)
                    => write!(f, "Unknown curve {:?}", oid),
            }
        }
    }
}

const NIST_P256_OID: &[u8] = &[0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07];
const NIST_P384_OID: &[u8] = &[0x2B, 0x81, 0x04, 0x00, 0x22];
const NIST_P521_OID: &[u8] = &[0x2B, 0x81, 0x04, 0x00, 0x23];
const BRAINPOOL_P256_OID: &[u8] =
    &[0x2B, 0x24, 0x03, 0x03, 0x02, 0x08, 0x01, 0x01, 0x07];
const BRAINPOOL_P512_OID: &[u8] =
    &[0x2B, 0x24, 0x03, 0x03, 0x02, 0x08, 0x01, 0x01, 0x0D];
const ED25519_OID: &[u8] =
    &[0x2B, 0x06, 0x01, 0x04, 0x01, 0xDA, 0x47, 0x0F, 0x01];
const CV25519_OID: &[u8] =
    &[0x2B, 0x06, 0x01, 0x04, 0x01, 0x97, 0x55, 0x01, 0x05, 0x01];

#[allow(clippy::len_without_is_empty)]
impl Curve {
    /// Parses the given OID.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::Curve;
    ///
    /// assert_eq!(Curve::from_oid(&[0x2B, 0x81, 0x04, 0x00, 0x22]), Curve::NistP384);
    /// assert_eq!(Curve::from_oid(&[0x2B, 0x11]), Curve::Unknown(Box::new([0x2B, 0x11])));
    /// ```
    pub fn from_oid(oid: &[u8]) -> Curve {
        // Match on OIDs, see section 11 of RFC6637.
        match oid {
            NIST_P256_OID => Curve::NistP256,
            NIST_P384_OID => Curve::NistP384,
            NIST_P521_OID => Curve::NistP521,
            BRAINPOOL_P256_OID => Curve::BrainpoolP256,
            BRAINPOOL_P512_OID => Curve::BrainpoolP512,
            ED25519_OID => Curve::Ed25519,
            CV25519_OID => Curve::Cv25519,
            oid => Curve::Unknown(Vec::from(oid).into_boxed_slice()),
        }
    }

    /// Returns this curve's OID.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::Curve;
    ///
    /// assert_eq!(Curve::NistP384.oid(), &[0x2B, 0x81, 0x04, 0x00, 0x22]);
    /// assert_eq!(Curve::Unknown(Box::new([0x2B, 0x11])).oid(), &[0x2B, 0x11]);
    /// ```
    pub fn oid(&self) -> &[u8] {
        match self {
            Curve::NistP256 => NIST_P256_OID,
            Curve::NistP384 => NIST_P384_OID,
            Curve::NistP521 => NIST_P521_OID,
            Curve::BrainpoolP256 => BRAINPOOL_P256_OID,
            Curve::BrainpoolP512 => BRAINPOOL_P512_OID,
            Curve::Ed25519 => ED25519_OID,
            Curve::Cv25519 => CV25519_OID,
            Curve::Unknown(ref oid) => oid,
        }
    }

    /// Returns the length of a coordinate in bits.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::Curve;
    ///
    /// assert!(if let Ok(256) = Curve::NistP256.len() { true } else { false });
    /// assert!(if let Ok(384) = Curve::NistP384.len() { true } else { false });
    /// assert!(if let Ok(256) = Curve::Ed25519.len() { true } else { false });
    /// assert!(if let Err(_) = Curve::Unknown(Box::new([0x2B, 0x11])).len() { true } else { false });
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Error::UnsupportedEllipticCurve` if the curve is not
    /// supported.
    pub fn len(&self) -> Result<usize> {
        match self {
            Curve::NistP256 => Ok(256),
            Curve::NistP384 => Ok(384),
            Curve::NistP521 => Ok(521),
            Curve::BrainpoolP256 => Ok(256),
            Curve::BrainpoolP512 => Ok(512),
            Curve::Ed25519 => Ok(256),
            Curve::Cv25519 => Ok(256),
            Curve::Unknown(_) =>
                Err(Error::UnsupportedEllipticCurve(self.clone())
                    .into()),
        }
    }

    /// Returns whether this algorithm is supported.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::Curve;
    ///
    /// assert!(Curve::Ed25519.is_supported());
    /// assert!(!Curve::Unknown(Box::new([0x2B, 0x11])).is_supported());
    /// ```
    pub fn is_supported(&self) -> bool {
        self.is_supported_by_backend()
    }
}

#[cfg(test)]
impl Arbitrary for Curve {
    fn arbitrary(g: &mut Gen) -> Self {
        match u8::arbitrary(g) % 8 {
            0 => Curve::NistP256,
            1 => Curve::NistP384,
            2 => Curve::NistP521,
            3 => Curve::BrainpoolP256,
            4 => Curve::BrainpoolP512,
            5 => Curve::Ed25519,
            6 => Curve::Cv25519,
            7 => Curve::Unknown({
                let mut k = <Vec<u8>>::arbitrary(g);
                k.truncate(255);
                k.into_boxed_slice()
            }),
            _ => unreachable!(),
        }
    }
}

/// The symmetric-key algorithms as defined in [Section 9.2 of RFC 4880].
///
///   [Section 9.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-9.2
///
/// The values can be converted into and from their corresponding values of the serialized format.
///
/// Use [`SymmetricAlgorithm::from`] to translate a numeric value to a
/// symbolic one.
///
///   [`SymmetricAlgorithm::from`]: std::convert::From
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// # Examples
///
/// Use `SymmetricAlgorithm` to set the preferred symmetric algorithms on a signature:
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// use openpgp::packet::signature::SignatureBuilder;
/// use openpgp::types::{HashAlgorithm, SymmetricAlgorithm, SignatureType};
///
/// # fn main() -> openpgp::Result<()> {
/// let mut builder = SignatureBuilder::new(SignatureType::DirectKey)
///     .set_hash_algo(HashAlgorithm::SHA512)
///     .set_preferred_symmetric_algorithms(vec![
///         SymmetricAlgorithm::AES256,
///     ])?;
/// # Ok(()) }
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum SymmetricAlgorithm {
    /// Null encryption.
    Unencrypted,
    /// IDEA block cipher.
    IDEA,
    /// 3-DES in EDE configuration.
    TripleDES,
    /// CAST5/CAST128 block cipher.
    CAST5,
    /// Schneier et.al. Blowfish block cipher.
    Blowfish,
    /// 10-round AES.
    AES128,
    /// 12-round AES.
    AES192,
    /// 14-round AES.
    AES256,
    /// Twofish block cipher.
    Twofish,
    /// 18 rounds of NESSIEs Camellia.
    Camellia128,
    /// 24 rounds of NESSIEs Camellia w/192 bit keys.
    Camellia192,
    /// 24 rounds of NESSIEs Camellia w/256 bit keys.
    Camellia256,
    /// Private algorithm identifier.
    Private(u8),
    /// Unknown algorithm identifier.
    Unknown(u8),
}
assert_send_and_sync!(SymmetricAlgorithm);

const SYMMETRIC_ALGORITHM_VARIANTS: [ SymmetricAlgorithm; 11 ] = [
    SymmetricAlgorithm::IDEA,
    SymmetricAlgorithm::TripleDES,
    SymmetricAlgorithm::CAST5,
    SymmetricAlgorithm::Blowfish,
    SymmetricAlgorithm::AES128,
    SymmetricAlgorithm::AES192,
    SymmetricAlgorithm::AES256,
    SymmetricAlgorithm::Twofish,
    SymmetricAlgorithm::Camellia128,
    SymmetricAlgorithm::Camellia192,
    SymmetricAlgorithm::Camellia256,
];

impl Default for SymmetricAlgorithm {
    fn default() -> Self {
        SymmetricAlgorithm::AES256
    }
}

impl From<u8> for SymmetricAlgorithm {
    fn from(u: u8) -> Self {
        match u {
            0 => SymmetricAlgorithm::Unencrypted,
            1 => SymmetricAlgorithm::IDEA,
            2 => SymmetricAlgorithm::TripleDES,
            3 => SymmetricAlgorithm::CAST5,
            4 => SymmetricAlgorithm::Blowfish,
            7 => SymmetricAlgorithm::AES128,
            8 => SymmetricAlgorithm::AES192,
            9 => SymmetricAlgorithm::AES256,
            10 => SymmetricAlgorithm::Twofish,
            11 => SymmetricAlgorithm::Camellia128,
            12 => SymmetricAlgorithm::Camellia192,
            13 => SymmetricAlgorithm::Camellia256,
            100..=110 => SymmetricAlgorithm::Private(u),
            u => SymmetricAlgorithm::Unknown(u),
        }
    }
}

impl From<SymmetricAlgorithm> for u8 {
    fn from(s: SymmetricAlgorithm) -> u8 {
        match s {
            SymmetricAlgorithm::Unencrypted => 0,
            SymmetricAlgorithm::IDEA => 1,
            SymmetricAlgorithm::TripleDES => 2,
            SymmetricAlgorithm::CAST5 => 3,
            SymmetricAlgorithm::Blowfish => 4,
            SymmetricAlgorithm::AES128 => 7,
            SymmetricAlgorithm::AES192 => 8,
            SymmetricAlgorithm::AES256 => 9,
            SymmetricAlgorithm::Twofish => 10,
            SymmetricAlgorithm::Camellia128 => 11,
            SymmetricAlgorithm::Camellia192 => 12,
            SymmetricAlgorithm::Camellia256 => 13,
            SymmetricAlgorithm::Private(u) => u,
            SymmetricAlgorithm::Unknown(u) => u,
        }
    }
}


/// Formats the symmetric algorithm name.
///
/// There are two ways the symmetric algorithm name can be formatted.
/// By default the short name is used.  The alternate format uses the
/// full algorithm name.
///
/// # Examples
///
/// ```
/// use sequoia_openpgp as openpgp;
/// use openpgp::types::SymmetricAlgorithm;
///
/// // default, short format
/// assert_eq!("AES-128", format!("{}", SymmetricAlgorithm::AES128));
///
/// // alternate, long format
/// assert_eq!("AES with 128-bit key", format!("{:#}", SymmetricAlgorithm::AES128));
/// ```
impl fmt::Display for SymmetricAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            match *self {
                SymmetricAlgorithm::Unencrypted =>
                    f.write_str("Unencrypted"),
                SymmetricAlgorithm::IDEA =>
                    f.write_str("IDEA"),
                SymmetricAlgorithm::TripleDES =>
                    f.write_str("TripleDES (EDE-DES, 168 bit key derived from 192))"),
                SymmetricAlgorithm::CAST5 =>
                    f.write_str("CAST5 (128 bit key, 16 rounds)"),
                SymmetricAlgorithm::Blowfish =>
                    f.write_str("Blowfish (128 bit key, 16 rounds)"),
                SymmetricAlgorithm::AES128 =>
                    f.write_str("AES with 128-bit key"),
                SymmetricAlgorithm::AES192 =>
                    f.write_str("AES with 192-bit key"),
                SymmetricAlgorithm::AES256 =>
                    f.write_str("AES with 256-bit key"),
                SymmetricAlgorithm::Twofish =>
                    f.write_str("Twofish with 256-bit key"),
                SymmetricAlgorithm::Camellia128 =>
                    f.write_str("Camellia with 128-bit key"),
                SymmetricAlgorithm::Camellia192 =>
                    f.write_str("Camellia with 192-bit key"),
                SymmetricAlgorithm::Camellia256 =>
                    f.write_str("Camellia with 256-bit key"),
                SymmetricAlgorithm::Private(u) =>
                    f.write_fmt(format_args!("Private/Experimental symmetric key algorithm {}", u)),
                SymmetricAlgorithm::Unknown(u) =>
                    f.write_fmt(format_args!("Unknown symmetric key algorithm {}", u)),
            }
        } else {
            match *self {
                SymmetricAlgorithm::Unencrypted =>
                    f.write_str("Unencrypted"),
                SymmetricAlgorithm::IDEA =>
                    f.write_str("IDEA"),
                SymmetricAlgorithm::TripleDES =>
                    f.write_str("3DES"),
                SymmetricAlgorithm::CAST5 =>
                    f.write_str("CAST5"),
                SymmetricAlgorithm::Blowfish =>
                    f.write_str("Blowfish"),
                SymmetricAlgorithm::AES128 =>
                    f.write_str("AES-128"),
                SymmetricAlgorithm::AES192 =>
                    f.write_str("AES-192"),
                SymmetricAlgorithm::AES256 =>
                    f.write_str("AES-256"),
                SymmetricAlgorithm::Twofish =>
                    f.write_str("Twofish"),
                SymmetricAlgorithm::Camellia128 =>
                    f.write_str("Camellia-128"),
                SymmetricAlgorithm::Camellia192 =>
                    f.write_str("Camellia-192"),
                SymmetricAlgorithm::Camellia256 =>
                    f.write_str("Camellia-256"),
                SymmetricAlgorithm::Private(u) =>
                    f.write_fmt(format_args!("Private symmetric key algo {}", u)),
                SymmetricAlgorithm::Unknown(u) =>
                    f.write_fmt(format_args!("Unknown symmetric key algo {}", u)),
            }
        }
    }
}

#[cfg(test)]
impl Arbitrary for SymmetricAlgorithm {
    fn arbitrary(g: &mut Gen) -> Self {
        u8::arbitrary(g).into()
    }
}

impl SymmetricAlgorithm {
    /// Returns an iterator over all valid variants.
    ///
    /// Returns an iterator over all known variants.  This does not
    /// include the [`SymmetricAlgorithm::Unencrypted`],
    /// [`SymmetricAlgorithm::Private`], or
    /// [`SymmetricAlgorithm::Unknown`] variants.
    pub fn variants() -> impl Iterator<Item=Self> {
        SYMMETRIC_ALGORITHM_VARIANTS.iter().cloned()
    }
}

/// The AEAD algorithms as defined in [Section 9.6 of RFC 4880bis].
///
///   [Section 9.6 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-05#section-9.6
///
/// The values can be converted into and from their corresponding values of the serialized format.
///
/// Use [`AEADAlgorithm::from`] to translate a numeric value to a
/// symbolic one.
///
///   [`AEADAlgorithm::from`]: std::convert::From
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// This feature is [experimental](super#experimental-features).
///
/// # Examples
///
/// Use `AEADAlgorithm` to set the preferred AEAD algorithms on a signature:
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// use openpgp::packet::signature::SignatureBuilder;
/// use openpgp::types::{Features, HashAlgorithm, AEADAlgorithm, SignatureType};
///
/// # fn main() -> openpgp::Result<()> {
/// let features = Features::empty().set_aead();
/// let mut builder = SignatureBuilder::new(SignatureType::DirectKey)
///     .set_features(features)?
///     .set_preferred_aead_algorithms(vec![
///         AEADAlgorithm::EAX,
///     ])?;
/// # Ok(()) }
#[non_exhaustive]
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum AEADAlgorithm {
    /// EAX mode.
    EAX,
    /// OCB mode.
    OCB,
    /// Private algorithm identifier.
    Private(u8),
    /// Unknown algorithm identifier.
    Unknown(u8),
}
assert_send_and_sync!(AEADAlgorithm);

const AEAD_ALGORITHM_VARIANTS: [AEADAlgorithm; 2] = [
    AEADAlgorithm::EAX,
    AEADAlgorithm::OCB,
];

impl AEADAlgorithm {
    /// Returns whether this algorithm is supported.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::AEADAlgorithm;
    ///
    /// assert!(! AEADAlgorithm::Private(100).is_supported());
    /// ```
    pub fn is_supported(&self) -> bool {
        self.is_supported_by_backend()
    }

    /// Returns an iterator over all valid variants.
    ///
    /// Returns an iterator over all known variants.  This does not
    /// include the [`AEADAlgorithm::Private`], or
    /// [`AEADAlgorithm::Unknown`] variants.
    pub fn variants() -> impl Iterator<Item=Self> {
        AEAD_ALGORITHM_VARIANTS.iter().cloned()
    }
}

impl From<u8> for AEADAlgorithm {
    fn from(u: u8) -> Self {
        match u {
            1 => AEADAlgorithm::EAX,
            2 => AEADAlgorithm::OCB,
            100..=110 => AEADAlgorithm::Private(u),
            u => AEADAlgorithm::Unknown(u),
        }
    }
}

impl From<AEADAlgorithm> for u8 {
    fn from(s: AEADAlgorithm) -> u8 {
        match s {
            AEADAlgorithm::EAX => 1,
            AEADAlgorithm::OCB => 2,
            AEADAlgorithm::Private(u) => u,
            AEADAlgorithm::Unknown(u) => u,
        }
    }
}

/// Formats the AEAD algorithm name.
///
/// There are two ways the AEAD algorithm name can be formatted.  By
/// default the short name is used.  The alternate format uses the
/// full algorithm name.
///
/// # Examples
///
/// ```
/// use sequoia_openpgp as openpgp;
/// use openpgp::types::AEADAlgorithm;
///
/// // default, short format
/// assert_eq!("EAX", format!("{}", AEADAlgorithm::EAX));
///
/// // alternate, long format
/// assert_eq!("EAX mode", format!("{:#}", AEADAlgorithm::EAX));
/// ```
impl fmt::Display for AEADAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            match *self {
                AEADAlgorithm::EAX =>
                    f.write_str("EAX mode"),
                AEADAlgorithm::OCB =>
                    f.write_str("OCB mode"),
                AEADAlgorithm::Private(u) =>
                    f.write_fmt(format_args!("Private/Experimental AEAD algorithm {}", u)),
                AEADAlgorithm::Unknown(u) =>
                    f.write_fmt(format_args!("Unknown AEAD algorithm {}", u)),
            }
        } else {
            match *self {
                AEADAlgorithm::EAX =>
                    f.write_str("EAX"),
                AEADAlgorithm::OCB =>
                    f.write_str("OCB"),
                AEADAlgorithm::Private(u) =>
                    f.write_fmt(format_args!("Private AEAD algo {}", u)),
                AEADAlgorithm::Unknown(u) =>
                    f.write_fmt(format_args!("Unknown AEAD algo {}", u)),
            }
        }
    }
}

#[cfg(test)]
impl Arbitrary for AEADAlgorithm {
    fn arbitrary(g: &mut Gen) -> Self {
        u8::arbitrary(g).into()
    }
}

/// The OpenPGP compression algorithms as defined in [Section 9.3 of RFC 4880].
///
///   [Section 9.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-9.3
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// # Examples
///
/// Use `CompressionAlgorithm` to set the preferred compressions algorithms on
/// a signature:
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// use openpgp::packet::signature::SignatureBuilder;
/// use openpgp::types::{HashAlgorithm, CompressionAlgorithm, SignatureType};
///
/// # fn main() -> openpgp::Result<()> {
/// let mut builder = SignatureBuilder::new(SignatureType::DirectKey)
///     .set_hash_algo(HashAlgorithm::SHA512)
///     .set_preferred_compression_algorithms(vec![
///         CompressionAlgorithm::Zlib,
///         CompressionAlgorithm::BZip2,
///     ])?;
/// # Ok(()) }
#[non_exhaustive]
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum CompressionAlgorithm {
    /// Null compression.
    Uncompressed,
    /// DEFLATE Compressed Data.
    ///
    /// See [RFC 1951] for details.  [Section 9.3 of RFC 4880]
    /// recommends that this algorithm should be implemented.
    ///
    /// [RFC 1951]: https://tools.ietf.org/html/rfc1951
    /// [Section 9.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-9.3
    Zip,
    /// ZLIB Compressed Data.
    ///
    /// See [RFC 1950] for details.
    ///
    /// [RFC 1950]: https://tools.ietf.org/html/rfc1950
    Zlib,
    /// bzip2
    BZip2,
    /// Private compression algorithm identifier.
    Private(u8),
    /// Unknown compression algorithm identifier.
    Unknown(u8),
}
assert_send_and_sync!(CompressionAlgorithm);

const COMPRESSION_ALGORITHM_VARIANTS: [CompressionAlgorithm; 4] = [
    CompressionAlgorithm::Uncompressed,
    CompressionAlgorithm::Zip,
    CompressionAlgorithm::Zlib,
    CompressionAlgorithm::BZip2,
];

impl Default for CompressionAlgorithm {
    fn default() -> Self {
        use self::CompressionAlgorithm::*;
        #[cfg(feature = "compression-deflate")]
        { Zip }
        #[cfg(all(feature = "compression-bzip2",
                  not(feature = "compression-deflate")))]
        { BZip2 }
        #[cfg(all(not(feature = "compression-bzip2"),
                  not(feature = "compression-deflate")))]
        { Uncompressed }
    }
}

impl CompressionAlgorithm {
    /// Returns whether this algorithm is supported.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::CompressionAlgorithm;
    ///
    /// assert!(CompressionAlgorithm::Uncompressed.is_supported());
    ///
    /// assert!(!CompressionAlgorithm::Private(101).is_supported());
    /// ```
    pub fn is_supported(&self) -> bool {
        use self::CompressionAlgorithm::*;
        match &self {
            Uncompressed => true,
            #[cfg(feature = "compression-deflate")]
            Zip | Zlib => true,
            #[cfg(feature = "compression-bzip2")]
            BZip2 => true,
            _ => false,
        }
    }

    /// Returns an iterator over all valid variants.
    ///
    /// Returns an iterator over all known variants.  This does not
    /// include the [`CompressionAlgorithm::Private`], or
    /// [`CompressionAlgorithm::Unknown`] variants.
    pub fn variants() -> impl Iterator<Item=Self> {
        COMPRESSION_ALGORITHM_VARIANTS.iter().cloned()
    }
}

impl From<u8> for CompressionAlgorithm {
    fn from(u: u8) -> Self {
        match u {
            0 => CompressionAlgorithm::Uncompressed,
            1 => CompressionAlgorithm::Zip,
            2 => CompressionAlgorithm::Zlib,
            3 => CompressionAlgorithm::BZip2,
            100..=110 => CompressionAlgorithm::Private(u),
            u => CompressionAlgorithm::Unknown(u),
        }
    }
}

impl From<CompressionAlgorithm> for u8 {
    fn from(c: CompressionAlgorithm) -> u8 {
        match c {
            CompressionAlgorithm::Uncompressed => 0,
            CompressionAlgorithm::Zip => 1,
            CompressionAlgorithm::Zlib => 2,
            CompressionAlgorithm::BZip2 => 3,
            CompressionAlgorithm::Private(u) => u,
            CompressionAlgorithm::Unknown(u) => u,
        }
    }
}

impl fmt::Display for CompressionAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CompressionAlgorithm::Uncompressed => f.write_str("Uncompressed"),
            CompressionAlgorithm::Zip => f.write_str("ZIP"),
            CompressionAlgorithm::Zlib => f.write_str("ZLIB"),
            CompressionAlgorithm::BZip2 => f.write_str("BZip2"),
            CompressionAlgorithm::Private(u) =>
                f.write_fmt(format_args!("Private/Experimental compression algorithm {}", u)),
            CompressionAlgorithm::Unknown(u) =>
                f.write_fmt(format_args!("Unknown comppression algorithm {}", u)),
        }
    }
}

#[cfg(test)]
impl Arbitrary for CompressionAlgorithm {
    fn arbitrary(g: &mut Gen) -> Self {
        u8::arbitrary(g).into()
    }
}

/// The OpenPGP hash algorithms as defined in [Section 9.4 of RFC 4880].
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// # Examples
///
/// Use `HashAlgorithm` to set the preferred hash algorithms on a signature:
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// use openpgp::packet::signature::SignatureBuilder;
/// use openpgp::types::{HashAlgorithm, SignatureType};
///
/// # fn main() -> openpgp::Result<()> {
/// let mut builder = SignatureBuilder::new(SignatureType::DirectKey)
///     .set_hash_algo(HashAlgorithm::SHA512);
/// # Ok(()) }
/// ```
///
/// [Section 9.4 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-9.4
#[non_exhaustive]
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum HashAlgorithm {
    /// Rivest et.al. message digest 5.
    MD5,
    /// NIST Secure Hash Algorithm (deprecated)
    SHA1,
    /// RIPEMD-160
    RipeMD,
    /// 256-bit version of SHA2
    SHA256,
    /// 384-bit version of SHA2
    SHA384,
    /// 512-bit version of SHA2
    SHA512,
    /// 224-bit version of SHA2
    SHA224,
    /// Private hash algorithm identifier.
    Private(u8),
    /// Unknown hash algorithm identifier.
    Unknown(u8),
}
assert_send_and_sync!(HashAlgorithm);

const HASH_ALGORITHM_VARIANTS: [HashAlgorithm; 7] = [
    HashAlgorithm::MD5,
    HashAlgorithm::SHA1,
    HashAlgorithm::RipeMD,
    HashAlgorithm::SHA256,
    HashAlgorithm::SHA384,
    HashAlgorithm::SHA512,
    HashAlgorithm::SHA224,
];

impl Default for HashAlgorithm {
    fn default() -> Self {
        // SHA512 is almost twice as fast as SHA256 on 64-bit
        // architectures because it operates on 64-bit words.
        HashAlgorithm::SHA512
    }
}

impl From<u8> for HashAlgorithm {
    fn from(u: u8) -> Self {
        match u {
            1 => HashAlgorithm::MD5,
            2 => HashAlgorithm::SHA1,
            3 => HashAlgorithm::RipeMD,
            8 => HashAlgorithm::SHA256,
            9 => HashAlgorithm::SHA384,
            10 => HashAlgorithm::SHA512,
            11 => HashAlgorithm::SHA224,
            100..=110 => HashAlgorithm::Private(u),
            u => HashAlgorithm::Unknown(u),
        }
    }
}

impl From<HashAlgorithm> for u8 {
    fn from(h: HashAlgorithm) -> u8 {
        match h {
            HashAlgorithm::MD5 => 1,
            HashAlgorithm::SHA1 => 2,
            HashAlgorithm::RipeMD => 3,
            HashAlgorithm::SHA256 => 8,
            HashAlgorithm::SHA384 => 9,
            HashAlgorithm::SHA512 => 10,
            HashAlgorithm::SHA224 => 11,
            HashAlgorithm::Private(u) => u,
            HashAlgorithm::Unknown(u) => u,
        }
    }
}

impl FromStr for HashAlgorithm {
    type Err = ();

    fn from_str(s: &str) -> result::Result<Self, ()> {
        if s.eq_ignore_ascii_case("MD5") {
            Ok(HashAlgorithm::MD5)
        } else if s.eq_ignore_ascii_case("SHA1") {
            Ok(HashAlgorithm::SHA1)
        } else if s.eq_ignore_ascii_case("RipeMD160") {
            Ok(HashAlgorithm::RipeMD)
        } else if s.eq_ignore_ascii_case("SHA256") {
            Ok(HashAlgorithm::SHA256)
        } else if s.eq_ignore_ascii_case("SHA384") {
            Ok(HashAlgorithm::SHA384)
        } else if s.eq_ignore_ascii_case("SHA512") {
            Ok(HashAlgorithm::SHA512)
        } else if s.eq_ignore_ascii_case("SHA224") {
            Ok(HashAlgorithm::SHA224)
        } else {
            Err(())
        }
    }
}

impl fmt::Display for HashAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            HashAlgorithm::MD5 => f.write_str("MD5"),
            HashAlgorithm::SHA1 => f.write_str("SHA1"),
            HashAlgorithm::RipeMD => f.write_str("RipeMD160"),
            HashAlgorithm::SHA256 => f.write_str("SHA256"),
            HashAlgorithm::SHA384 => f.write_str("SHA384"),
            HashAlgorithm::SHA512 => f.write_str("SHA512"),
            HashAlgorithm::SHA224 => f.write_str("SHA224"),
            HashAlgorithm::Private(u) =>
                f.write_fmt(format_args!("Private/Experimental hash algorithm {}", u)),
            HashAlgorithm::Unknown(u) =>
                f.write_fmt(format_args!("Unknown hash algorithm {}", u)),
        }
    }
}

impl HashAlgorithm {
    /// Returns the text name of this algorithm.
    ///
    /// [Section 9.4 of RFC 4880] defines a textual representation of
    /// hash algorithms.  This is used in cleartext signed messages
    /// (see [Section 7 of RFC 4880]).
    ///
    ///   [Section 9.4 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-9.4
    ///   [Section 7 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-7
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::types::HashAlgorithm;
    /// # fn main() -> openpgp::Result<()> {
    /// assert_eq!(HashAlgorithm::RipeMD.text_name()?, "RIPEMD160");
    /// # Ok(()) }
    /// ```
    pub fn text_name(&self) -> Result<&str> {
        match self {
            HashAlgorithm::MD5 =>    Ok("MD5"),
            HashAlgorithm::SHA1 =>   Ok("SHA1"),
            HashAlgorithm::RipeMD => Ok("RIPEMD160"),
            HashAlgorithm::SHA256 => Ok("SHA256"),
            HashAlgorithm::SHA384 => Ok("SHA384"),
            HashAlgorithm::SHA512 => Ok("SHA512"),
            HashAlgorithm::SHA224 => Ok("SHA224"),
            HashAlgorithm::Private(_) =>
                Err(Error::UnsupportedHashAlgorithm(*self).into()),
            HashAlgorithm::Unknown(_) =>
                Err(Error::UnsupportedHashAlgorithm(*self).into()),
        }
    }

    /// Returns an iterator over all valid variants.
    ///
    /// Returns an iterator over all known variants.  This does not
    /// include the [`HashAlgorithm::Private`], or
    /// [`HashAlgorithm::Unknown`] variants.
    pub fn variants() -> impl Iterator<Item=Self> {
        HASH_ALGORITHM_VARIANTS.iter().cloned()
    }
}

#[cfg(test)]
impl Arbitrary for HashAlgorithm {
    fn arbitrary(g: &mut Gen) -> Self {
        u8::arbitrary(g).into()
    }
}

/// Signature type as defined in [Section 5.2.1 of RFC 4880].
///
///   [Section 5.2.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.1
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// # Examples
///
/// Use `SignatureType` to create a timestamp signature:
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// use std::time::SystemTime;
/// use openpgp::packet::signature::SignatureBuilder;
/// use openpgp::types::SignatureType;
///
/// # fn main() -> openpgp::Result<()> {
/// let mut builder = SignatureBuilder::new(SignatureType::Timestamp)
///     .set_signature_creation_time(SystemTime::now())?;
/// # Ok(()) }
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum SignatureType {
    /// Signature over a binary document.
    Binary,
    /// Signature over a canonical text document.
    Text,
    /// Standalone signature.
    Standalone,

    /// Generic certification of a User ID and Public-Key packet.
    GenericCertification,
    /// Persona certification of a User ID and Public-Key packet.
    PersonaCertification,
    /// Casual certification of a User ID and Public-Key packet.
    CasualCertification,
    /// Positive certification of a User ID and Public-Key packet.
    PositiveCertification,

    /// Attestation Key Signature (proposed).
    ///
    /// Allows the certificate owner to attest to third party
    /// certifications. See [Section 5.2.3.30 of RFC 4880bis] for
    /// details.
    ///
    ///   [Section 5.2.3.30 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-10.html#section-5.2.3.30
    AttestationKey,

    /// Subkey Binding Signature
    SubkeyBinding,
    /// Primary Key Binding Signature
    PrimaryKeyBinding,
    /// Signature directly on a key
    DirectKey,

    /// Key revocation signature
    KeyRevocation,
    /// Subkey revocation signature
    SubkeyRevocation,
    /// Certification revocation signature
    CertificationRevocation,

    /// Timestamp signature.
    Timestamp,
    /// Third-Party Confirmation signature.
    Confirmation,

    /// Catchall.
    Unknown(u8),
}
assert_send_and_sync!(SignatureType);

const SIGNATURE_TYPE_VARIANTS: [SignatureType; 16] = [
    SignatureType::Binary,
    SignatureType::Text,
    SignatureType::Standalone,
    SignatureType::GenericCertification,
    SignatureType::PersonaCertification,
    SignatureType::CasualCertification,
    SignatureType::PositiveCertification,
    SignatureType::AttestationKey,
    SignatureType::SubkeyBinding,
    SignatureType::PrimaryKeyBinding,
    SignatureType::DirectKey,
    SignatureType::KeyRevocation,
    SignatureType::SubkeyRevocation,
    SignatureType::CertificationRevocation,
    SignatureType::Timestamp,
    SignatureType::Confirmation,
];

impl From<u8> for SignatureType {
    fn from(u: u8) -> Self {
        match u {
            0x00 => SignatureType::Binary,
            0x01 => SignatureType::Text,
            0x02 => SignatureType::Standalone,
            0x10 => SignatureType::GenericCertification,
            0x11 => SignatureType::PersonaCertification,
            0x12 => SignatureType::CasualCertification,
            0x13 => SignatureType::PositiveCertification,
            0x16 => SignatureType::AttestationKey,
            0x18 => SignatureType::SubkeyBinding,
            0x19 => SignatureType::PrimaryKeyBinding,
            0x1f => SignatureType::DirectKey,
            0x20 => SignatureType::KeyRevocation,
            0x28 => SignatureType::SubkeyRevocation,
            0x30 => SignatureType::CertificationRevocation,
            0x40 => SignatureType::Timestamp,
            0x50 => SignatureType::Confirmation,
            _ => SignatureType::Unknown(u),
        }
    }
}

impl From<SignatureType> for u8 {
    fn from(t: SignatureType) -> Self {
        match t {
            SignatureType::Binary => 0x00,
            SignatureType::Text => 0x01,
            SignatureType::Standalone => 0x02,
            SignatureType::GenericCertification => 0x10,
            SignatureType::PersonaCertification => 0x11,
            SignatureType::CasualCertification => 0x12,
            SignatureType::PositiveCertification => 0x13,
            SignatureType::AttestationKey => 0x16,
            SignatureType::SubkeyBinding => 0x18,
            SignatureType::PrimaryKeyBinding => 0x19,
            SignatureType::DirectKey => 0x1f,
            SignatureType::KeyRevocation => 0x20,
            SignatureType::SubkeyRevocation => 0x28,
            SignatureType::CertificationRevocation => 0x30,
            SignatureType::Timestamp => 0x40,
            SignatureType::Confirmation => 0x50,
            SignatureType::Unknown(u) => u,
        }
    }
}

impl fmt::Display for SignatureType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SignatureType::Binary =>
                f.write_str("Binary"),
            SignatureType::Text =>
                f.write_str("Text"),
            SignatureType::Standalone =>
                f.write_str("Standalone"),
            SignatureType::GenericCertification =>
                f.write_str("GenericCertification"),
            SignatureType::PersonaCertification =>
                f.write_str("PersonaCertification"),
            SignatureType::CasualCertification =>
                f.write_str("CasualCertification"),
            SignatureType::PositiveCertification =>
                f.write_str("PositiveCertification"),
            SignatureType::AttestationKey =>
                f.write_str("AttestationKey"),
            SignatureType::SubkeyBinding =>
                f.write_str("SubkeyBinding"),
            SignatureType::PrimaryKeyBinding =>
                f.write_str("PrimaryKeyBinding"),
            SignatureType::DirectKey =>
                f.write_str("DirectKey"),
            SignatureType::KeyRevocation =>
                f.write_str("KeyRevocation"),
            SignatureType::SubkeyRevocation =>
                f.write_str("SubkeyRevocation"),
            SignatureType::CertificationRevocation =>
                f.write_str("CertificationRevocation"),
            SignatureType::Timestamp =>
                f.write_str("Timestamp"),
            SignatureType::Confirmation =>
                f.write_str("Confirmation"),
            SignatureType::Unknown(u) =>
                f.write_fmt(format_args!("Unknown signature type 0x{:x}", u)),
        }
    }
}

#[cfg(test)]
impl Arbitrary for SignatureType {
    fn arbitrary(g: &mut Gen) -> Self {
        u8::arbitrary(g).into()
    }
}

impl SignatureType {
    /// Returns an iterator over all valid variants.
    ///
    /// Returns an iterator over all known variants.  This does not
    /// include the [`SignatureType::Unknown`] variants.
    pub fn variants() -> impl Iterator<Item=Self> {
        SIGNATURE_TYPE_VARIANTS.iter().cloned()
    }
}

/// Describes the reason for a revocation.
///
/// See the description of revocation subpackets [Section 5.2.3.23 of RFC 4880].
///
///   [Section 5.2.3.23 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.23
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// # Examples
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
/// use openpgp::policy::StandardPolicy;
/// use openpgp::types::{RevocationStatus, ReasonForRevocation, SignatureType};
///
/// # fn main() -> openpgp::Result<()> {
/// let p = &StandardPolicy::new();
///
/// // A certificate with a User ID.
/// let (cert, _) = CertBuilder::new()
///     .add_userid("Alice <alice@example.org>")
///     .generate()?;
///
/// let mut keypair = cert.primary_key().key().clone()
///     .parts_into_secret()?.into_keypair()?;
/// let ca = cert.userids().nth(0).unwrap();
///
/// // Generate the revocation for the first and only UserID.
/// let revocation =
///     UserIDRevocationBuilder::new()
///     .set_reason_for_revocation(
///         ReasonForRevocation::UIDRetired,
///         b"Left example.org.")?
///     .build(&mut keypair, &cert, ca.userid(), None)?;
/// assert_eq!(revocation.typ(), SignatureType::CertificationRevocation);
///
/// // Now merge the revocation signature into the Cert.
/// let cert = cert.insert_packets(revocation.clone())?;
///
/// // Check that it is revoked.
/// let ca = cert.userids().nth(0).unwrap();
/// let status = ca.with_policy(p, None)?.revocation_status();
/// if let RevocationStatus::Revoked(revs) = status {
///     assert_eq!(revs.len(), 1);
///     let rev = revs[0];
///
///     assert_eq!(rev.typ(), SignatureType::CertificationRevocation);
///     assert_eq!(rev.reason_for_revocation(),
///                Some((ReasonForRevocation::UIDRetired,
///                      "Left example.org.".as_bytes())));
///    // User ID has been revoked.
/// }
/// # else { unreachable!(); }
/// # Ok(()) }
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum ReasonForRevocation {
    /// No reason specified (key revocations or cert revocations)
    Unspecified,

    /// Key is superseded (key revocations)
    KeySuperseded,

    /// Key material has been compromised (key revocations)
    KeyCompromised,

    /// Key is retired and no longer used (key revocations)
    KeyRetired,

    /// User ID information is no longer valid (cert revocations)
    UIDRetired,

    /// Private reason identifier.
    Private(u8),

    /// Unknown reason identifier.
    Unknown(u8),
}
assert_send_and_sync!(ReasonForRevocation);

const REASON_FOR_REVOCATION_VARIANTS: [ReasonForRevocation; 5] = [
    ReasonForRevocation::Unspecified,
    ReasonForRevocation::KeySuperseded,
    ReasonForRevocation::KeyCompromised,
    ReasonForRevocation::KeyRetired,
    ReasonForRevocation::UIDRetired,
];

impl From<u8> for ReasonForRevocation {
    fn from(u: u8) -> Self {
        use self::ReasonForRevocation::*;
        match u {
            0 => Unspecified,
            1 => KeySuperseded,
            2 => KeyCompromised,
            3 => KeyRetired,
            32 => UIDRetired,
            100..=110 => Private(u),
            u => Unknown(u),
        }
    }
}

impl From<ReasonForRevocation> for u8 {
    fn from(r: ReasonForRevocation) -> u8 {
        use self::ReasonForRevocation::*;
        match r {
            Unspecified => 0,
            KeySuperseded => 1,
            KeyCompromised => 2,
            KeyRetired => 3,
            UIDRetired => 32,
            Private(u) => u,
            Unknown(u) => u,
        }
    }
}

impl fmt::Display for ReasonForRevocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ReasonForRevocation::*;
        match *self {
            Unspecified =>
                f.write_str("No reason specified"),
            KeySuperseded =>
                f.write_str("Key is superseded"),
            KeyCompromised =>
                f.write_str("Key material has been compromised"),
            KeyRetired =>
                f.write_str("Key is retired and no longer used"),
            UIDRetired =>
                f.write_str("User ID information is no longer valid"),
            Private(u) =>
                f.write_fmt(format_args!(
                    "Private/Experimental revocation reason {}", u)),
            Unknown(u) =>
                f.write_fmt(format_args!(
                    "Unknown revocation reason {}", u)),
        }
    }
}

#[cfg(test)]
impl Arbitrary for ReasonForRevocation {
    fn arbitrary(g: &mut Gen) -> Self {
        u8::arbitrary(g).into()
    }
}

/// Describes whether a `ReasonForRevocation` should be consider hard
/// or soft.
///
/// A hard revocation is a revocation that indicates that the key was
/// somehow compromised, and the provence of *all* artifacts should be
/// called into question.
///
/// A soft revocation is a revocation that indicates that the key
/// should be considered invalid *after* the revocation signature's
/// creation time.  `KeySuperseded`, `KeyRetired`, and `UIDRetired`
/// are considered soft revocations.
///
/// # Examples
///
/// A certificate is considered to be revoked when a hard revocation is present
/// even if it is not live at the specified time.
///
/// Here, a certificate is generated at `t0` and then revoked later at `t2`.
/// At `t1` (`t0` < `t1` < `t2`) depending on the revocation type it will be
/// either considered revoked (hard revocation) or not revoked (soft revocation):
///
/// ```rust
/// # use sequoia_openpgp as openpgp;
/// use std::time::{Duration, SystemTime};
/// use openpgp::cert::prelude::*;
/// use openpgp::types::{RevocationStatus, ReasonForRevocation};
/// use openpgp::policy::StandardPolicy;
///
/// # fn main() -> openpgp::Result<()> {
/// let p = &StandardPolicy::new();
///
/// let t0 = SystemTime::now();
/// let (cert, _) =
///     CertBuilder::general_purpose(None, Some("alice@example.org"))
///     .set_creation_time(t0)
///     .generate()?;
///
/// let t2 = t0 + Duration::from_secs(3600);
///
/// let mut signer = cert.primary_key().key().clone()
///     .parts_into_secret()?.into_keypair()?;
///
/// // Create a hard revocation (KeyCompromised):
/// let sig = CertRevocationBuilder::new()
///     .set_reason_for_revocation(ReasonForRevocation::KeyCompromised,
///                                b"The butler did it :/")?
///     .set_signature_creation_time(t2)?
///     .build(&mut signer, &cert, None)?;
///
/// let t1 = t0 + Duration::from_secs(1200);
/// let cert1 = cert.clone().insert_packets(sig.clone())?;
/// assert_eq!(cert1.revocation_status(p, Some(t1)),
///            RevocationStatus::Revoked(vec![&sig.into()]));
///
/// // Create a soft revocation (KeySuperseded):
/// let sig = CertRevocationBuilder::new()
///     .set_reason_for_revocation(ReasonForRevocation::KeySuperseded,
///                                b"Migrated to key XYZ")?
///     .set_signature_creation_time(t2)?
///     .build(&mut signer, &cert, None)?;
///
/// let t1 = t0 + Duration::from_secs(1200);
/// let cert2 = cert.clone().insert_packets(sig.clone())?;
/// assert_eq!(cert2.revocation_status(p, Some(t1)),
///            RevocationStatus::NotAsFarAsWeKnow);
/// #     Ok(())
/// # }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevocationType {
    /// A hard revocation.
    ///
    /// Artifacts stemming from the revoked object should not be
    /// trusted.
    Hard,
    /// A soft revocation.
    ///
    /// Artifacts stemming from the revoked object *after* the
    /// revocation time should not be trusted.  Earlier objects should
    /// be considered okay.
    ///
    /// Only `KeySuperseded`, `KeyRetired`, and `UIDRetired` are
    /// considered soft revocations.  All other reasons for
    /// revocations including unknown reasons are considered hard
    /// revocations.
    Soft,
}
assert_send_and_sync!(RevocationType);

impl ReasonForRevocation {
    /// Returns the revocation's `RevocationType`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::{ReasonForRevocation, RevocationType};
    ///
    /// assert_eq!(ReasonForRevocation::KeyCompromised.revocation_type(), RevocationType::Hard);
    /// assert_eq!(ReasonForRevocation::Private(101).revocation_type(), RevocationType::Hard);
    ///
    /// assert_eq!(ReasonForRevocation::KeyRetired.revocation_type(), RevocationType::Soft);
    /// ```
    pub fn revocation_type(&self) -> RevocationType {
        match self {
            ReasonForRevocation::Unspecified => RevocationType::Hard,
            ReasonForRevocation::KeySuperseded => RevocationType::Soft,
            ReasonForRevocation::KeyCompromised => RevocationType::Hard,
            ReasonForRevocation::KeyRetired => RevocationType::Soft,
            ReasonForRevocation::UIDRetired => RevocationType::Soft,
            ReasonForRevocation::Private(_) => RevocationType::Hard,
            ReasonForRevocation::Unknown(_) => RevocationType::Hard,
        }
    }

    /// Returns an iterator over all valid variants.
    ///
    /// Returns an iterator over all known variants.  This does not
    /// include the [`ReasonForRevocation::Private`] or
    /// [`ReasonForRevocation::Unknown`] variants.
    pub fn variants() -> impl Iterator<Item=Self> {
        REASON_FOR_REVOCATION_VARIANTS.iter().cloned()
    }
}

/// Describes the format of the body of a literal data packet.
///
/// See the description of literal data packets [Section 5.9 of RFC 4880].
///
///   [Section 5.9 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.9
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// # Examples
///
/// Construct a new [`Message`] containing one text literal packet:
///
/// [`Message`]: crate::Message
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// use std::convert::TryFrom;
/// use openpgp::packet::prelude::*;
/// use openpgp::types::DataFormat;
/// use openpgp::message::Message;
///
/// let mut packets = Vec::new();
/// let mut lit = Literal::new(DataFormat::Text);
/// lit.set_body(b"data".to_vec());
/// packets.push(lit.into());
///
/// let message = Message::try_from(packets);
/// assert!(message.is_ok(), "{:?}", message);
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum DataFormat {
    /// Binary data.
    ///
    /// This is a hint that the content is probably binary data.
    Binary,

    /// Text data.
    ///
    /// This is a hint that the content is probably text; the encoding
    /// is not specified.
    Text,

    /// Text data, probably valid UTF-8.
    ///
    /// This is a hint that the content is probably UTF-8 encoded.
    Unicode,

    /// MIME message.
    ///
    /// This is defined in [Section 5.10 of RFC4880bis].
    ///
    ///   [Section 5.10 of RFC4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-05#section-5.10
    #[deprecated(since = "1.10.0", note = "Do not use as semantics are unclear")]
    MIME,

    /// Unknown format specifier.
    Unknown(char),
}
assert_send_and_sync!(DataFormat);

#[allow(deprecated)]
const DATA_FORMAT_VARIANTS: [DataFormat; 4] = [
    DataFormat::Binary,
    DataFormat::Text,
    DataFormat::Unicode,
    DataFormat::MIME,
];

impl Default for DataFormat {
    fn default() -> Self {
        DataFormat::Binary
    }
}

impl From<u8> for DataFormat {
    fn from(u: u8) -> Self {
        (u as char).into()
    }
}

impl From<char> for DataFormat {
    fn from(c: char) -> Self {
        use self::DataFormat::*;
        match c {
            'b' => Binary,
            't' => Text,
            'u' => Unicode,
            #[allow(deprecated)]
            'm' => MIME,
            c => Unknown(c),
        }
    }
}

impl From<DataFormat> for u8 {
    fn from(f: DataFormat) -> u8 {
        char::from(f) as u8
    }
}

impl From<DataFormat> for char {
    fn from(f: DataFormat) -> char {
        use self::DataFormat::*;
        match f {
            Binary => 'b',
            Text => 't',
            Unicode => 'u',
            #[allow(deprecated)]
            MIME => 'm',
            Unknown(c) => c,
        }
    }
}

impl fmt::Display for DataFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::DataFormat::*;
        match *self {
            Binary =>
                f.write_str("Binary data"),
            Text =>
                f.write_str("Text data"),
            Unicode =>
                f.write_str("Text data (UTF-8)"),
            #[allow(deprecated)]
            MIME =>
                f.write_str("MIME message body part"),
            Unknown(c) =>
                f.write_fmt(format_args!(
                    "Unknown data format identifier {:?}", c)),
        }
    }
}

#[cfg(test)]
impl Arbitrary for DataFormat {
    fn arbitrary(g: &mut Gen) -> Self {
        u8::arbitrary(g).into()
    }
}

impl DataFormat {
    /// Returns an iterator over all valid variants.
    ///
    /// Returns an iterator over all known variants.  This does not
    /// include the [`DataFormat::Unknown`] variants.
    pub fn variants() -> impl Iterator<Item=Self> {
        DATA_FORMAT_VARIANTS.iter().cloned()
    }
}

/// The revocation status.
///
/// # Examples
///
/// Generates a new certificate then checks if the User ID is revoked or not under
/// the given policy using [`ValidUserIDAmalgamation`]:
///
/// [`ValidUserIDAmalgamation`]: crate::cert::amalgamation::ValidUserIDAmalgamation
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
/// use openpgp::policy::StandardPolicy;
/// use openpgp::types::RevocationStatus;
///
/// # fn main() -> openpgp::Result<()> {
/// let p = &StandardPolicy::new();
///
/// let (cert, _) =
///     CertBuilder::general_purpose(None, Some("alice@example.org"))
///     .generate()?;
/// let cert = cert.with_policy(p, None)?;
/// let ua = cert.userids().nth(0).expect("User IDs");
///
/// match ua.revocation_status() {
///     RevocationStatus::Revoked(revs) => {
///         // The certificate holder revoked the User ID.
/// #       unreachable!();
///     }
///     RevocationStatus::CouldBe(revs) => {
///         // There are third-party revocations.  You still need
///         // to check that they are valid (this is necessary,
///         // because without the Certificates are not normally
///         // available to Sequoia).
/// #       unreachable!();
///     }
///     RevocationStatus::NotAsFarAsWeKnow => {
///         // We have no evidence that the User ID is revoked.
///     }
/// }
/// #     Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevocationStatus<'a> {
    /// The key is definitely revoked.
    ///
    /// The relevant self-revocations are returned.
    Revoked(Vec<&'a crate::packet::Signature>),
    /// There is a revocation certificate from a possible designated
    /// revoker.
    CouldBe(Vec<&'a crate::packet::Signature>),
    /// The key does not appear to be revoked.
    ///
    /// An attacker could still have performed a DoS, which prevents
    /// us from seeing the revocation certificate.
    NotAsFarAsWeKnow,
}
assert_send_and_sync!(RevocationStatus<'_>);

#[cfg(test)]
mod tests {
    use super::*;

    quickcheck! {
        fn comp_roundtrip(comp: CompressionAlgorithm) -> bool {
            let val: u8 = comp.into();
            comp == CompressionAlgorithm::from(val)
        }
    }

    quickcheck! {
        fn comp_display(comp: CompressionAlgorithm) -> bool {
            let s = format!("{}", comp);
            !s.is_empty()
        }
    }

    quickcheck! {
        fn comp_parse(comp: CompressionAlgorithm) -> bool {
            match comp {
                CompressionAlgorithm::Unknown(u) => u > 110 || (u > 3 && u < 100),
                CompressionAlgorithm::Private(u) => (100..=110).contains(&u),
                _ => true
            }
        }
    }


    quickcheck! {
        fn sym_roundtrip(sym: SymmetricAlgorithm) -> bool {
            let val: u8 = sym.into();
            sym == SymmetricAlgorithm::from(val)
        }
    }

    quickcheck! {
        fn sym_display(sym: SymmetricAlgorithm) -> bool {
            let s = format!("{}", sym);
            !s.is_empty()
        }
    }

    quickcheck! {
        fn sym_parse(sym: SymmetricAlgorithm) -> bool {
            match sym {
                SymmetricAlgorithm::Unknown(u) =>
                    u == 5 || u == 6 || u > 110 || (u > 10 && u < 100),
                SymmetricAlgorithm::Private(u) =>
                    (100..=110).contains(&u),
                _ => true
            }
        }
    }


    quickcheck! {
        fn aead_roundtrip(aead: AEADAlgorithm) -> bool {
            let val: u8 = aead.into();
            aead == AEADAlgorithm::from(val)
        }
    }

    quickcheck! {
        fn aead_display(aead: AEADAlgorithm) -> bool {
            let s = format!("{}", aead);
            !s.is_empty()
        }
    }

    quickcheck! {
        fn aead_parse(aead: AEADAlgorithm) -> bool {
            match aead {
                AEADAlgorithm::Unknown(u) =>
                    u == 0 || u > 110 || (u > 2 && u < 100),
                AEADAlgorithm::Private(u) =>
                    (100..=110).contains(&u),
                _ => true
            }
        }
    }


    quickcheck! {
        fn pk_roundtrip(pk: PublicKeyAlgorithm) -> bool {
            let val: u8 = pk.into();
            pk == PublicKeyAlgorithm::from(val)
        }
    }

    quickcheck! {
        fn pk_display(pk: PublicKeyAlgorithm) -> bool {
            let s = format!("{}", pk);
            !s.is_empty()
        }
    }

    quickcheck! {
        fn pk_parse(pk: PublicKeyAlgorithm) -> bool {
            match pk {
                PublicKeyAlgorithm::Unknown(u) =>
                    u == 0 || u > 110 || (4..=15).contains(&u)
                    || (18..100).contains(&u),
                PublicKeyAlgorithm::Private(u) => (100..=110).contains(&u),
                _ => true
            }
        }
    }


    quickcheck! {
        fn curve_roundtrip(curve: Curve) -> bool {
            curve == Curve::from_oid(curve.oid())
        }
    }


    quickcheck! {
        fn signature_type_roundtrip(t: SignatureType) -> bool {
            let val: u8 = t.into();
            t == SignatureType::from(val)
        }
    }

    quickcheck! {
        fn signature_type_display(t: SignatureType) -> bool {
            let s = format!("{}", t);
            !s.is_empty()
        }
    }


    quickcheck! {
        fn hash_roundtrip(hash: HashAlgorithm) -> bool {
            let val: u8 = hash.into();
            hash == HashAlgorithm::from(val)
        }
    }

    quickcheck! {
        fn hash_roundtrip_str(hash: HashAlgorithm) -> bool {
            match hash {
                HashAlgorithm::Private(_) | HashAlgorithm::Unknown(_) => true,
                hash => {
                    let s = format!("{}", hash);
                    hash == HashAlgorithm::from_str(&s).unwrap()
                }
            }
        }
    }

    quickcheck! {
        fn hash_roundtrip_text_name(hash: HashAlgorithm) -> bool {
            match hash {
                HashAlgorithm::Private(_) | HashAlgorithm::Unknown(_) => true,
                hash => {
                    let s = hash.text_name().unwrap();
                    hash == HashAlgorithm::from_str(s).unwrap()
                }
            }
        }
    }

    quickcheck! {
        fn hash_display(hash: HashAlgorithm) -> bool {
            let s = format!("{}", hash);
            !s.is_empty()
        }
    }

    quickcheck! {
        fn hash_parse(hash: HashAlgorithm) -> bool {
            match hash {
                HashAlgorithm::Unknown(u) => u == 0 || (u > 11 && u < 100) ||
                    u > 110 || (4..=7).contains(&u) || u == 0,
                HashAlgorithm::Private(u) => (100..=110).contains(&u),
                _ => true
            }
        }
    }

    quickcheck! {
        fn rfr_roundtrip(rfr: ReasonForRevocation) -> bool {
            let val: u8 = rfr.into();
            rfr == ReasonForRevocation::from(val)
        }
    }

    quickcheck! {
        fn rfr_display(rfr: ReasonForRevocation) -> bool {
            let s = format!("{}", rfr);
            !s.is_empty()
        }
    }

    quickcheck! {
        fn rfr_parse(rfr: ReasonForRevocation) -> bool {
            match rfr {
                ReasonForRevocation::Unknown(u) =>
                    (u > 3 && u < 32)
                    || (u > 32 && u < 100)
                    || u > 110,
                ReasonForRevocation::Private(u) =>
                    (100..=110).contains(&u),
                _ => true
            }
        }
    }

    quickcheck! {
        fn df_roundtrip(df: DataFormat) -> bool {
            let val: u8 = df.into();
            df == DataFormat::from(val)
        }
    }

    quickcheck! {
        fn df_display(df: DataFormat) -> bool {
            let s = format!("{}", df);
            !s.is_empty()
        }
    }

    quickcheck! {
        fn df_parse(df: DataFormat) -> bool {
            match df {
                DataFormat::Unknown(u) =>
                    u != 'b' && u != 't' && u != 'u' && u != 'm',
                _ => true
            }
        }
    }

    #[test]
    fn public_key_algorithms_variants() {
        use std::collections::HashSet;
        use std::iter::FromIterator;

        // PUBLIC_KEY_ALGORITHM_VARIANTS is a list.  Derive it in a
        // different way to double check that nothing is missing.
        let derived_variants = (0..=u8::MAX)
            .map(PublicKeyAlgorithm::from)
            .filter(|t| {
                match t {
                    PublicKeyAlgorithm::Private(_) => false,
                    PublicKeyAlgorithm::Unknown(_) => false,
                    _ => true,
                }
            })
            .collect::<HashSet<_>>();

        let known_variants
            = HashSet::from_iter(PUBLIC_KEY_ALGORITHM_VARIANTS.iter().cloned());

        let missing = known_variants
            .symmetric_difference(&derived_variants)
            .collect::<Vec<_>>();

        assert!(missing.is_empty(), "{:?}", missing);
    }

    #[test]
    fn symmetric_algorithms_variants() {
        use std::collections::HashSet;
        use std::iter::FromIterator;

        // SYMMETRIC_ALGORITHM_VARIANTS is a list.  Derive it in a
        // different way to double check that nothing is missing.
        let derived_variants = (0..=u8::MAX)
            .map(SymmetricAlgorithm::from)
            .filter(|t| {
                match t {
                    SymmetricAlgorithm::Unencrypted => false,
                    SymmetricAlgorithm::Private(_) => false,
                    SymmetricAlgorithm::Unknown(_) => false,
                    _ => true,
                }
            })
            .collect::<HashSet<_>>();

        let known_variants
            = HashSet::from_iter(SYMMETRIC_ALGORITHM_VARIANTS
                                 .iter().cloned());

        let missing = known_variants
            .symmetric_difference(&derived_variants)
            .collect::<Vec<_>>();

        assert!(missing.is_empty(), "{:?}", missing);
    }

    #[test]
    fn aead_algorithms_variants() {
        use std::collections::HashSet;
        use std::iter::FromIterator;

        // AEAD_ALGORITHM_VARIANTS is a list.  Derive it in a
        // different way to double check that nothing is missing.
        let derived_variants = (0..=u8::MAX)
            .map(AEADAlgorithm::from)
            .filter(|t| {
                match t {
                    AEADAlgorithm::Private(_) => false,
                    AEADAlgorithm::Unknown(_) => false,
                    _ => true,
                }
            })
            .collect::<HashSet<_>>();

        let known_variants
            = HashSet::from_iter(AEAD_ALGORITHM_VARIANTS
                                 .iter().cloned());

        let missing = known_variants
            .symmetric_difference(&derived_variants)
            .collect::<Vec<_>>();

        assert!(missing.is_empty(), "{:?}", missing);
    }

    #[test]
    fn compression_algorithms_variants() {
        use std::collections::HashSet;
        use std::iter::FromIterator;

        // COMPRESSION_ALGORITHM_VARIANTS is a list.  Derive it in a
        // different way to double check that nothing is missing.
        let derived_variants = (0..=u8::MAX)
            .map(CompressionAlgorithm::from)
            .filter(|t| {
                match t {
                    CompressionAlgorithm::Private(_) => false,
                    CompressionAlgorithm::Unknown(_) => false,
                    _ => true,
                }
            })
            .collect::<HashSet<_>>();

        let known_variants
            = HashSet::from_iter(COMPRESSION_ALGORITHM_VARIANTS
                                 .iter().cloned());

        let missing = known_variants
            .symmetric_difference(&derived_variants)
            .collect::<Vec<_>>();

        assert!(missing.is_empty(), "{:?}", missing);
    }

    #[test]
    fn hash_algorithms_variants() {
        use std::collections::HashSet;
        use std::iter::FromIterator;

        // HASH_ALGORITHM_VARIANTS is a list.  Derive it in a
        // different way to double check that nothing is missing.
        let derived_variants = (0..=u8::MAX)
            .map(HashAlgorithm::from)
            .filter(|t| {
                match t {
                    HashAlgorithm::Private(_) => false,
                    HashAlgorithm::Unknown(_) => false,
                    _ => true,
                }
            })
            .collect::<HashSet<_>>();

        let known_variants
            = HashSet::from_iter(HASH_ALGORITHM_VARIANTS
                                 .iter().cloned());

        let missing = known_variants
            .symmetric_difference(&derived_variants)
            .collect::<Vec<_>>();

        assert!(missing.is_empty(), "{:?}", missing);
    }

    #[test]
    fn signature_types_variants() {
        use std::collections::HashSet;
        use std::iter::FromIterator;

        // SIGNATURE_TYPE_VARIANTS is a list.  Derive it in a
        // different way to double check that nothing is missing.
        let derived_variants = (0..=u8::MAX)
            .map(SignatureType::from)
            .filter(|t| {
                match t {
                    SignatureType::Unknown(_) => false,
                    _ => true,
                }
            })
            .collect::<HashSet<_>>();

        let known_variants
            = HashSet::from_iter(SIGNATURE_TYPE_VARIANTS
                                 .iter().cloned());

        let missing = known_variants
            .symmetric_difference(&derived_variants)
            .collect::<Vec<_>>();

        assert!(missing.is_empty(), "{:?}", missing);
    }

    #[test]
    fn reason_for_revocation_variants() {
        use std::collections::HashSet;
        use std::iter::FromIterator;

        // REASON_FOR_REVOCATION_VARIANTS is a list.  Derive it in a
        // different way to double check that nothing is missing.
        let derived_variants = (0..=u8::MAX)
            .map(ReasonForRevocation::from)
            .filter(|t| {
                match t {
                    ReasonForRevocation::Private(_) => false,
                    ReasonForRevocation::Unknown(_) => false,
                    _ => true,
                }
            })
            .collect::<HashSet<_>>();

        let known_variants
            = HashSet::from_iter(REASON_FOR_REVOCATION_VARIANTS
                                 .iter().cloned());

        let missing = known_variants
            .symmetric_difference(&derived_variants)
            .collect::<Vec<_>>();

        assert!(missing.is_empty(), "{:?}", missing);
    }

    #[test]
    fn data_format_variants() {
        use std::collections::HashSet;
        use std::iter::FromIterator;

        // DATA_FORMAT_VARIANTS is a list.  Derive it in a different
        // way to double check that nothing is missing.
        let derived_variants = (0..=u8::MAX)
            .map(DataFormat::from)
            .filter(|t| {
                match t {
                    DataFormat::Unknown(_) => false,
                    _ => true,
                }
            })
            .collect::<HashSet<_>>();

        let known_variants
            = HashSet::from_iter(DATA_FORMAT_VARIANTS
                                 .iter().cloned());

        let missing = known_variants
            .symmetric_difference(&derived_variants)
            .collect::<Vec<_>>();

        assert!(missing.is_empty(), "{:?}", missing);
    }
}
