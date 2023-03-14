//! Multiprecision Integers.
//!
//! Cryptographic objects like [public keys], [secret keys],
//! [ciphertexts], and [signatures] are scalar numbers of arbitrary
//! precision.  OpenPGP specifies that these are stored encoded as
//! big-endian integers with leading zeros stripped (See [Section 3.2
//! of RFC 4880]).  Multiprecision integers in OpenPGP are extended by
//! [RFC 6637] to store curves and coordinates used in elliptic curve
//! cryptography (ECC).
//!
//!   [public keys]: PublicKey
//!   [secret keys]: SecretKeyMaterial
//!   [ciphertexts]: Ciphertext
//!   [signatures]: Signature
//!   [Section 3.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3.2
//!   [RFC 6637]: https://tools.ietf.org/html/rfc6637

use std::fmt;
use std::cmp::Ordering;
use std::io::Write;
use std::borrow::Cow;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::types::{
    Curve,
    HashAlgorithm,
    PublicKeyAlgorithm,
    SymmetricAlgorithm,
};
use crate::crypto::hash::{self, Hash};
use crate::crypto::mem::{secure_cmp, Protected};
use crate::serialize::Marshal;

use crate::Error;
use crate::Result;

/// A Multiprecision Integer.
#[derive(Clone)]
pub struct MPI {
    /// Integer value as big-endian with leading zeros stripped.
    value: Box<[u8]>,
}
assert_send_and_sync!(MPI);

impl From<Vec<u8>> for MPI {
    fn from(v: Vec<u8>) -> Self {
        Self::new(&v)
    }
}

impl MPI {
    /// Creates a new MPI.
    ///
    /// This function takes care of removing leading zeros.
    pub fn new(value: &[u8]) -> Self {
        let mut leading_zeros = 0;
        for b in value {
            leading_zeros += b.leading_zeros() as usize;
            if *b != 0 {
                break;
            }
        }

        let offset = leading_zeros / 8;
        let value = Vec::from(&value[offset..]).into_boxed_slice();

        MPI {
            value,
        }
    }

    /// Creates new MPI encoding an uncompressed EC point.
    ///
    /// Encodes the given point on a elliptic curve (see [Section 6 of
    /// RFC 6637] for details).  This is used to encode public keys
    /// and ciphertexts for the NIST curves (`NistP256`, `NistP384`,
    /// and `NistP521`).
    ///
    ///   [Section 6 of RFC 6637]: https://tools.ietf.org/html/rfc6637#section-6
    pub fn new_point(x: &[u8], y: &[u8], field_bits: usize) -> Self {
        Self::new_point_common(x, y, field_bits).into()
    }

    /// Common implementation shared between MPI and ProtectedMPI.
    fn new_point_common(x: &[u8], y: &[u8], field_bits: usize) -> Vec<u8> {
        let field_sz = if field_bits % 8 > 0 { 1 } else { 0 } + field_bits / 8;
        let mut val = vec![0x0u8; 1 + 2 * field_sz];
        let x_missing = field_sz - x.len();
        let y_missing = field_sz - y.len();

        val[0] = 0x4;
        val[1 + x_missing..1 + field_sz].copy_from_slice(x);
        val[1 + field_sz + y_missing..].copy_from_slice(y);
        val
    }

    /// Creates new MPI encoding a compressed EC point using native
    /// encoding.
    ///
    /// Encodes the given point on a elliptic curve (see [Section 13.2
    /// of RFC4880bis] for details).  This is used to encode public
    /// keys and ciphertexts for the Bernstein curves (currently
    /// `X25519`).
    ///
    ///   [Section 13.2 of RFC4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09#section-13.2
    pub fn new_compressed_point(x: &[u8]) -> Self {
        Self::new_compressed_point_common(x).into()
    }

    /// Common implementation shared between MPI and ProtectedMPI.
    fn new_compressed_point_common(x: &[u8]) -> Vec<u8> {
        let mut val = vec![0; 1 + x.len()];
        val[0] = 0x40;
        val[1..].copy_from_slice(x);
        val
    }

    /// Creates a new MPI representing zero.
    pub fn zero() -> Self {
        Self::new(&[])
    }

    /// Tests whether the MPI represents zero.
    pub fn is_zero(&self) -> bool {
        self.value().is_empty()
    }

    /// Returns the length of the MPI in bits.
    ///
    /// Leading zero-bits are not included in the returned size.
    pub fn bits(&self) -> usize {
        self.value.len() * 8
            - self.value.get(0).map(|&b| b.leading_zeros() as usize)
                  .unwrap_or(0)
    }

    /// Returns the value of this MPI.
    ///
    /// Note that due to stripping of zero-bytes, the returned value
    /// may be shorter than expected.
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// Returns the value of this MPI zero-padded to the given length.
    ///
    /// MPI-encoding strips leading zero-bytes.  This function adds
    /// them back, if necessary.  If the size exceeds `to`, an error
    /// is returned.
    pub fn value_padded(&self, to: usize) -> Result<Cow<[u8]>> {
        crate::crypto::pad(self.value(), to)
    }

    /// Decodes an EC point encoded as MPI.
    ///
    /// Decodes the MPI into a point on an elliptic curve (see
    /// [Section 6 of RFC 6637] and [Section 13.2 of RFC4880bis] for
    /// details).  If the point is not compressed, the function
    /// returns `(x, y)`.  If it is compressed, `y` will be empty.
    ///
    ///   [Section 6 of RFC 6637]: https://tools.ietf.org/html/rfc6637#section-6
    ///   [Section 13.2 of RFC4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09#section-13.2
    ///
    /// # Errors
    ///
    /// Returns `Error::UnsupportedEllipticCurve` if the curve is not
    /// supported, `Error::MalformedMPI` if the point is formatted
    /// incorrectly, `Error::InvalidOperation` if the given curve is
    /// operating on native octet strings.
    pub fn decode_point(&self, curve: &Curve) -> Result<(&[u8], &[u8])> {
        Self::decode_point_common(self.value(), curve)
    }

    /// Common implementation shared between MPI and ProtectedMPI.
    fn decode_point_common<'a>(value: &'a [u8], curve: &Curve)
                               -> Result<(&'a [u8], &'a [u8])> {
        const ED25519_KEY_SIZE: usize = 32;
        const CURVE25519_SIZE: usize = 32;
        use self::Curve::*;
        match &curve {
            Ed25519 | Cv25519 => {
                assert_eq!(CURVE25519_SIZE, ED25519_KEY_SIZE);
                // This curve uses a custom compression format which
                // only contains the X coordinate.
                if value.len() != 1 + CURVE25519_SIZE {
                    return Err(Error::MalformedMPI(
                        format!("Bad size of Curve25519 key: {} expected: {}",
                                value.len(),
                                1 + CURVE25519_SIZE
                        )
                    ).into());
                }

                if value.get(0).map(|&b| b != 0x40).unwrap_or(true) {
                    return Err(Error::MalformedMPI(
                        "Bad encoding of Curve25519 key".into()).into());
                }

                Ok((&value[1..], &[]))
            },

            NistP256
                | NistP384
                | NistP521
                | BrainpoolP256
                | BrainpoolP512
                =>
            {
                // Length of one coordinate in bytes, rounded up.
                let coordinate_length = (curve.len()? + 7) / 8;

                // Check length of Q.
                let expected_length =
                    1 // 0x04.
                    + (2 // (x, y)
                       * coordinate_length);

                if value.len() != expected_length {
                    return Err(Error::MalformedMPI(
                        format!("Invalid length of MPI: {} (expected {})",
                                value.len(), expected_length)).into());
                }

                if value.get(0).map(|&b| b != 0x04).unwrap_or(true) {
                    return Err(Error::MalformedMPI(
                        format!("Bad prefix: {:?} (expected Some(0x04))",
                                value.get(0))).into());
                }

                Ok((&value[1..1 + coordinate_length],
                    &value[1 + coordinate_length..]))
            },

            Unknown(_) =>
                Err(Error::UnsupportedEllipticCurve(curve.clone()).into()),
        }
    }

    /// Securely compares two MPIs in constant time.
    fn secure_memcmp(&self, other: &Self) -> Ordering {
        let cmp = unsafe {
            if self.value.len() == other.value.len() {
                ::memsec::memcmp(self.value.as_ptr(), other.value.as_ptr(),
                                 other.value.len())
            } else {
                self.value.len() as i32 - other.value.len() as i32
            }
        };

        match cmp {
            0 => Ordering::Equal,
            x if x < 0 => Ordering::Less,
            _ => Ordering::Greater,
        }
    }
}

impl fmt::Debug for MPI {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!(
            "{} bits: {}", self.bits(),
            crate::fmt::to_hex(&*self.value, true)))
    }
}

impl Hash for MPI {
    fn hash(&self, hash: &mut dyn hash::Digest) {
        let len = self.bits() as u16;

        hash.update(&len.to_be_bytes());
        hash.update(&self.value);
    }
}

#[cfg(test)]
impl Arbitrary for MPI {
    fn arbitrary(g: &mut Gen) -> Self {
        loop {
            let buf = <Vec<u8>>::arbitrary(g);

            if !buf.is_empty() && buf[0] != 0 {
                break MPI::new(&buf);
            }
        }
    }
}

impl PartialOrd for MPI {
    fn partial_cmp(&self, other: &MPI) -> Option<Ordering> {
        Some(self.secure_memcmp(other))
    }
}

impl Ord for MPI {
    fn cmp(&self, other: &MPI) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl PartialEq for MPI {
    fn eq(&self, other: &MPI) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for MPI {}

impl std::hash::Hash for MPI {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

/// Holds a single MPI containing secrets.
///
/// The memory will be cleared when the object is dropped.  Used by
/// [`SecretKeyMaterial`] to protect secret keys.
///
#[derive(Clone)]
pub struct ProtectedMPI {
    /// Integer value as big-endian.
    value: Protected,
}
assert_send_and_sync!(ProtectedMPI);

impl From<Vec<u8>> for ProtectedMPI {
    fn from(m: Vec<u8>) -> Self {
        MPI::from(m).into()
    }
}

impl From<Protected> for ProtectedMPI {
    fn from(m: Protected) -> Self {
        MPI::new(&m).into()
    }
}

impl From<MPI> for ProtectedMPI {
    fn from(m: MPI) -> Self {
        ProtectedMPI {
            value: m.value.into(),
        }
    }
}

impl PartialOrd for ProtectedMPI {
    fn partial_cmp(&self, other: &ProtectedMPI) -> Option<Ordering> {
        Some(self.secure_memcmp(other))
    }
}

impl Ord for ProtectedMPI {
    fn cmp(&self, other: &ProtectedMPI) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl PartialEq for ProtectedMPI {
    fn eq(&self, other: &ProtectedMPI) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ProtectedMPI {}

impl std::hash::Hash for ProtectedMPI {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl ProtectedMPI {
    /// Creates new MPI encoding an uncompressed EC point.
    ///
    /// Encodes the given point on a elliptic curve (see [Section 6 of
    /// RFC 6637] for details).  This is used to encode public keys
    /// and ciphertexts for the NIST curves (`NistP256`, `NistP384`,
    /// and `NistP521`).
    ///
    ///   [Section 6 of RFC 6637]: https://tools.ietf.org/html/rfc6637#section-6
    pub fn new_point(x: &[u8], y: &[u8], field_bits: usize) -> Self {
        MPI::new_point_common(x, y, field_bits).into()
    }

    /// Creates new MPI encoding a compressed EC point using native
    /// encoding.
    ///
    /// Encodes the given point on a elliptic curve (see [Section 13.2
    /// of RFC4880bis] for details).  This is used to encode public
    /// keys and ciphertexts for the Bernstein curves (currently
    /// `X25519`).
    ///
    ///   [Section 13.2 of RFC4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09#section-13.2
    pub fn new_compressed_point(x: &[u8]) -> Self {
        MPI::new_compressed_point_common(x).into()
    }

    /// Returns the length of the MPI in bits.
    ///
    /// Leading zero-bits are not included in the returned size.
    pub fn bits(&self) -> usize {
        self.value.len() * 8
            - self.value.get(0).map(|&b| b.leading_zeros() as usize)
                  .unwrap_or(0)
    }

    /// Returns the value of this MPI.
    ///
    /// Note that due to stripping of zero-bytes, the returned value
    /// may be shorter than expected.
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// Returns the value of this MPI zero-padded to the given length.
    ///
    /// MPI-encoding strips leading zero-bytes.  This function adds
    /// them back.  This operation is done unconditionally to avoid
    /// timing differences.  If the size exceeds `to`, the result is
    /// silently truncated to avoid timing differences.
    pub fn value_padded(&self, to: usize) -> Protected {
        let missing = to.saturating_sub(self.value.len());
        let limit = self.value.len().min(to);
        let mut v: Protected = vec![0; to].into();
        v[missing..].copy_from_slice(&self.value()[..limit]);
        v
    }

    /// Decodes an EC point encoded as MPI.
    ///
    /// Decodes the MPI into a point on an elliptic curve (see
    /// [Section 6 of RFC 6637] and [Section 13.2 of RFC4880bis] for
    /// details).  If the point is not compressed, the function
    /// returns `(x, y)`.  If it is compressed, `y` will be empty.
    ///
    ///   [Section 6 of RFC 6637]: https://tools.ietf.org/html/rfc6637#section-6
    ///   [Section 13.2 of RFC4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09#section-13.2
    ///
    /// # Errors
    ///
    /// Returns `Error::UnsupportedEllipticCurve` if the curve is not
    /// supported, `Error::MalformedMPI` if the point is formatted
    /// incorrectly, `Error::InvalidOperation` if the given curve is
    /// operating on native octet strings.
    pub fn decode_point(&self, curve: &Curve) -> Result<(&[u8], &[u8])> {
        MPI::decode_point_common(self.value(), curve)
    }

    /// Securely compares two MPIs in constant time.
    fn secure_memcmp(&self, other: &Self) -> Ordering {
        (self.value.len() as i32).cmp(&(other.value.len() as i32))
            .then(
                // Protected compares in constant time.
                self.value.cmp(&other.value))
    }
}

impl fmt::Debug for ProtectedMPI {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if cfg!(debug_assertions) {
            f.write_fmt(format_args!(
                "{} bits: {}", self.bits(),
                crate::fmt::to_hex(&*self.value, true)))
        } else {
            f.write_str("<Redacted>")
        }
    }
}

/// A public key.
///
/// Provides a typed and structured way of storing multiple MPIs (and
/// the occasional elliptic curve) in [`Key`] packets.
///
///   [`Key`]: crate::packet::Key
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum PublicKey {
    /// RSA public key.
    RSA {
        /// Public exponent
        e: MPI,
        /// Public modulo N = pq.
        n: MPI,
    },

    /// NIST DSA public key.
    DSA {
        /// Prime of the ring Zp.
        p: MPI,
        /// Order of `g` in Zp.
        q: MPI,
        /// Public generator of Zp.
        g: MPI,
        /// Public key g^x mod p.
        y: MPI,
    },

    /// ElGamal public key.
    ElGamal {
        /// Prime of the ring Zp.
        p: MPI,
        /// Generator of Zp.
        g: MPI,
        /// Public key g^x mod p.
        y: MPI,
    },

    /// DJB's "Twisted" Edwards curve DSA public key.
    EdDSA {
        /// Curve we're using. Must be curve 25519.
        curve: Curve,
        /// Public point.
        q: MPI,
    },

    /// NIST's Elliptic Curve DSA public key.
    ECDSA {
        /// Curve we're using.
        curve: Curve,
        /// Public point.
        q: MPI,
    },

    /// Elliptic Curve Diffie-Hellman public key.
    ECDH {
        /// Curve we're using.
        curve: Curve,
        /// Public point.
        q: MPI,
        /// Algorithm used to derive the Key Encapsulation Key.
        hash: HashAlgorithm,
        /// Algorithm used to encapsulate the session key.
        sym: SymmetricAlgorithm,
    },

    /// Unknown number of MPIs for an unknown algorithm.
    Unknown {
        /// The successfully parsed MPIs.
        mpis: Box<[MPI]>,
        /// Any data that failed to parse.
        rest: Box<[u8]>,
    },
}
assert_send_and_sync!(PublicKey);

impl PublicKey {
    /// Returns the length of the public key in bits.
    ///
    /// For finite field crypto this returns the size of the field we
    /// operate in, for ECC it returns `Curve::bits()`.
    ///
    /// Note: This information is useless and should not be used to
    /// gauge the security of a particular key. This function exists
    /// only because some legacy PGP application like HKP need it.
    ///
    /// Returns `None` for unknown keys and curves.
    pub fn bits(&self) -> Option<usize> {
        use self::PublicKey::*;
        match self {
            RSA { ref n,.. } => Some(n.bits()),
            DSA { ref p,.. } => Some(p.bits()),
            ElGamal { ref p,.. } => Some(p.bits()),
            EdDSA { ref curve,.. } => curve.bits(),
            ECDSA { ref curve,.. } => curve.bits(),
            ECDH { ref curve,.. } => curve.bits(),
            Unknown { .. } => None,
        }
    }

    /// Returns, if known, the public-key algorithm for this public
    /// key.
    pub fn algo(&self) -> Option<PublicKeyAlgorithm> {
        use self::PublicKey::*;
        match self {
            RSA { .. } => Some(PublicKeyAlgorithm::RSAEncryptSign),
            DSA { .. } => Some(PublicKeyAlgorithm::DSA),
            ElGamal { .. } => Some(PublicKeyAlgorithm::ElGamalEncrypt),
            EdDSA { .. } => Some(PublicKeyAlgorithm::EdDSA),
            ECDSA { .. } => Some(PublicKeyAlgorithm::ECDSA),
            ECDH { .. } => Some(PublicKeyAlgorithm::ECDH),
            Unknown { .. } => None,
        }
    }
}

impl Hash for PublicKey {
    fn hash(&self, mut hash: &mut dyn hash::Digest) {
        self.serialize(&mut hash as &mut dyn Write)
            .expect("hashing does not fail")
    }
}

#[cfg(test)]
impl Arbitrary for PublicKey {
    fn arbitrary(g: &mut Gen) -> Self {
        use self::PublicKey::*;
        use crate::arbitrary_helper::gen_arbitrary_from_range;

        match gen_arbitrary_from_range(0..6, g) {
            0 => RSA {
                e: MPI::arbitrary(g),
                n: MPI::arbitrary(g),
            },

            1 => DSA {
                p: MPI::arbitrary(g),
                q: MPI::arbitrary(g),
                g: MPI::arbitrary(g),
                y: MPI::arbitrary(g),
            },

            2 => ElGamal {
                p: MPI::arbitrary(g),
                g: MPI::arbitrary(g),
                y: MPI::arbitrary(g),
            },

            3 => EdDSA {
                curve: Curve::arbitrary(g),
                q: MPI::arbitrary(g),
            },

            4 => ECDSA {
                curve: Curve::arbitrary(g),
                q: MPI::arbitrary(g),
            },

            5 => ECDH {
                curve: Curve::arbitrary(g),
                q: MPI::arbitrary(g),
                hash: HashAlgorithm::arbitrary(g),
                sym: SymmetricAlgorithm::arbitrary(g),
            },

            _ => unreachable!(),
        }
    }
}

/// A secret key.
///
/// Provides a typed and structured way of storing multiple MPIs in
/// [`Key`] packets.  Secret key components are protected by storing
/// them using [`ProtectedMPI`].
///
///   [`Key`]: crate::packet::Key
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
// Deriving Hash here is okay: PartialEq is manually implemented to
// ensure that secrets are compared in constant-time.
#[allow(clippy::derive_hash_xor_eq)]
#[non_exhaustive]
#[derive(Clone, Hash)]
pub enum SecretKeyMaterial {
    /// RSA secret key.
    RSA {
        /// Secret exponent, inverse of e in Phi(N).
        d: ProtectedMPI,
        /// Smaller secret prime.
        p: ProtectedMPI,
        /// Larger secret prime.
        q: ProtectedMPI,
        /// Inverse of p mod q.
        u: ProtectedMPI,
    },

    /// NIST DSA secret key.
    DSA {
        /// Secret key log_g(y) in Zp.
        x: ProtectedMPI,
    },

    /// ElGamal secret key.
    ElGamal {
        /// Secret key log_g(y) in Zp.
        x: ProtectedMPI,
    },

    /// DJB's "Twisted" Edwards curve DSA secret key.
    EdDSA {
        /// Secret scalar.
        scalar: ProtectedMPI,
    },

    /// NIST's Elliptic Curve DSA secret key.
    ECDSA {
        /// Secret scalar.
        scalar: ProtectedMPI,
    },

    /// Elliptic Curve Diffie-Hellman public key.
    ECDH {
        /// Secret scalar.
        scalar: ProtectedMPI,
    },

    /// Unknown number of MPIs for an unknown algorithm.
    Unknown {
        /// The successfully parsed MPIs.
        mpis: Box<[ProtectedMPI]>,
        /// Any data that failed to parse.
        rest: Protected,
    },
}
assert_send_and_sync!(SecretKeyMaterial);

impl fmt::Debug for SecretKeyMaterial {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if cfg!(debug_assertions) {
            match self {
                SecretKeyMaterial::RSA{ ref d, ref p, ref q, ref u } =>
                    write!(f, "RSA {{ d: {:?}, p: {:?}, q: {:?}, u: {:?} }}", d, p, q, u),
                SecretKeyMaterial::DSA{ ref x } =>
                    write!(f, "DSA {{ x: {:?} }}", x),
                SecretKeyMaterial::ElGamal{ ref x } =>
                    write!(f, "ElGamal {{ x: {:?} }}", x),
                SecretKeyMaterial::EdDSA{ ref scalar } =>
                    write!(f, "EdDSA {{ scalar: {:?} }}", scalar),
                SecretKeyMaterial::ECDSA{ ref scalar } =>
                    write!(f, "ECDSA {{ scalar: {:?} }}", scalar),
                SecretKeyMaterial::ECDH{ ref scalar } =>
                    write!(f, "ECDH {{ scalar: {:?} }}", scalar),
                SecretKeyMaterial::Unknown{ ref mpis, ref rest } =>
                    write!(f, "Unknown {{ mips: {:?}, rest: {:?} }}", mpis, rest),
            }
        } else {
            match self {
                SecretKeyMaterial::RSA{ .. } =>
                    f.write_str("RSA { <Redacted> }"),
                SecretKeyMaterial::DSA{ .. } =>
                    f.write_str("DSA { <Redacted> }"),
                SecretKeyMaterial::ElGamal{ .. } =>
                    f.write_str("ElGamal { <Redacted> }"),
                SecretKeyMaterial::EdDSA{ .. } =>
                    f.write_str("EdDSA { <Redacted> }"),
                SecretKeyMaterial::ECDSA{ .. } =>
                    f.write_str("ECDSA { <Redacted> }"),
                SecretKeyMaterial::ECDH{ .. } =>
                    f.write_str("ECDH { <Redacted> }"),
                SecretKeyMaterial::Unknown{ .. } =>
                    f.write_str("Unknown { <Redacted> }"),
            }
        }
    }
}

impl PartialOrd for SecretKeyMaterial {
    fn partial_cmp(&self, other: &SecretKeyMaterial) -> Option<Ordering> {
        use std::iter;

        fn discriminant(sk: &SecretKeyMaterial) -> usize {
            match sk {
                SecretKeyMaterial::RSA{ .. } => 0,
                SecretKeyMaterial::DSA{ .. } => 1,
                SecretKeyMaterial::ElGamal{ .. } => 2,
                SecretKeyMaterial::EdDSA{ .. } => 3,
                SecretKeyMaterial::ECDSA{ .. } => 4,
                SecretKeyMaterial::ECDH{ .. } => 5,
                SecretKeyMaterial::Unknown{ .. } => 6,
            }
        }

        let ret = match (self, other) {
            (&SecretKeyMaterial::RSA{ d: ref d1, p: ref p1, q: ref q1, u: ref u1 }
            ,&SecretKeyMaterial::RSA{ d: ref d2, p: ref p2, q: ref q2, u: ref u2 }) => {
                let o1 = d1.cmp(d2);
                let o2 = p1.cmp(p2);
                let o3 = q1.cmp(q2);
                let o4 = u1.cmp(u2);

                if o1 != Ordering::Equal { return Some(o1); }
                if o2 != Ordering::Equal { return Some(o2); }
                if o3 != Ordering::Equal { return Some(o3); }
                o4
            }
            (&SecretKeyMaterial::DSA{ x: ref x1 }
            ,&SecretKeyMaterial::DSA{ x: ref x2 }) => {
                x1.cmp(x2)
            }
            (&SecretKeyMaterial::ElGamal{ x: ref x1 }
            ,&SecretKeyMaterial::ElGamal{ x: ref x2 }) => {
                x1.cmp(x2)
            }
            (&SecretKeyMaterial::EdDSA{ scalar: ref scalar1 }
            ,&SecretKeyMaterial::EdDSA{ scalar: ref scalar2 }) => {
                scalar1.cmp(scalar2)
            }
            (&SecretKeyMaterial::ECDSA{ scalar: ref scalar1 }
            ,&SecretKeyMaterial::ECDSA{ scalar: ref scalar2 }) => {
                scalar1.cmp(scalar2)
            }
            (&SecretKeyMaterial::ECDH{ scalar: ref scalar1 }
            ,&SecretKeyMaterial::ECDH{ scalar: ref scalar2 }) => {
                scalar1.cmp(scalar2)
            }
            (&SecretKeyMaterial::Unknown{ mpis: ref mpis1, rest: ref rest1 }
            ,&SecretKeyMaterial::Unknown{ mpis: ref mpis2, rest: ref rest2 }) => {
                let o1 = secure_cmp(rest1, rest2);
                let on = mpis1.iter().zip(mpis2.iter()).map(|(a,b)| {
                    a.cmp(b)
                }).collect::<Vec<_>>();

                iter::once(o1)
                    .chain(on.iter().cloned())
                    .fold(Ordering::Equal, |acc, x| acc.then(x))
            }

            (a, b) => {
                let ret = discriminant(a).cmp(&discriminant(b));

                assert!(ret != Ordering::Equal);
                ret
            }
        };

        Some(ret)
    }
}

impl Ord for SecretKeyMaterial {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl PartialEq for SecretKeyMaterial {
    fn eq(&self, other: &Self) -> bool { self.cmp(other) == Ordering::Equal }
}

impl Eq for SecretKeyMaterial {}

impl SecretKeyMaterial {
    /// Returns, if known, the public-key algorithm for this secret
    /// key.
    pub fn algo(&self) -> Option<PublicKeyAlgorithm> {
        use self::SecretKeyMaterial::*;
        match self {
            RSA { .. } => Some(PublicKeyAlgorithm::RSAEncryptSign),
            DSA { .. } => Some(PublicKeyAlgorithm::DSA),
            ElGamal { .. } => Some(PublicKeyAlgorithm::ElGamalEncrypt),
            EdDSA { .. } => Some(PublicKeyAlgorithm::EdDSA),
            ECDSA { .. } => Some(PublicKeyAlgorithm::ECDSA),
            ECDH { .. } => Some(PublicKeyAlgorithm::ECDH),
            Unknown { .. } => None,
        }
    }
}

impl Hash for SecretKeyMaterial {
    fn hash(&self, mut hash: &mut dyn hash::Digest) {
        self.serialize(&mut hash as &mut dyn Write)
            .expect("hashing does not fail")
    }
}

#[cfg(test)]
impl Arbitrary for SecretKeyMaterial {
    fn arbitrary(g: &mut Gen) -> Self {
        use crate::arbitrary_helper::gen_arbitrary_from_range;

        match gen_arbitrary_from_range(0..6, g) {
            0 => SecretKeyMaterial::RSA {
                d: MPI::arbitrary(g).into(),
                p: MPI::arbitrary(g).into(),
                q: MPI::arbitrary(g).into(),
                u: MPI::arbitrary(g).into(),
            },

            1 => SecretKeyMaterial::DSA {
                x: MPI::arbitrary(g).into(),
            },

            2 => SecretKeyMaterial::ElGamal {
                x: MPI::arbitrary(g).into(),
            },

            3 => SecretKeyMaterial::EdDSA {
                scalar: MPI::arbitrary(g).into(),
            },

            4 => SecretKeyMaterial::ECDSA {
                scalar: MPI::arbitrary(g).into(),
            },

            5 => SecretKeyMaterial::ECDH {
                scalar: MPI::arbitrary(g).into(),
            },

            _ => unreachable!(),
        }
    }
}

/// Checksum method for secret key material.
///
/// Secret key material may be protected by a checksum.  See [Section
/// 5.5.3 of RFC 4880] for details.
///
///   [Section 5.5.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.5.3
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum SecretKeyChecksum {
    /// SHA1 over the decrypted secret key.
    SHA1,

    /// Sum of the decrypted secret key octets modulo 65536.
    Sum16,
}
assert_send_and_sync!(SecretKeyChecksum);

impl Default for SecretKeyChecksum {
    fn default() -> Self {
        SecretKeyChecksum::SHA1
    }
}

/// An encrypted session key.
///
/// Provides a typed and structured way of storing multiple MPIs in
/// [`PKESK`] packets.
///
///   [`PKESK`]: crate::packet::PKESK
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Ciphertext {
    /// RSA ciphertext.
    RSA {
        ///  m^e mod N.
        c: MPI,
    },

    /// ElGamal ciphertext.
    ElGamal {
        /// Ephemeral key.
        e: MPI,
        /// .
        c: MPI,
    },

    /// Elliptic curve ElGamal public key.
    ECDH {
        /// Ephemeral key.
        e: MPI,
        /// Symmetrically encrypted session key.
        key: Box<[u8]>,
    },

    /// Unknown number of MPIs for an unknown algorithm.
    Unknown {
        /// The successfully parsed MPIs.
        mpis: Box<[MPI]>,
        /// Any data that failed to parse.
        rest: Box<[u8]>,
    },
}
assert_send_and_sync!(Ciphertext);

impl Ciphertext {
    /// Returns, if known, the public-key algorithm for this
    /// ciphertext.
    pub fn pk_algo(&self) -> Option<PublicKeyAlgorithm> {
        use self::Ciphertext::*;

        // Fields are mostly MPIs that consist of two octets length
        // plus the big endian value itself. All other field types are
        // commented.
        match self {
            RSA { .. } => Some(PublicKeyAlgorithm::RSAEncryptSign),
            ElGamal { .. } => Some(PublicKeyAlgorithm::ElGamalEncrypt),
            ECDH { .. } => Some(PublicKeyAlgorithm::ECDH),
            Unknown { .. } => None,
        }
    }
}

impl Hash for Ciphertext {
    fn hash(&self, mut hash: &mut dyn hash::Digest) {
        self.serialize(&mut hash as &mut dyn Write)
            .expect("hashing does not fail")
    }
}

#[cfg(test)]
impl Arbitrary for Ciphertext {
    fn arbitrary(g: &mut Gen) -> Self {
        use crate::arbitrary_helper::gen_arbitrary_from_range;

        match gen_arbitrary_from_range(0..3, g) {
            0 => Ciphertext::RSA {
                c: MPI::arbitrary(g),
            },

            1 => Ciphertext::ElGamal {
                e: MPI::arbitrary(g),
                c: MPI::arbitrary(g)
            },

            2 => Ciphertext::ECDH {
                e: MPI::arbitrary(g),
                key: {
                    let mut k = <Vec<u8>>::arbitrary(g);
                    k.truncate(255);
                    k.into_boxed_slice()
                },
            },
            _ => unreachable!(),
        }
    }
}

/// A cryptographic signature.
///
/// Provides a typed and structured way of storing multiple MPIs in
/// [`Signature`] packets.
///
///   [`Signature`]: crate::packet::Signature
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Signature {
    /// RSA signature.
    RSA {
        /// Signature m^d mod N.
        s: MPI,
    },

    /// NIST's DSA signature.
    DSA {
        /// `r` value.
        r: MPI,
        /// `s` value.
        s: MPI,
    },

    /// ElGamal signature.
    ElGamal {
        /// `r` value.
        r: MPI,
        /// `s` value.
        s: MPI,
    },

    /// DJB's "Twisted" Edwards curve DSA signature.
    EdDSA {
        /// `r` value.
        r: MPI,
        /// `s` value.
        s: MPI,
    },

    /// NIST's Elliptic curve DSA signature.
    ECDSA {
        /// `r` value.
        r: MPI,
        /// `s` value.
        s: MPI,
    },

    /// Unknown number of MPIs for an unknown algorithm.
    Unknown {
        /// The successfully parsed MPIs.
        mpis: Box<[MPI]>,
        /// Any data that failed to parse.
        rest: Box<[u8]>,
    },
}
assert_send_and_sync!(Signature);

impl Hash for Signature {
    fn hash(&self, mut hash: &mut dyn hash::Digest) {
        self.serialize(&mut hash as &mut dyn Write)
            .expect("hashing does not fail")
    }
}

#[cfg(test)]
impl Arbitrary for Signature {
    fn arbitrary(g: &mut Gen) -> Self {
        use crate::arbitrary_helper::gen_arbitrary_from_range;

        match gen_arbitrary_from_range(0..4, g) {
            0 => Signature::RSA  {
                s: MPI::arbitrary(g),
            },

            1 => Signature::DSA {
                r: MPI::arbitrary(g),
                s: MPI::arbitrary(g),
            },

            2 => Signature::EdDSA  {
                r: MPI::arbitrary(g),
                s: MPI::arbitrary(g),
            },

            3 => Signature::ECDSA  {
                r: MPI::arbitrary(g),
                s: MPI::arbitrary(g),
            },

            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::Parse;

    quickcheck! {
        fn mpi_roundtrip(mpi: MPI) -> bool {
            let mut buf = Vec::new();
            mpi.serialize(&mut buf).unwrap();
            MPI::from_bytes(&buf).unwrap() == mpi
        }
    }

    quickcheck! {
        fn pk_roundtrip(pk: PublicKey) -> bool {
            use std::io::Cursor;
            use crate::PublicKeyAlgorithm::*;

            let mut buf = Vec::new();
            pk.serialize(&mut buf).unwrap();
            let cur = Cursor::new(buf);

            #[allow(deprecated)]
            let pk_ = match &pk {
                PublicKey::RSA { .. } =>
                    PublicKey::parse(RSAEncryptSign, cur).unwrap(),
                PublicKey::DSA { .. } =>
                    PublicKey::parse(DSA, cur).unwrap(),
                PublicKey::ElGamal { .. } =>
                    PublicKey::parse(ElGamalEncrypt, cur).unwrap(),
                PublicKey::EdDSA { .. } =>
                    PublicKey::parse(EdDSA, cur).unwrap(),
                PublicKey::ECDSA { .. } =>
                    PublicKey::parse(ECDSA, cur).unwrap(),
                PublicKey::ECDH { .. } =>
                    PublicKey::parse(ECDH, cur).unwrap(),

                PublicKey::Unknown { .. } => unreachable!(),
            };

            pk == pk_
        }
    }

    #[test]
    fn pk_bits() {
        for (name, key_no, bits) in &[
            ("testy.pgp", 0, 2048),
            ("testy-new.pgp", 1, 256),
            ("dennis-simon-anton.pgp", 0, 2048),
            ("dsa2048-elgamal3072.pgp", 1, 3072),
            ("emmelie-dorothea-dina-samantha-awina-ed25519.pgp", 0, 256),
            ("erika-corinna-daniela-simone-antonia-nistp256.pgp", 0, 256),
            ("erika-corinna-daniela-simone-antonia-nistp384.pgp", 0, 384),
            ("erika-corinna-daniela-simone-antonia-nistp521.pgp", 0, 521),
        ] {
            let cert = crate::Cert::from_bytes(crate::tests::key(name)).unwrap();
            let ka = cert.keys().nth(*key_no).unwrap();
            assert_eq!(ka.key().mpis().bits().unwrap(), *bits,
                       "Cert {}, key no {}", name, *key_no);
        }
    }

    quickcheck! {
        fn sk_roundtrip(sk: SecretKeyMaterial) -> bool {
            use std::io::Cursor;
            use crate::PublicKeyAlgorithm::*;

            let mut buf = Vec::new();
            sk.serialize(&mut buf).unwrap();
            let cur = Cursor::new(buf);

            #[allow(deprecated)]
            let sk_ = match &sk {
                SecretKeyMaterial::RSA { .. } =>
                    SecretKeyMaterial::parse(RSAEncryptSign, cur).unwrap(),
                SecretKeyMaterial::DSA { .. } =>
                    SecretKeyMaterial::parse(DSA, cur).unwrap(),
                SecretKeyMaterial::EdDSA { .. } =>
                    SecretKeyMaterial::parse(EdDSA, cur).unwrap(),
                SecretKeyMaterial::ECDSA { .. } =>
                    SecretKeyMaterial::parse(ECDSA, cur).unwrap(),
                SecretKeyMaterial::ECDH { .. } =>
                    SecretKeyMaterial::parse(ECDH, cur).unwrap(),
                SecretKeyMaterial::ElGamal { .. } =>
                    SecretKeyMaterial::parse(ElGamalEncrypt, cur).unwrap(),

                SecretKeyMaterial::Unknown { .. } => unreachable!(),
            };

            sk == sk_
        }
    }

    quickcheck! {
        fn ct_roundtrip(ct: Ciphertext) -> bool {
            use std::io::Cursor;
            use crate::PublicKeyAlgorithm::*;

            let mut buf = Vec::new();
            ct.serialize(&mut buf).unwrap();
            let cur = Cursor::new(buf);

            #[allow(deprecated)]
            let ct_ = match &ct {
                Ciphertext::RSA { .. } =>
                    Ciphertext::parse(RSAEncryptSign, cur).unwrap(),
                Ciphertext::ElGamal { .. } =>
                    Ciphertext::parse(ElGamalEncrypt, cur).unwrap(),
                Ciphertext::ECDH { .. } =>
                    Ciphertext::parse(ECDH, cur).unwrap(),

                Ciphertext::Unknown { .. } => unreachable!(),
            };

            ct == ct_
        }
    }

    quickcheck! {
        fn signature_roundtrip(sig: Signature) -> bool {
            use std::io::Cursor;
            use crate::PublicKeyAlgorithm::*;

            let mut buf = Vec::new();
            sig.serialize(&mut buf).unwrap();
            let cur = Cursor::new(buf);

            #[allow(deprecated)]
            let sig_ = match &sig {
                Signature::RSA { .. } =>
                    Signature::parse(RSAEncryptSign, cur).unwrap(),
                Signature::DSA { .. } =>
                    Signature::parse(DSA, cur).unwrap(),
                Signature::ElGamal { .. } =>
                    Signature::parse(ElGamalEncryptSign, cur).unwrap(),
                Signature::EdDSA { .. } =>
                    Signature::parse(EdDSA, cur).unwrap(),
                Signature::ECDSA { .. } =>
                    Signature::parse(ECDSA, cur).unwrap(),

                Signature::Unknown { .. } => unreachable!(),
            };

            sig == sig_
        }
    }
}
