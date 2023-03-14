//! Pure Rust implementation of the NIST P-256 elliptic curve,
//! including support for the
//! [Elliptic Curve Digital Signature Algorithm (ECDSA)][ECDSA],
//! [Elliptic Curve Diffie-Hellman (ECDH)][ECDH], and general purpose
//! elliptic curve/field arithmetic which can be used to implement
//! protocols based on group operations.
//!
//! ## About NIST P-256
//!
//! NIST P-256 is a Weierstrass curve specified in FIPS 186-4:
//! Digital Signature Standard (DSS):
//!
//! <https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.186-4.pdf>
//!
//! Also known as `prime256v1` (ANSI X9.62) and `secp256r1` (SECG), P-256 is
//! included in the US National Security Agency's "Suite B" and is widely used
//! in Internet and connected device protocols like TLS, the X.509 PKI, and
//! Bluetooth.
//!
//! ## ⚠️ Security Warning
//!
//! The elliptic curve arithmetic contained in this crate has never been
//! independently audited!
//!
//! This crate has been designed with the goal of ensuring that secret-dependent
//! operations are performed in constant time (using the `subtle` crate and
//! constant-time formulas). However, it has not been thoroughly assessed to ensure
//! that generated assembly is constant time on common CPU architectures.
//!
//! USE AT YOUR OWN RISK!
//!
//! ## Minimum Supported Rust Version
//!
//! Rust **1.47** or higher.
//!
//! Minimum supported Rust version may be changed in the future, but it will be
//! accompanied with a minor version bump.
//!
//! [ECDSA]: https://en.wikipedia.org/wiki/Elliptic_Curve_Digital_Signature_Algorithm
//! [ECDH]: https://en.wikipedia.org/wiki/Elliptic-curve_Diffie%E2%80%93Hellman

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_root_url = "https://docs.rs/p256/0.8.1"
)]
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

#[cfg(feature = "arithmetic")]
mod arithmetic;

#[cfg(feature = "ecdh")]
#[cfg_attr(docsrs, doc(cfg(feature = "ecdh")))]
pub mod ecdh;

#[cfg(feature = "ecdsa-core")]
#[cfg_attr(docsrs, doc(cfg(feature = "ecdsa-core")))]
pub mod ecdsa;

#[cfg(any(feature = "test-vectors", test))]
#[cfg_attr(docsrs, doc(cfg(feature = "test-vectors")))]
pub mod test_vectors;

pub use elliptic_curve;

#[cfg(feature = "arithmetic")]
pub use arithmetic::{
    affine::AffinePoint,
    projective::ProjectivePoint,
    scalar::blinding::BlindedScalar,
    scalar::{NonZeroScalar, Scalar, ScalarBits},
};

#[cfg(feature = "pkcs8")]
#[cfg_attr(docsrs, doc(cfg(feature = "pkcs8")))]
pub use elliptic_curve::pkcs8;

use elliptic_curve::consts::U32;

/// NIST P-256 elliptic curve.
///
/// This curve is also known as prime256v1 (ANSI X9.62) and secp256r1 (SECG)
/// and is specified in FIPS 186-4: Digital Signature Standard (DSS):
///
/// <https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.186-4.pdf>
///
/// It's included in the US National Security Agency's "Suite B" and is widely
/// used in protocols like TLS and the associated X.509 PKI.
///
/// Its equation is `y² = x³ - 3x + b` over a ~256-bit prime field where `b` is
/// the "verifiably random"† constant:
///
/// ```text
/// b = 41058363725152142129326129780047268409114441015993725554835256314039467401291
/// ```
///
/// † *NOTE: the specific origins of this constant have never been fully disclosed
///   (it is the SHA-1 digest of an inexplicable NSA-selected constant)*
#[derive(Clone, Debug, Default, Eq, PartialEq, PartialOrd, Ord)]
pub struct NistP256;

impl elliptic_curve::Curve for NistP256 {
    /// 256-bit (32-byte)
    type FieldSize = U32;
}

#[cfg(target_pointer_width = "32")]
impl elliptic_curve::Order for NistP256 {
    type Limbs = [u32; 8];

    const ORDER: Self::Limbs = [
        0xfc63_2551,
        0xf3b9_cac2,
        0xa717_9e84,
        0xbce6_faad,
        0xffff_ffff,
        0xffff_ffff,
        0x0000_0000,
        0xffff_ffff,
    ];
}

#[cfg(target_pointer_width = "64")]
impl elliptic_curve::Order for NistP256 {
    type Limbs = [u64; 4];

    const ORDER: Self::Limbs = [
        0xf3b9_cac2_fc63_2551,
        0xbce6_faad_a717_9e84,
        0xffff_ffff_ffff_ffff,
        0xffff_ffff_0000_0000,
    ];
}

impl elliptic_curve::weierstrass::Curve for NistP256 {}

impl elliptic_curve::weierstrass::PointCompression for NistP256 {
    /// NIST P-256 points are typically uncompressed.
    const COMPRESS_POINTS: bool = false;
}

#[cfg(feature = "jwk")]
#[cfg_attr(docsrs, doc(cfg(feature = "jwk")))]
impl elliptic_curve::JwkParameters for NistP256 {
    const CRV: &'static str = "P-256";
}

#[cfg(feature = "pkcs8")]
impl elliptic_curve::AlgorithmParameters for NistP256 {
    const OID: pkcs8::ObjectIdentifier = pkcs8::ObjectIdentifier::new("1.2.840.10045.3.1.7");
}

/// NIST P-256 field element serialized as bytes.
///
/// Byte array containing a serialized field element value (base field or scalar).
pub type FieldBytes = elliptic_curve::FieldBytes<NistP256>;

/// NIST P-256 SEC1 encoded point.
pub type EncodedPoint = elliptic_curve::sec1::EncodedPoint<NistP256>;

/// NIST P-256 public key.
#[cfg(feature = "arithmetic")]
pub type PublicKey = elliptic_curve::PublicKey<NistP256>;

/// NIST P-256 secret key.
#[cfg(feature = "zeroize")]
#[cfg_attr(docsrs, doc(cfg(feature = "zeroize")))]
pub type SecretKey = elliptic_curve::SecretKey<NistP256>;

/// Bytes containing a NIST P-256 secret scalar
#[cfg(feature = "zeroize")]
#[cfg_attr(docsrs, doc(cfg(feature = "zeroize")))]
pub type SecretBytes = elliptic_curve::SecretBytes<NistP256>;

#[cfg(all(not(feature = "arithmetic"), feature = "zeroize"))]
impl elliptic_curve::SecretValue for NistP256 {
    type Secret = SecretBytes;

    /// Parse the secret value from bytes
    fn from_secret_bytes(bytes: &FieldBytes) -> Option<SecretBytes> {
        Some(bytes.clone().into())
    }
}

#[cfg(all(not(feature = "arithmetic"), feature = "zeroize"))]
impl elliptic_curve::sec1::ValidatePublicKey for NistP256 {}
