//! Elliptic Curve Digital Signature Algorithm (ECDSA)
//!
//! This module contains support for computing and verifying ECDSA signatures.
//! To use it, you will need to enable one of the two following Cargo features:
//!
//! - `ecdsa-core`: provides only the [`Signature`] type (which represents an
//!   ECDSA/P-256 signature). Does not require the `arithmetic` feature.
//!   This is useful for 3rd-party crates which wish to use the `Signature`
//!   type for interoperability purposes (particularly in conjunction with the
//!   [`signature::Signer`] trait. Example use cases for this include other
//!   software implementations of ECDSA/P-256 and wrappers for cloud KMS
//!   services or hardware devices (HSM or crypto hardware wallet).
//! - `ecdsa`: provides `ecdsa-core` features plus the [`SigningKey`] and
//!   [`VerifyingKey`] types which natively implement ECDSA/P-256 signing and
//!   verification.
//!
//! ## Signing/Verification Example
//!
//! This example requires the `ecdsa` Cargo feature is enabled:
//!
//! ```
//! # #[cfg(feature = "ecdsa")]
//! # {
//! use p256::{
//!     ecdsa::{SigningKey, Signature, signature::Signer},
//! };
//! use rand_core::OsRng; // requires 'getrandom' feature
//!
//! // Signing
//! let signing_key = SigningKey::random(&mut OsRng); // Serialize with `::to_bytes()`
//! let message = b"ECDSA proves knowledge of a secret number in the context of a single message";
//! let signature = signing_key.sign(message);
//!
//! // Verification
//! use p256::ecdsa::{VerifyingKey, signature::Verifier};
//!
//! let verify_key = VerifyingKey::from(&signing_key); // Serialize with `::to_encoded_point()`
//! assert!(verify_key.verify(message, &signature).is_ok());
//! # }
//! ```

pub use ecdsa_core::signature::{self, Error};

use super::NistP256;

#[cfg(feature = "ecdsa")]
use {
    crate::{AffinePoint, ProjectivePoint, Scalar},
    core::borrow::Borrow,
    ecdsa_core::hazmat::{SignPrimitive, VerifyPrimitive},
    elliptic_curve::ops::Invert,
};

/// ECDSA/P-256 signature (fixed-size)
pub type Signature = ecdsa_core::Signature<NistP256>;

/// ECDSA/P-256 signature (ASN.1 DER encoded)
pub type DerSignature = ecdsa_core::der::Signature<NistP256>;

/// ECDSA/P-256 signing key
#[cfg(feature = "ecdsa")]
#[cfg_attr(docsrs, doc(cfg(feature = "ecdsa")))]
pub type SigningKey = ecdsa_core::SigningKey<NistP256>;

/// ECDSA/P-256 verification key (i.e. public key)
#[cfg(feature = "ecdsa")]
#[cfg_attr(docsrs, doc(cfg(feature = "ecdsa")))]
pub type VerifyingKey = ecdsa_core::VerifyingKey<NistP256>;

#[cfg(feature = "sha256")]
#[cfg_attr(docsrs, doc(cfg(feature = "sha256")))]
impl ecdsa_core::hazmat::DigestPrimitive for NistP256 {
    type Digest = sha2::Sha256;
}

#[cfg(feature = "ecdsa")]
impl SignPrimitive<NistP256> for Scalar {
    #[allow(clippy::many_single_char_names)]
    fn try_sign_prehashed<K>(&self, ephemeral_scalar: &K, z: &Scalar) -> Result<Signature, Error>
    where
        K: Borrow<Scalar> + Invert<Output = Scalar>,
    {
        let k_inverse = ephemeral_scalar.invert();
        let k = ephemeral_scalar.borrow();

        if k_inverse.is_none().into() || k.is_zero().into() {
            return Err(Error::new());
        }

        let k_inverse = k_inverse.unwrap();

        // Compute `x`-coordinate of affine point 𝑘×𝑮
        let x = (ProjectivePoint::generator() * k).to_affine().x;

        // Lift `x` (element of base field) to serialized big endian integer,
        // then reduce it to an element of the scalar field
        let r = Scalar::from_bytes_reduced(&x.to_bytes());

        // Compute `s` as a signature over `r` and `z`.
        let s = k_inverse * (z + &(r * self));

        if s.is_zero().into() {
            return Err(Error::new());
        }

        Signature::from_scalars(r, s)
    }
}

#[cfg(feature = "ecdsa")]
impl VerifyPrimitive<NistP256> for AffinePoint {
    fn verify_prehashed(&self, z: &Scalar, signature: &Signature) -> Result<(), Error> {
        let r = signature.r();
        let s = signature.s();
        let s_inv = s.invert().unwrap();
        let u1 = z * &s_inv;
        let u2 = *r * s_inv;

        let x = ((ProjectivePoint::generator() * u1) + (ProjectivePoint::from(*self) * u2))
            .to_affine()
            .x;

        if Scalar::from_bytes_reduced(&x.to_bytes()) == *r {
            Ok(())
        } else {
            Err(Error::new())
        }
    }
}

#[cfg(all(test, feature = "ecdsa"))]
mod tests {
    use crate::{
        ecdsa::{signature::Signer, SigningKey},
        test_vectors::ecdsa::ECDSA_TEST_VECTORS,
        BlindedScalar, Scalar,
    };
    use ecdsa_core::hazmat::SignPrimitive;
    use elliptic_curve::{ff::PrimeField, generic_array::GenericArray, rand_core::OsRng};
    use hex_literal::hex;

    // Test vector from RFC 6979 Appendix 2.5 (NIST P-256 + SHA-256)
    // <https://tools.ietf.org/html/rfc6979#appendix-A.2.5>
    #[test]
    fn rfc6979() {
        let x = &hex!("c9afa9d845ba75166b5c215767b1d6934e50c3db36e89b127b8a622b120f6721");
        let signer = SigningKey::from_bytes(x).unwrap();
        let signature = signer.sign(b"sample");
        assert_eq!(
            signature.as_ref(),
            &hex!(
                "efd48b2aacb6a8fd1140dd9cd45e81d69d2c877b56aaf991c34d0ea84eaf3716
                     f7cb1c942d657c41d436c7a1b6e29f65f3e900dbb9aff4064dc4ab2f843acda8"
            )[..]
        );
    }

    #[test]
    fn scalar_blinding() {
        let vector = &ECDSA_TEST_VECTORS[0];
        let d = Scalar::from_repr(GenericArray::clone_from_slice(vector.d)).unwrap();
        let k = Scalar::from_repr(GenericArray::clone_from_slice(vector.k)).unwrap();
        let k_blinded = BlindedScalar::new(k, &mut OsRng);
        let z = Scalar::from_repr(GenericArray::clone_from_slice(vector.m)).unwrap();
        let sig = d.try_sign_prehashed(&k_blinded, &z).unwrap();

        assert_eq!(vector.r, sig.r().to_bytes().as_slice());
        assert_eq!(vector.s, sig.s().to_bytes().as_slice());
    }

    mod sign {
        use crate::{test_vectors::ecdsa::ECDSA_TEST_VECTORS, NistP256};
        ecdsa_core::new_signing_test!(NistP256, ECDSA_TEST_VECTORS);
    }

    mod verify {
        use crate::{test_vectors::ecdsa::ECDSA_TEST_VECTORS, NistP256};
        ecdsa_core::new_verification_test!(NistP256, ECDSA_TEST_VECTORS);
    }

    mod wycheproof {
        use crate::NistP256;
        ecdsa_core::new_wycheproof_test!(wycheproof, "wycheproof", NistP256);
    }
}
