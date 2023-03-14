// -*- mode: rust; -*-
//
// This file is part of ed25519-dalek.
// Copyright (c) 2017-2019 isis lovecruft
// See LICENSE for licensing information.
//
// Authors:
// - isis agora lovecruft <isis@patternsinthevoid.net>

//! An ed25519 signature.

use core::convert::TryFrom;
use core::fmt::Debug;

use curve25519_dalek::edwards::CompressedEdwardsY;
use curve25519_dalek::scalar::Scalar;
use ed25519::signature::Signature as _;

use crate::constants::*;
use crate::errors::*;

/// An ed25519 signature.
///
/// # Note
///
/// These signatures, unlike the ed25519 signature reference implementation, are
/// "detached"—that is, they do **not** include a copy of the message which has
/// been signed.
#[allow(non_snake_case)]
#[derive(Copy, Eq, PartialEq)]
pub(crate) struct InternalSignature {
    /// `R` is an `EdwardsPoint`, formed by using an hash function with
    /// 512-bits output to produce the digest of:
    ///
    /// - the nonce half of the `ExpandedSecretKey`, and
    /// - the message to be signed.
    ///
    /// This digest is then interpreted as a `Scalar` and reduced into an
    /// element in ℤ/lℤ.  The scalar is then multiplied by the distinguished
    /// basepoint to produce `R`, and `EdwardsPoint`.
    pub(crate) R: CompressedEdwardsY,

    /// `s` is a `Scalar`, formed by using an hash function with 512-bits output
    /// to produce the digest of:
    ///
    /// - the `r` portion of this `Signature`,
    /// - the `PublicKey` which should be used to verify this `Signature`, and
    /// - the message to be signed.
    ///
    /// This digest is then interpreted as a `Scalar` and reduced into an
    /// element in ℤ/lℤ.
    pub(crate) s: Scalar,
}

impl Clone for InternalSignature {
    fn clone(&self) -> Self {
        *self
    }
}

impl Debug for InternalSignature {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        write!(f, "Signature( R: {:?}, s: {:?} )", &self.R, &self.s)
    }
}

#[cfg(feature = "legacy_compatibility")]
#[inline(always)]
fn check_scalar(bytes: [u8; 32]) -> Result<Scalar, SignatureError> {
    // The highest 3 bits must not be set.  No other checking for the
    // remaining 2^253 - 2^252 + 27742317777372353535851937790883648493
    // potential non-reduced scalars is performed.
    //
    // This is compatible with ed25519-donna and libsodium when
    // -DED25519_COMPAT is NOT specified.
    if bytes[31] & 224 != 0 {
        return Err(InternalError::ScalarFormatError.into());
    }

    Ok(Scalar::from_bits(bytes))
}

#[cfg(not(feature = "legacy_compatibility"))]
#[inline(always)]
fn check_scalar(bytes: [u8; 32]) -> Result<Scalar, SignatureError> {
    // Since this is only used in signature deserialisation (i.e. upon
    // verification), we can do a "succeed fast" trick by checking that the most
    // significant 4 bits are unset.  If they are unset, we can succeed fast
    // because we are guaranteed that the scalar is fully reduced.  However, if
    // the 4th most significant bit is set, we must do the full reduction check,
    // as the order of the basepoint is roughly a 2^(252.5) bit number.
    //
    // This succeed-fast trick should succeed for roughly half of all scalars.
    if bytes[31] & 240 == 0 {
        return Ok(Scalar::from_bits(bytes))
    }

    match Scalar::from_canonical_bytes(bytes) {
        None => return Err(InternalError::ScalarFormatError.into()),
        Some(x) => return Ok(x),
    };
}

impl InternalSignature {
    /// Convert this `Signature` to a byte array.
    #[inline]
    pub fn to_bytes(&self) -> [u8; SIGNATURE_LENGTH] {
        let mut signature_bytes: [u8; SIGNATURE_LENGTH] = [0u8; SIGNATURE_LENGTH];

        signature_bytes[..32].copy_from_slice(&self.R.as_bytes()[..]);
        signature_bytes[32..].copy_from_slice(&self.s.as_bytes()[..]);
        signature_bytes
    }

    /// Construct a `Signature` from a slice of bytes.
    ///
    /// # Scalar Malleability Checking
    ///
    /// As originally specified in the ed25519 paper (cf. the "Malleability"
    /// section of the README in this repo), no checks whatsoever were performed
    /// for signature malleability.
    ///
    /// Later, a semi-functional, hacky check was added to most libraries to
    /// "ensure" that the scalar portion, `s`, of the signature was reduced `mod
    /// \ell`, the order of the basepoint:
    ///
    /// ```ignore
    /// if signature.s[31] & 224 != 0 {
    ///     return Err();
    /// }
    /// ```
    ///
    /// This bit-twiddling ensures that the most significant three bits of the
    /// scalar are not set:
    ///
    /// ```python,ignore
    /// >>> 0b00010000 & 224
    /// 0
    /// >>> 0b00100000 & 224
    /// 32
    /// >>> 0b01000000 & 224
    /// 64
    /// >>> 0b10000000 & 224
    /// 128
    /// ```
    ///
    /// However, this check is hacky and insufficient to check that the scalar is
    /// fully reduced `mod \ell = 2^252 + 27742317777372353535851937790883648493` as
    /// it leaves us with a guanteed bound of 253 bits.  This means that there are
    /// `2^253 - 2^252 + 2774231777737235353585193779088364849311` remaining scalars
    /// which could cause malleabilllity.
    ///
    /// RFC8032 [states](https://tools.ietf.org/html/rfc8032#section-5.1.7):
    ///
    /// > To verify a signature on a message M using public key A, [...]
    /// > first split the signature into two 32-octet halves.  Decode the first
    /// > half as a point R, and the second half as an integer S, in the range
    /// > 0 <= s < L.  Decode the public key A as point A'.  If any of the
    /// > decodings fail (including S being out of range), the signature is
    /// > invalid.
    ///
    /// However, by the time this was standardised, most libraries in use were
    /// only checking the most significant three bits.  (See also the
    /// documentation for `PublicKey.verify_strict`.)
    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Result<InternalSignature, SignatureError> {
        if bytes.len() != SIGNATURE_LENGTH {
            return Err(InternalError::BytesLengthError {
                name: "Signature",
                length: SIGNATURE_LENGTH,
            }.into());
        }
        let mut lower: [u8; 32] = [0u8; 32];
        let mut upper: [u8; 32] = [0u8; 32];

        lower.copy_from_slice(&bytes[..32]);
        upper.copy_from_slice(&bytes[32..]);

        let s: Scalar;

        match check_scalar(upper) {
            Ok(x)  => s = x,
            Err(x) => return Err(x),
        }

        Ok(InternalSignature {
            R: CompressedEdwardsY(lower),
            s: s,
        })
    }
}

impl TryFrom<&ed25519::Signature> for InternalSignature {
    type Error = SignatureError;

    fn try_from(sig: &ed25519::Signature) -> Result<InternalSignature, SignatureError> {
        InternalSignature::from_bytes(sig.as_bytes())
    }
}

impl From<InternalSignature> for ed25519::Signature {
    fn from(sig: InternalSignature) -> ed25519::Signature {
        ed25519::Signature::from_bytes(&sig.to_bytes()).unwrap()
    }
}
