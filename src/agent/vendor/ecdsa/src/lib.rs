//! Elliptic Curve Digital Signature Algorithm (ECDSA) as specified in
//! [FIPS 186-4] (Digital Signature Standard)
//!
//! ## About
//!
//! This crate provides generic ECDSA support which can be used in the
//! following ways:
//!
//! - Generic implementation of ECDSA usable with the following crates:
//!   - [`k256`] (secp256k1)
//!   - [`p256`] (NIST P-256)
//! - ECDSA signature types alone which can be used to provide interoperability
//!   between other crates that provide an ECDSA implementation:
//!   - [`p384`] (NIST P-384)
//!
//! Any crates which provide an implementation of ECDSA for a particular
//! elliptic curve can leverage the types from this crate, along with the
//! [`k256`], [`p256`], and/or [`p384`] crates to expose ECDSA functionality in
//! a generic, interoperable way by leveraging the [`Signature`] type with in
//! conjunction with the [`signature::Signer`] and [`signature::Verifier`]
//! traits.
//!
//! For example, the [`ring-compat`] crate implements the [`signature::Signer`]
//! and [`signature::Verifier`] traits in conjunction with the
//! [`p256::ecdsa::Signature`] and [`p384::ecdsa::Signature`] types to
//! wrap the ECDSA implementations from [*ring*] in a generic, interoperable
//! API.
//!
//! ## Minimum Supported Rust Version
//!
//! Rust **1.47** or higher.
//!
//! Minimum supported Rust version may be changed in the future, but it will be
//! accompanied with a minor version bump.
//!
//! [FIPS 186-4]: https://csrc.nist.gov/publications/detail/fips/186/4/final
//! [`k256`]: https://docs.rs/k256
//! [`p256`]: https://docs.rs/p256
//! [`p256::ecdsa::Signature`]: https://docs.rs/p256/latest/p256/ecdsa/type.Signature.html
//! [`p384`]: https://docs.rs/p384
//! [`p384::ecdsa::Signature`]: https://docs.rs/p384/latest/p384/ecdsa/type.Signature.html
//! [`ring-compat`]: https://docs.rs/ring-compat
//! [*ring*]: https://docs.rs/ring

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo_small.png",
    html_root_url = "https://docs.rs/ecdsa/0.11.1"
)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "der")]
#[cfg_attr(docsrs, doc(cfg(feature = "der")))]
pub mod der;

#[cfg(feature = "dev")]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
pub mod dev;

#[cfg(feature = "hazmat")]
#[cfg_attr(docsrs, doc(cfg(feature = "hazmat")))]
pub mod hazmat;

#[cfg(feature = "sign")]
#[cfg_attr(docsrs, doc(cfg(feature = "sign")))]
pub mod rfc6979;

#[cfg(feature = "sign")]
mod sign;

#[cfg(feature = "verify")]
mod verify;

// Re-export the `elliptic-curve` crate (and select types)
pub use elliptic_curve::{self, sec1::EncodedPoint, weierstrass::Curve};

// Re-export the `signature` crate (and select types)
pub use signature::{self, Error};

#[cfg(feature = "sign")]
#[cfg_attr(docsrs, doc(cfg(feature = "sign")))]
pub use sign::SigningKey;

#[cfg(feature = "verify")]
#[cfg_attr(docsrs, doc(cfg(feature = "verify")))]
pub use verify::VerifyingKey;

use core::{
    convert::TryFrom,
    fmt::{self, Debug},
    ops::Add,
};
use elliptic_curve::{
    generic_array::{sequence::Concat, typenum::Unsigned, ArrayLength, GenericArray},
    FieldBytes, Order, ScalarBytes,
};

#[cfg(feature = "arithmetic")]
use elliptic_curve::{ff::PrimeField, NonZeroScalar, ProjectiveArithmetic, Scalar};

#[cfg(feature = "der")]
use elliptic_curve::generic_array::typenum::NonZero;

/// Size of a fixed sized signature for the given elliptic curve.
pub type SignatureSize<C> = <<C as elliptic_curve::Curve>::FieldSize as Add>::Output;

/// Fixed-size byte array containing an ECDSA signature
pub type SignatureBytes<C> = GenericArray<u8, SignatureSize<C>>;

/// ECDSA signature (fixed-size). Generic over elliptic curve types.
///
/// Serialized as fixed-sized big endian scalar values with no added framing:
///
/// - `r`: field element size for the given curve, big-endian
/// - `s`: field element size for the given curve, big-endian
///
/// For example, in a curve with a 256-bit modulus like NIST P-256 or
/// secp256k1, `r` and `s` will both be 32-bytes, resulting in a signature
/// with a total of 64-bytes.
///
/// ASN.1 DER-encoded signatures also supported via the
/// [`Signature::from_der`] and [`Signature::to_der`] methods.
#[derive(Clone, Eq, PartialEq)]
pub struct Signature<C: Curve + Order>
where
    SignatureSize<C>: ArrayLength<u8>,
{
    bytes: SignatureBytes<C>,
}

impl<C> Signature<C>
where
    C: Curve + Order,
    SignatureSize<C>: ArrayLength<u8>,
{
    /// Create a [`Signature`] from the serialized `r` and `s` scalar values
    /// which comprise the signature.
    pub fn from_scalars(
        r: impl Into<FieldBytes<C>>,
        s: impl Into<FieldBytes<C>>,
    ) -> Result<Self, Error> {
        Self::try_from(r.into().concat(s.into()).as_slice())
    }

    /// Parse a signature from ASN.1 DER
    #[cfg(feature = "der")]
    #[cfg_attr(docsrs, doc(cfg(feature = "der")))]
    pub fn from_der(bytes: &[u8]) -> Result<Self, Error>
    where
        C::FieldSize: Add + ArrayLength<u8> + NonZero,
        der::MaxSize<C>: ArrayLength<u8>,
        <C::FieldSize as Add>::Output: Add<der::MaxOverhead> + ArrayLength<u8>,
    {
        der::Signature::<C>::try_from(bytes).and_then(Self::try_from)
    }

    /// Serialize this signature as ASN.1 DER
    #[cfg(feature = "der")]
    #[cfg_attr(docsrs, doc(cfg(feature = "der")))]
    pub fn to_der(&self) -> der::Signature<C>
    where
        C::FieldSize: Add + ArrayLength<u8> + NonZero,
        der::MaxSize<C>: ArrayLength<u8>,
        <C::FieldSize as Add>::Output: Add<der::MaxOverhead> + ArrayLength<u8>,
    {
        let (r, s) = self.bytes.split_at(C::FieldSize::to_usize());
        der::Signature::from_scalar_bytes(r, s)
    }
}

#[cfg(feature = "arithmetic")]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
impl<C> Signature<C>
where
    C: Curve + Order + ProjectiveArithmetic,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    <Scalar<C> as PrimeField>::Repr: From<Scalar<C>> + for<'a> From<&'a Scalar<C>>,
    SignatureSize<C>: ArrayLength<u8>,
{
    /// Get the `r` component of this signature
    pub fn r(&self) -> NonZeroScalar<C> {
        let r_bytes = GenericArray::clone_from_slice(&self.bytes[..C::FieldSize::to_usize()]);
        NonZeroScalar::from_repr(r_bytes)
            .unwrap_or_else(|| unreachable!("r-component ensured valid in constructor"))
    }

    /// Get the `s` component of this signature
    pub fn s(&self) -> NonZeroScalar<C> {
        let s_bytes = GenericArray::clone_from_slice(&self.bytes[C::FieldSize::to_usize()..]);
        NonZeroScalar::from_repr(s_bytes)
            .unwrap_or_else(|| unreachable!("r-component ensured valid in constructor"))
    }

    /// Normalize signature into "low S" form as described in
    /// [BIP 0062: Dealing with Malleability][1].
    ///
    /// [1]: https://github.com/bitcoin/bips/blob/master/bip-0062.mediawiki
    pub fn normalize_s(&mut self) -> Result<bool, Error>
    where
        Scalar<C>: NormalizeLow,
    {
        let s_bytes = GenericArray::from_mut_slice(&mut self.bytes[C::FieldSize::to_usize()..]);
        Scalar::<C>::from_repr(s_bytes.clone())
            .map(|s| {
                let (s_low, was_high) = s.normalize_low();

                if was_high {
                    s_bytes.copy_from_slice(&s_low.to_repr());
                }

                was_high
            })
            .ok_or_else(Error::new)
    }
}

impl<C> signature::Signature for Signature<C>
where
    C: Curve + Order,
    SignatureSize<C>: ArrayLength<u8>,
{
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Self::try_from(bytes)
    }
}

impl<C> AsRef<[u8]> for Signature<C>
where
    C: Curve + Order,
    SignatureSize<C>: ArrayLength<u8>,
{
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

impl<C> Copy for Signature<C>
where
    C: Curve + Order,
    SignatureSize<C>: ArrayLength<u8>,
    <SignatureSize<C> as ArrayLength<u8>>::ArrayType: Copy,
{
}

impl<C> Debug for Signature<C>
where
    C: Curve + Order,
    SignatureSize<C>: ArrayLength<u8>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ecdsa::Signature<{:?}>({:?})",
            C::default(),
            self.as_ref()
        )
    }
}

impl<C> TryFrom<&[u8]> for Signature<C>
where
    C: Curve + Order,
    SignatureSize<C>: ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != <SignatureSize<C>>::to_usize() {
            return Err(Error::new());
        }

        for scalar in bytes.chunks_exact(C::FieldSize::to_usize()) {
            if scalar.iter().all(|&byte| byte == 0) {
                return Err(Error::new());
            }

            if ScalarBytes::<C>::new(GenericArray::clone_from_slice(scalar))
                .is_none()
                .into()
            {
                return Err(Error::new());
            }
        }

        Ok(Self {
            bytes: GenericArray::clone_from_slice(bytes),
        })
    }
}

#[cfg(feature = "der")]
#[cfg_attr(docsrs, doc(cfg(feature = "der")))]
impl<C> TryFrom<der::Signature<C>> for Signature<C>
where
    C: Curve + Order,
    C::FieldSize: Add + ArrayLength<u8> + NonZero,
    der::MaxSize<C>: ArrayLength<u8>,
    <C::FieldSize as Add>::Output: Add<der::MaxOverhead> + ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(doc: der::Signature<C>) -> Result<Signature<C>, Error> {
        let mut bytes = SignatureBytes::<C>::default();
        let scalar_size = C::FieldSize::to_usize();
        let r_begin = scalar_size.checked_sub(doc.r().len()).unwrap();
        let s_begin = bytes.len().checked_sub(doc.s().len()).unwrap();

        bytes[r_begin..scalar_size].copy_from_slice(doc.r());
        bytes[s_begin..].copy_from_slice(doc.s());
        Self::try_from(bytes.as_slice())
    }
}

/// Normalize a scalar (i.e. ECDSA S) to the lower half the field, as described
/// in [BIP 0062: Dealing with Malleability][1].
///
/// [1]: https://github.com/bitcoin/bips/blob/master/bip-0062.mediawiki
pub trait NormalizeLow: Sized {
    /// Normalize scalar to the lower half of the field (i.e. negate it if it's
    /// larger than half the curve's order).
    /// Returns a tuple with the new scalar and a boolean indicating whether the given scalar
    /// was in the higher half.
    ///
    /// May be implemented to work in variable time.
    fn normalize_low(&self) -> (Self, bool);
}
