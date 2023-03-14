//! Low-level ECDSA primitives.
//!
//! # ⚠️ Warning: Hazmat!
//!
//! YOU PROBABLY DON'T WANT TO USE THESE!
//!
//! These primitives are easy-to-misuse low-level interfaces intended to be
//! implemented by elliptic curve crates and consumed only by this crate!
//!
//! If you are an end user / non-expert in cryptography, do not use these!
//! Failure to use them correctly can lead to catastrophic failures including
//! FULL PRIVATE KEY RECOVERY!

#[cfg(feature = "arithmetic")]
use {
    crate::SignatureSize,
    core::borrow::Borrow,
    elliptic_curve::{ff::PrimeField, ops::Invert, FieldBytes, ProjectiveArithmetic, Scalar},
    signature::Error,
};

#[cfg(feature = "digest")]
use crate::signature::{digest::Digest, PrehashSignature};

#[cfg(any(feature = "arithmetic", feature = "digest"))]
use crate::{
    elliptic_curve::{generic_array::ArrayLength, weierstrass::Curve},
    Order, Signature,
};

/// Try to sign the given prehashed message using ECDSA.
///
/// This trait is intended to be implemented on a type with access
/// to the secret scalar via `&self`, such as particular curve's `Scalar` type,
/// or potentially a key handle to a hardware device.
#[cfg(feature = "arithmetic")]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
pub trait SignPrimitive<C>
where
    C: Curve + Order + ProjectiveArithmetic,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    SignatureSize<C>: ArrayLength<u8>,
{
    /// Try to sign the prehashed message.
    ///
    /// Accepts the following arguments:
    ///
    /// - `ephemeral_scalar`: ECDSA `k` value. MUST BE UNIFORMLY RANDOM!!!
    /// - `hashed_msg`: scalar computed from a hashed message digest to be signed.
    ///   MUST BE OUTPUT OF A CRYPTOGRAPHICALLY SECURE DIGEST ALGORITHM!!!
    fn try_sign_prehashed<K: Borrow<Scalar<C>> + Invert<Output = Scalar<C>>>(
        &self,
        ephemeral_scalar: &K,
        hashed_msg: &Scalar<C>,
    ) -> Result<Signature<C>, Error>;
}

/// [`SignPrimitive`] for signature implementations that can provide public key
/// recovery implementation.
#[cfg(feature = "arithmetic")]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
pub trait RecoverableSignPrimitive<C>
where
    C: Curve + Order + ProjectiveArithmetic,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    SignatureSize<C>: ArrayLength<u8>,
{
    /// Try to sign the prehashed message.
    ///
    /// Accepts the same arguments as [`SignPrimitive::try_sign_prehashed`]
    /// but returns a boolean flag which indicates whether or not the
    /// y-coordinate of the computed 𝐑 = 𝑘×𝑮 point is odd, which can be
    /// incorporated into recoverable signatures.
    fn try_sign_recoverable_prehashed<K: Borrow<Scalar<C>> + Invert<Output = Scalar<C>>>(
        &self,
        ephemeral_scalar: &K,
        hashed_msg: &Scalar<C>,
    ) -> Result<(Signature<C>, bool), Error>;
}

#[cfg(feature = "arithmetic")]
impl<C, T> SignPrimitive<C> for T
where
    C: Curve + Order + ProjectiveArithmetic,
    T: RecoverableSignPrimitive<C>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    SignatureSize<C>: ArrayLength<u8>,
{
    fn try_sign_prehashed<K: Borrow<Scalar<C>> + Invert<Output = Scalar<C>>>(
        &self,
        ephemeral_scalar: &K,
        hashed_msg: &Scalar<C>,
    ) -> Result<Signature<C>, Error> {
        self.try_sign_recoverable_prehashed(ephemeral_scalar, hashed_msg)
            .map(|res| res.0)
    }
}

/// Verify the given prehashed message using ECDSA.
///
/// This trait is intended to be implemented on type which can access
/// the affine point represeting the public key via `&self`, such as a
/// particular curve's `AffinePoint` type.
#[cfg(feature = "arithmetic")]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
pub trait VerifyPrimitive<C>
where
    C: Curve + Order + ProjectiveArithmetic,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    SignatureSize<C>: ArrayLength<u8>,
{
    /// Verify the prehashed message against the provided signature
    ///
    /// Accepts the following arguments:
    ///
    /// - `hashed_msg`: prehashed message to be verified
    /// - `signature`: signature to be verified against the key and message
    fn verify_prehashed(
        &self,
        hashed_msg: &Scalar<C>,
        signature: &Signature<C>,
    ) -> Result<(), Error>;
}

/// Bind a preferred [`Digest`] algorithm to an elliptic curve type.
///
/// Generally there is a preferred variety of the SHA-2 family used with ECDSA
/// for a particular elliptic curve.
///
/// This trait can be used to specify it, and with it receive a blanket impl of
/// [`PrehashSignature`], used by [`signature_derive`][1]) for the [`Signature`]
/// type for a particular elliptic curve.
///
/// [1]: https://github.com/RustCrypto/traits/tree/master/signature/derive
#[cfg(feature = "digest")]
#[cfg_attr(docsrs, doc(cfg(feature = "digest")))]
pub trait DigestPrimitive: Curve + Order {
    /// Preferred digest to use when computing ECDSA signatures for this
    /// elliptic curve. This should be a member of the SHA-2 family.
    type Digest: Digest;
}

/// Instantiate this type from the output of a digest.
///
/// This trait is intended for use in ECDSA and should perform a conversion
/// which is compatible with the rules for calculating `h` from `H(M)` set out
/// in RFC6979 section 2.4. This conversion cannot fail.
///
/// This trait may also be useful for other hash-to-scalar or hash-to-curve
/// use cases.
#[cfg(feature = "digest")]
#[cfg_attr(docsrs, doc(cfg(feature = "digest")))]
pub trait FromDigest<C: Curve> {
    /// Instantiate this type from a [`Digest`] instance
    fn from_digest<D>(digest: D) -> Self
    where
        D: Digest<OutputSize = C::FieldSize>;
}

#[cfg(feature = "digest")]
impl<C> PrehashSignature for Signature<C>
where
    C: DigestPrimitive,
    <C::FieldSize as core::ops::Add>::Output: ArrayLength<u8>,
{
    type Digest = C::Digest;
}
